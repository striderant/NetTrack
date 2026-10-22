//! Per-connection TCP extended statistics.
//!
//! `GetPerTcpConnectionEStats` reports the exact number of bytes a single TCP
//! connection has moved, which is the only way to attribute bandwidth to one
//! socket rather than to an interface.
//!
//! Two things make it awkward:
//!
//! * the counters have to be *enabled* per connection first, with
//!   `SetPerTcpConnectionEStats`, and
//! * the call takes one round trip per connection.
//!
//! Enabling is therefore done once per connection and remembered, while the
//! reads are spread over a small thread pool. Connections that Windows cannot
//! be asked about (listeners, or sockets in a transient state) are skipped.

use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::Mutex;

use rayon::prelude::*;
use windows::Win32::NetworkManagement::IpHelper::{
    GetPerTcp6ConnectionEStats, GetPerTcpConnectionEStats, MIB_TCP6ROW, MIB_TCPROW_LH,
    MIB_TCP_STATE, SetPerTcp6ConnectionEStats, SetPerTcpConnectionEStats, TcpConnectionEstatsData,
    TCP_ESTATS_DATA_ROD_v0, TCP_ESTATS_DATA_RW_v0,
};
use windows::Win32::Networking::WinSock::IN6_ADDR;

use crate::model::{FlowKey, Proto, TcpState};
use crate::net::sockets::SocketEntry;

/// Byte and segment counters for one TCP connection.
#[derive(Debug, Clone, Copy, Default)]
pub struct TcpCounters {
    /// Payload bytes sent.
    pub bytes_sent: u64,
    /// Payload bytes received.
    pub bytes_received: u64,
    pub segments_sent: u64,
    pub segments_received: u64,
}

/// Collects extended statistics for TCP connections.
pub struct EstatsCollector {
    /// Connections whose counters have been enabled already.
    enabled: Mutex<HashSet<FlowKey>>,
    pool: rayon::ThreadPool,
}

impl EstatsCollector {
    /// Create a collector with a dedicated worker pool.
    pub fn new(workers: usize) -> Self {
        let workers = workers.clamp(1, 16);
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .thread_name(|i| format!("nettrack-estats-{i}"))
            .build()
            .unwrap_or_else(|_| rayon::ThreadPoolBuilder::new().build().expect("thread pool"));

        Self {
            enabled: Mutex::new(HashSet::new()),
            pool,
        }
    }

    /// Read the counters for every eligible connection in `entries`.
    ///
    /// At most `max` connections are polled; the returned tuple reports how
    /// many were actually sampled so the interface can say so.
    pub fn query(
        &self,
        entries: &[SocketEntry],
        max: usize,
    ) -> (HashMap<FlowKey, TcpCounters>, usize, usize) {
        let eligible: Vec<&SocketEntry> = entries
            .iter()
            .filter(|e| {
                e.proto == Proto::Tcp
                    && match e.state {
                        crate::model::ConnState::Tcp(s) => s.supports_estats(),
                        _ => false,
                    }
            })
            .take(max)
            .collect();

        let total = eligible.len();

        let results: Vec<Option<(FlowKey, TcpCounters)>> = self.pool.install(|| {
            eligible
                .par_iter()
                .map(|entry| {
                    let key = FlowKey {
                        proto: entry.proto,
                        local_addr: entry.local.addr,
                        local_port: entry.local.port,
                        remote_addr: entry.remote.map(|e| e.addr),
                        remote_port: entry.remote.map(|e| e.port),
                    };
                    self.read_counters(entry).map(|c| (key, c))
                })
                .collect()
        });

        let mut out = HashMap::with_capacity(total);
        for item in results.into_iter().flatten() {
            out.insert(item.0, item.1);
        }

        (out, total, entries.len())
    }

    /// Whether the counters for a connection have been enabled already.
    pub fn is_enabled(&self, key: &FlowKey) -> bool {
        self.enabled.lock().is_ok_and(|set| set.contains(key))
    }

    /// Enable the counters for a connection, if they are not enabled yet.
    fn ensure_enabled(&self, entry: &SocketEntry) -> bool {
        let key = FlowKey {
            proto: entry.proto,
            local_addr: entry.local.addr,
            local_port: entry.local.port,
            remote_addr: entry.remote.map(|e| e.addr),
            remote_port: entry.remote.map(|e| e.port),
        };

        if self.enabled.lock().is_ok_and(|set| set.contains(&key)) {
            return true;
        }

        let ok = match (&entry.local.addr, &entry.remote) {
            (IpAddr::V4(local), Some(remote)) => {
                let IpAddr::V4(remote) = remote.addr else {
                    return false;
                };
                let row = tcp_row(
                    *local,
                    entry.local.port,
                    remote,
                    remote_port_of(entry),
                    state_of(entry),
                );
                set_enabled(&row)
            }
            (IpAddr::V6(local), Some(remote)) => {
                let IpAddr::V6(remote) = remote.addr else {
                    return false;
                };
                let row = tcp6_row(
                    *local,
                    entry.local.port,
                    remote,
                    remote_port_of(entry),
                    state_of(entry),
                );
                set_enabled6(&row)
            }
            _ => false,
        };

        if !ok {
            tracing::debug!(
                "could not enable the extended statistics for {}:{}",
                entry.local.addr,
                entry.local.port
            );
        }

        if ok && let Ok(mut set) = self.enabled.lock() {
            set.insert(key);
        }
        ok
    }

    /// Read the counters for one connection, enabling them first if needed.
    fn read_counters(&self, entry: &SocketEntry) -> Option<TcpCounters> {
        if !self.ensure_enabled(entry) {
            return None;
        }

        match (&entry.local.addr, &entry.remote) {
            (IpAddr::V4(local), Some(remote)) => {
                let IpAddr::V4(remote) = remote.addr else {
                    return None;
                };
                let row = tcp_row(
                    *local,
                    entry.local.port,
                    remote,
                    remote_port_of(entry),
                    state_of(entry),
                );
                read_v4(&row)
            }
            (IpAddr::V6(local), Some(remote)) => {
                let IpAddr::V6(remote) = remote.addr else {
                    return None;
                };
                let row = tcp6_row(
                    *local,
                    entry.local.port,
                    remote,
                    remote_port_of(entry),
                    state_of(entry),
                );
                read_v6(&row)
            }
            _ => None,
        }
    }
}

fn state_of(entry: &SocketEntry) -> u32 {
    match entry.state {
        crate::model::ConnState::Tcp(s) => s.raw(),
        _ => 0,
    }
}

/// The port of the peer of a connection.
fn remote_port_of(entry: &SocketEntry) -> u16 {
    entry.remote.map(|r| r.port).unwrap_or(0)
}

/// Build an IPv4 `MIB_TCPROW` for the statistics API.
fn tcp_row(
    local: Ipv4Addr,
    local_port: u16,
    remote: Ipv4Addr,
    remote_port: u16,
    state: u32,
) -> MIB_TCPROW_LH {
    let mut row = MIB_TCPROW_LH::default();
    row.Anonymous.dwState = state;
    row.dwLocalAddr = u32::from(local);
    // The MIB rows keep the port in network byte order.
    row.dwLocalPort = u32::from(local_port.to_be());
    row.dwRemoteAddr = u32::from(remote);
    row.dwRemotePort = u32::from(remote_port.to_be());
    row
}

/// Build an IPv6 `MIB_TCP6ROW` for the statistics API.
fn tcp6_row(
    local: Ipv6Addr,
    local_port: u16,
    remote: Ipv6Addr,
    remote_port: u16,
    state: u32,
) -> MIB_TCP6ROW {
    let mut row = MIB_TCP6ROW::default();
    row.State = MIB_TCP_STATE(state as i32);
    row.LocalAddr = in6_addr(local);
    row.dwLocalPort = u32::from(local_port.to_be());
    row.RemoteAddr = in6_addr(remote);
    row.dwRemotePort = u32::from(remote_port.to_be());
    row
}

/// Wrap an IPv6 address in the WinSock structure the API expects.
fn in6_addr(addr: Ipv6Addr) -> IN6_ADDR {
    let mut out = IN6_ADDR::default();
    // SAFETY: `IN6_ADDR` is a plain wrapper around sixteen bytes.
    unsafe {
        let bytes = &mut *(&mut out as *mut IN6_ADDR as *mut [u8; 16]);
        bytes.copy_from_slice(&addr.octets());
    }
    out
}

/// Turn on counter collection for one connection.
fn set_enabled(row: &MIB_TCPROW_LH) -> bool {
    let rw = TCP_ESTATS_DATA_RW_v0 {
        EnableCollection: true,
    };
    // SAFETY: the buffer is exactly the structure the API expects.
    let bytes = unsafe {
        std::slice::from_raw_parts(
            &rw as *const _ as *const u8,
            std::mem::size_of::<TCP_ESTATS_DATA_RW_v0>(),
        )
    };
    let status = unsafe { SetPerTcpConnectionEStats(row, TcpConnectionEstatsData, bytes, 0, 0) };
    if status != 0 {
        tracing::debug!(
            "SetPerTcpConnectionEStats failed with 0x{status:08x} for {}:{}",
            row.dwLocalAddr,
            row.dwLocalPort
        );
    }
    status == 0
}

fn set_enabled6(row: &MIB_TCP6ROW) -> bool {
    let rw = TCP_ESTATS_DATA_RW_v0 {
        EnableCollection: true,
    };
    // SAFETY: the buffer is exactly the structure the API expects.
    let bytes = unsafe {
        std::slice::from_raw_parts(
            &rw as *const _ as *const u8,
            std::mem::size_of::<TCP_ESTATS_DATA_RW_v0>(),
        )
    };
    unsafe { SetPerTcp6ConnectionEStats(row, TcpConnectionEstatsData, bytes, 0, 0) == 0 }
}

fn read_v4(row: &MIB_TCPROW_LH) -> Option<TcpCounters> {
    let mut rod = TCP_ESTATS_DATA_ROD_v0::default();
    let size = std::mem::size_of::<TCP_ESTATS_DATA_ROD_v0>();

    // SAFETY: the buffer is exactly the structure the API expects.
    let buffer = unsafe {
        std::slice::from_raw_parts_mut(&mut rod as *mut _ as *mut u8, size)
    };

    let status = unsafe {
        GetPerTcpConnectionEStats(row, TcpConnectionEstatsData, None, 0, None, 0, Some(buffer), 0)
    };

    if status != 0 {
        tracing::debug!(
            "GetPerTcpConnectionEStats failed with 0x{status:08x} for {}:{} -> {}:{}",
            row.dwLocalAddr,
            row.dwLocalPort,
            row.dwRemoteAddr,
            row.dwRemotePort
        );
        return None;
    }

    Some(TcpCounters {
        bytes_sent: rod.DataBytesOut,
        bytes_received: rod.DataBytesIn,
        segments_sent: rod.DataSegsOut,
        segments_received: rod.DataSegsIn,
    })
}

fn read_v6(row: &MIB_TCP6ROW) -> Option<TcpCounters> {
    let mut rod = TCP_ESTATS_DATA_ROD_v0::default();
    let size = std::mem::size_of::<TCP_ESTATS_DATA_ROD_v0>();

    // SAFETY: the buffer is exactly the structure the API expects.
    let buffer = unsafe {
        std::slice::from_raw_parts_mut(&mut rod as *mut _ as *mut u8, size)
    };

    let status = unsafe {
        GetPerTcp6ConnectionEStats(row, TcpConnectionEstatsData, None, 0, None, 0, Some(buffer), 0)
    };

    if status != 0 {
        return None;
    }

    Some(TcpCounters {
        bytes_sent: rod.DataBytesOut,
        bytes_received: rod.DataBytesIn,
        segments_sent: rod.DataSegsOut,
        segments_received: rod.DataSegsIn,
    })
}

/// The state a connection must be in for the statistics API to answer.
pub fn estats_eligible(state: TcpState) -> bool {
    state.supports_estats()
}
