//! The background collector.
//!
//! One thread polls the OS connection table, the ETW kernel network provider
//! and the DNS cache, merges them into a single view, and hands that view to
//! the interface thread over a channel. The interface thread never performs
//! a system call itself, so a slow call can never make the interface stutter.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crossbeam_channel::{Sender, bounded};

use crate::admin;
use crate::config::Settings;
use crate::model::{
    ConnState, Connection, Endpoint, FlowKey, Proto, RowKind, TcpState,
};
use crate::net::dns::DnsCache;
use crate::net::estats::EstatsCollector;
use crate::net::etw::{EtwCollector, EtwStatus, NetEvent, NetEventKind};
use crate::net::geoip::GeoIp;
use crate::net::proc::ProcessCache;
use crate::net::sockets::SocketEntry;
use crate::net::system::{self, Interface, Neighbour, Route, TcpStats};
use crate::utils::ports;

/// The upper bound on remembered UDP flows, so a busy server cannot make the
/// table grow without limit.
const MAX_FLOWS: usize = 4096;

/// A view of the whole system, handed to the interface.
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub connections: Vec<Connection>,
    pub interfaces: Vec<Interface>,
    pub routes: Vec<Route>,
    pub neighbours: Vec<Neighbour>,
    pub tcp_stats: TcpStats,
    pub etw_active: bool,
    pub etw_seen: u64,
    pub etw_dropped: u64,
    pub elevated: bool,
    /// How many connections were sampled for extended statistics.
    pub stats_sampled: usize,
    /// How many connections could have been sampled.
    pub stats_total: usize,
    pub dns_entries: usize,
    pub geoip_loaded: bool,
    pub error: Option<String>,
}

/// A flow observed live through ETW.
#[derive(Debug, Clone)]
struct FlowEntry {
    proto: Proto,
    local: Endpoint,
    remote: Endpoint,
    pid: u32,
    tx_bytes: u64,
    rx_bytes: u64,
    first_seen: Instant,
    last_activity: Instant,
    /// The most recent notable event, used for the row marker.
    last_event: NetEventKind,
    /// Number of retransmits seen on this flow.
    retransmits: u32,
}

/// The background collector.
pub struct Collector {
    settings: Settings,
    process_cache: ProcessCache,
    dns_cache: DnsCache,
    geoip: GeoIp,
    estats: EstatsCollector,
    etw: Option<EtwCollector>,
    etw_status: EtwStatus,
    flows: HashMap<FlowKey, FlowEntry>,
    /// Cumulative byte counters from the previous tick, used for rates.
    previous: HashMap<FlowKey, (u64, u64, Instant)>,
    last_dns_refresh: Instant,
    last_system_refresh: Instant,
}

impl Collector {
    /// Build a collector and start the ETW session.
    pub fn new(settings: Settings) -> Self {
        let (etw, etw_status) = EtwCollector::start();
        let etw = Some(etw);

        let geoip = if settings.geoip_enabled {
            GeoIp::open()
        } else {
            GeoIp {
                reader: None,
                source: None,
                error: Some("disabled by configuration".to_string()),
            }
        };

        Self {
            process_cache: ProcessCache::new(),
            dns_cache: DnsCache::new(),
            geoip,
            estats: EstatsCollector::new(settings.stats_workers),
            etw,
            etw_status,
            flows: HashMap::new(),
            previous: HashMap::new(),
            last_dns_refresh: Instant::now(),
            last_system_refresh: Instant::now(),
            settings,
        }
    }

    /// Whether the ETW session is running.
    pub fn etw_status(&self) -> &EtwStatus {
        &self.etw_status
    }

    /// Take a fresh view of the system.
    pub fn collect(&mut self) -> Snapshot {
        let mut error = None;

        // Live traffic first, so the socket table can be merged with it.
        self.drain_etw();

        let socket_entries = match crate::net::collect_sockets(self.settings.show_ipv6) {
            Ok(entries) => entries,
            Err(e) => {
                error = Some(format!("could not read the connection table: {e}"));
                Vec::new()
            }
        };

        self.process_cache.refresh();

        let (counters, sampled, total) = self
            .estats
            .query(&socket_entries, self.settings.stats_max_connections);

        // Refresh the DNS cache on its own, slower schedule.
        if self.last_dns_refresh.elapsed()
            >= Duration::from_millis(self.settings.dns_refresh_rate_ms)
        {
            self.dns_cache.refresh();
            self.last_dns_refresh = Instant::now();
        }

        // Interfaces, routes and neighbours change slowly.
        let (interfaces, routes, neighbours, tcp_stats) =
            if self.last_system_refresh.elapsed() >= Duration::from_secs(2) {
                self.last_system_refresh = Instant::now();
                (
                    system::interfaces().unwrap_or_default(),
                    system::routes().unwrap_or_default(),
                    system::neighbours().unwrap_or_default(),
                    system::tcp_stats().unwrap_or_default(),
                )
            } else {
                (Vec::new(), Vec::new(), Vec::new(), TcpStats::default())
            };

        let connections = self.merge(socket_entries, &counters);
        self.expire_flows();

        Snapshot {
            connections,
            interfaces,
            routes,
            neighbours,
            tcp_stats,
            etw_active: self.etw.as_ref().is_some_and(|e| e.is_active()),
            etw_seen: self.etw.as_ref().map_or(0, |e| e.seen()),
            etw_dropped: self.etw.as_ref().map_or(0, |e| e.dropped()),
            elevated: admin::is_elevated(),
            stats_sampled: sampled,
            stats_total: total,
            dns_entries: self.dns_cache.len(),
            geoip_loaded: self.geoip.is_loaded(),
            error,
        }
    }

    /// Move pending ETW events into the flow table.
    pub fn drain_etw(&mut self) {
        let Some(etw) = &self.etw else {
            return;
        };

        for event in etw.drain() {
            self.apply_event(event);
        }
    }

    /// Fold one ETW event into the flow table.
    fn apply_event(&mut self, event: NetEvent) {
        let key = FlowKey {
            proto: event.proto,
            local_addr: event.local.addr,
            local_port: event.local.port,
            remote_addr: Some(event.remote.addr),
            remote_port: Some(event.remote.port),
        };

        let now = Instant::now();
        let entry = self.flows.entry(key).or_insert_with(|| FlowEntry {
            proto: event.proto,
            local: event.local,
            remote: event.remote,
            pid: event.pid,
            tx_bytes: 0,
            rx_bytes: 0,
            first_seen: now,
            last_activity: now,
            last_event: event.kind,
            retransmits: 0,
        });

        entry.last_activity = now;
        entry.pid = event.pid;
        entry.last_event = event.kind;

        match event.kind {
            NetEventKind::Send => entry.tx_bytes = entry.tx_bytes.saturating_add(event.size),
            NetEventKind::Receive => {
                entry.rx_bytes = entry.rx_bytes.saturating_add(event.size);
            }
            NetEventKind::Retransmit => entry.retransmits = entry.retransmits.saturating_add(1),
            _ => {}
        }
    }

    /// Merge the socket table, the flow table and the byte counters into the
    /// rows the interface shows.
    fn merge(
        &mut self,
        socket_entries: Vec<SocketEntry>,
        counters: &HashMap<FlowKey, crate::net::estats::TcpCounters>,
    ) -> Vec<Connection> {
        let now = Instant::now();
        let mut out: Vec<Connection> = Vec::with_capacity(socket_entries.len());

        // Index the live flows so socket rows can pick up their byte counts.
        // The byte counts are copied out so that no borrow of the flow table
        // is alive while the rate calculation mutates the collector.
        let flow_bytes: HashMap<FlowKey, (u64, u64)> = self
            .flows
            .iter()
            .map(|(k, f)| (*k, (f.tx_bytes, f.rx_bytes)))
            .collect();

        for entry in &socket_entries {
            let key = FlowKey {
                proto: entry.proto,
                local_addr: entry.local.addr,
                local_port: entry.local.port,
                remote_addr: entry.remote.map(|e| e.addr),
                remote_port: entry.remote.map(|e| e.port),
            };

            let process = self.process_cache.get(entry.pid);
            let (tx_bytes, rx_bytes) = match counters.get(&key) {
                Some(c) => (c.bytes_sent, c.bytes_received),
                None => {
                    // Fall back to the ETW byte counters when the extended
                    // statistics are not available for this connection.
                    flow_bytes.get(&key).copied().unwrap_or((0, 0))
                }
            };

            let (tx_rate, rx_rate) = self.rate_for(&key, tx_bytes, rx_bytes, now);

            let mut connection = Connection {
                proto: entry.proto,
                kind: RowKind::Socket,
                local: entry.local,
                remote: entry.remote,
                state: entry.state,
                pid: entry.pid,
                process,
                tx_bytes,
                rx_bytes,
                tx_rate,
                rx_rate,
                hostname: None,
                country: None,
                app_proto: None,
                sni: None,
                alpn: None,
                http_host: None,
                first_seen: now,
                last_seen: now,
            };

            self.annotate(&mut connection);
            out.push(connection);
        }

        // Add the flows the socket table does not know about. These are the
        // UDP conversations, and TCP connections that opened and closed
        // between two polls.
        //
        // The flow table is copied first so that the rate calculation can
        // mutate the collector without a borrow conflict.
        let flows: Vec<(FlowKey, FlowEntry)> = self
            .flows
            .iter()
            .map(|(k, f)| (*k, f.clone()))
            .collect();

        for (key, flow) in &flows {
            let known = socket_entries.iter().any(|entry| {
                let entry_key = FlowKey {
                    proto: entry.proto,
                    local_addr: entry.local.addr,
                    local_port: entry.local.port,
                    remote_addr: entry.remote.map(|e| e.addr),
                    remote_port: entry.remote.map(|e| e.port),
                };
                entry_key == *key
            });
            if known {
                continue;
            }

            // Skip flows that have been idle for too long.
            if flow.last_activity.elapsed()
                > Duration::from_secs(self.settings.flow_idle_timeout_secs)
            {
                continue;
            }

            let (tx_rate, rx_rate) = self.rate_for(key, flow.tx_bytes, flow.rx_bytes, now);
            let mut connection = Connection {
                proto: flow.proto,
                kind: RowKind::Flow,
                local: flow.local,
                remote: Some(flow.remote),
                state: match flow.proto {
                    Proto::Udp => ConnState::UdpFlow,
                    Proto::Tcp => ConnState::Tcp(TcpState::Established),
                },
                pid: flow.pid,
                process: self.process_cache.get(flow.pid),
                tx_bytes: flow.tx_bytes,
                rx_bytes: flow.rx_bytes,
                tx_rate,
                rx_rate,
                hostname: None,
                country: None,
                app_proto: None,
                sni: None,
                alpn: None,
                http_host: None,
                first_seen: flow.first_seen,
                last_seen: flow.last_activity,
            };

            self.annotate(&mut connection);
            out.push(connection);
        }

        out
    }

    /// Compute the byte rate of a connection against the previous tick.
    fn rate_for(&mut self, key: &FlowKey, tx: u64, rx: u64, now: Instant) -> (u64, u64) {
        let (prev_tx, prev_rx, prev_at) = self.previous.get(key).copied().unwrap_or((tx, rx, now));

        // On the first sighting of a connection there is nothing to compare
        // against, so the rate is reported as zero rather than as the whole
        // lifetime total.
        let elapsed = now.duration_since(prev_at).as_secs_f64();
        self.previous.insert(*key, (tx, rx, now));

        if elapsed <= 0.0 {
            return (0, 0);
        }

        let tx_rate = tx.saturating_sub(prev_tx) as f64 / elapsed;
        let rx_rate = rx.saturating_sub(prev_rx) as f64 / elapsed;

        (tx_rate as u64, rx_rate as u64)
    }

    /// Fill in the hostname, the country and the application protocol.
    fn annotate(&self, connection: &mut Connection) {
        let Some(remote) = connection.remote else {
            return;
        };

        if self.settings.resolve_hostnames
            && let Some(name) = self.dns_cache.name(&remote.addr)
        {
            connection.hostname = Some(name.to_string());
        }

        if self.settings.geoip_enabled
            && let Some(country) = self.geoip.lookup(&remote.addr)
        {
            connection.country = Some(country);
        }

        connection.app_proto = ports::guess_app_proto(
            connection.proto == Proto::Udp,
            remote.port,
        )
        .map(|s| s.to_string());
    }

    /// Drop flows that have gone quiet.
    fn expire_flows(&mut self) {
        let timeout = Duration::from_secs(self.settings.flow_idle_timeout_secs);
        self.flows.retain(|_, f| f.last_activity.elapsed() <= timeout);

        if self.flows.len() > MAX_FLOWS {
            // Drop the least recently active flows first.
            let mut entries: Vec<(FlowKey, Instant)> =
                self.flows.iter().map(|(k, f)| (*k, f.last_activity)).collect();
            entries.sort_by_key(|(_, t)| *t);
            for (key, _) in entries.into_iter().take(self.flows.len() - MAX_FLOWS) {
                self.flows.remove(&key);
            }
        }
    }
}

/// Start the collector thread.
pub fn spawn(
    settings: Settings,
    snapshot_tx: Sender<Snapshot>,
) -> (std::thread::JoinHandle<()>, Arc<AtomicBool>) {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_thread = Arc::clone(&stop);

    let handle = std::thread::Builder::new()
        .name("nettrack-collector".to_string())
        .spawn(move || {
            let mut collector = Collector::new(settings);
            let interval = Duration::from_millis(collector.settings.refresh_rate_ms);

            while !stop_thread.load(Ordering::Relaxed) {
                let snapshot = collector.collect();

                // A closed channel means the interface is gone.
                if snapshot_tx.send(snapshot).is_err() {
                    break;
                }

                // Sleep in small slices so a stop request is noticed quickly.
                let mut slept = Duration::ZERO;
                while slept < interval {
                    if stop_thread.load(Ordering::Relaxed) {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(50).min(interval - slept));
                    slept += Duration::from_millis(50);
                }
            }
        })
        .expect("spawn collector thread");

    (handle, stop)
}

/// Create the snapshot channel.
pub fn channel() -> (Sender<Snapshot>, crossbeam_channel::Receiver<Snapshot>) {
    bounded(4)
}
