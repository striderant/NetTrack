//! Reading the OS connection table.
//!
//! On Windows this is `GetExtendedTcpTable` and `GetExtendedUdpTable` with the
//! `OWNER_PID` variants, which return the owning process id for every entry.
//!
//! Note that the UDP table only records the *local* endpoint of a socket:
//! Windows does not keep track of a peer for a connectionless socket, so the
//! remote side of UDP traffic can only be learned from live traffic (ETW) or
//! from packet capture.

use netstat2::{AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo, get_sockets_info};
use netstat2::error::Error;

use crate::model::{ConnState, Endpoint, Proto, TcpState};

/// A single entry of the OS connection table.
#[derive(Debug, Clone)]
pub struct SocketEntry {
    pub proto: Proto,
    pub local: Endpoint,
    /// `None` for UDP, which the OS does not track a peer for.
    pub remote: Option<Endpoint>,
    pub state: ConnState,
    pub pid: u32,
}

/// Read every TCP and UDP socket, for IPv4 and IPv6.
pub fn collect(show_ipv6: bool) -> Result<Vec<SocketEntry>, Error> {
    let af = if show_ipv6 {
        AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6
    } else {
        AddressFamilyFlags::IPV4
    };

    let sockets = get_sockets_info(af, ProtocolFlags::TCP | ProtocolFlags::UDP)?;

    let mut out = Vec::with_capacity(sockets.len());
    for socket in sockets {
        // A socket can be shared by several processes (a forked server, or a
        // socket duplicated across a process tree). The first owner is the one
        // that created it, which is the interesting one.
        let pid = socket.associated_pids.first().copied().unwrap_or(0);

        let entry = match socket.protocol_socket_info {
            ProtocolSocketInfo::Tcp(tcp) => SocketEntry {
                proto: Proto::Tcp,
                local: Endpoint::new(tcp.local_addr, tcp.local_port),
                remote: Some(Endpoint::new(tcp.remote_addr, tcp.remote_port)),
                state: ConnState::Tcp(map_state(tcp.state)),
                pid,
            },
            ProtocolSocketInfo::Udp(udp) => SocketEntry {
                proto: Proto::Udp,
                local: Endpoint::new(udp.local_addr, udp.local_port),
                remote: None,
                state: ConnState::UdpBound,
                pid,
            },
        };
        out.push(entry);
    }

    Ok(out)
}

fn map_state(state: netstat2::TcpState) -> TcpState {
    match state {
        netstat2::TcpState::Closed => TcpState::Closed,
        netstat2::TcpState::Listen => TcpState::Listen,
        netstat2::TcpState::SynSent => TcpState::SynSent,
        netstat2::TcpState::SynReceived => TcpState::SynReceived,
        netstat2::TcpState::Established => TcpState::Established,
        netstat2::TcpState::FinWait1 => TcpState::FinWait1,
        netstat2::TcpState::FinWait2 => TcpState::FinWait2,
        netstat2::TcpState::CloseWait => TcpState::CloseWait,
        netstat2::TcpState::Closing => TcpState::Closing,
        netstat2::TcpState::LastAck => TcpState::LastAck,
        netstat2::TcpState::TimeWait => TcpState::TimeWait,
        netstat2::TcpState::DeleteTcb => TcpState::DeleteTcb,
        netstat2::TcpState::Unknown => TcpState::Unknown,
    }
}
