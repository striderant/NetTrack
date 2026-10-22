//! Core data model shared by the collectors and the TUI.
//!
//! Everything the UI renders is ultimately a [`Connection`]. A connection is
//! either
//!
//! * a **socket** read out of the OS connection table (TCP and UDP sockets
//!   always have a local endpoint; only TCP sockets have a remote one), or
//! * a **flow** observed live by the ETW layer, which is the only way to learn
//!   the peer of a connectionless UDP socket.

use std::net::IpAddr;
use std::path::PathBuf;
use std::time::Instant;

/// Transport protocol of a socket or flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Proto {
    Tcp,
    Udp,
}

impl Proto {
    pub fn label(self) -> &'static str {
        match self {
            Proto::Tcp => "TCP",
            Proto::Udp => "UDP",
        }
    }
}

/// Whether a row describes a bound socket or an observed traffic flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RowKind {
    /// A socket taken from the OS connection table.
    Socket,
    /// A traffic flow observed live (UDP peer, or a TCP flow).
    Flow,
}

/// The eleven TCP connection states plus the two transient ones Windows adds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TcpState {
    Closed,
    Listen,
    SynSent,
    SynReceived,
    Established,
    FinWait1,
    FinWait2,
    CloseWait,
    Closing,
    LastAck,
    TimeWait,
    DeleteTcb,
    Unknown,
}

impl TcpState {
    /// Map the raw `MIB_TCP_STATE` value onto a state.
    pub fn from_raw(raw: u32) -> Self {
        match raw {
            1 => TcpState::Closed,
            2 => TcpState::Listen,
            3 => TcpState::SynSent,
            4 => TcpState::SynReceived,
            5 => TcpState::Established,
            6 => TcpState::FinWait1,
            7 => TcpState::FinWait2,
            8 => TcpState::CloseWait,
            9 => TcpState::Closing,
            10 => TcpState::LastAck,
            11 => TcpState::TimeWait,
            12 => TcpState::DeleteTcb,
            _ => TcpState::Unknown,
        }
    }

    /// The raw `MIB_TCP_STATE` value for this state.
    pub fn raw(self) -> u32 {
        match self {
            TcpState::Closed => 1,
            TcpState::Listen => 2,
            TcpState::SynSent => 3,
            TcpState::SynReceived => 4,
            TcpState::Established => 5,
            TcpState::FinWait1 => 6,
            TcpState::FinWait2 => 7,
            TcpState::CloseWait => 8,
            TcpState::Closing => 9,
            TcpState::LastAck => 10,
            TcpState::TimeWait => 11,
            TcpState::DeleteTcb => 12,
            TcpState::Unknown => 0,
        }
    }

    /// Compact uppercase label used in the table.
    pub fn label(self) -> &'static str {
        match self {
            TcpState::Closed => "CLOSED",
            TcpState::Listen => "LISTEN",
            TcpState::SynSent => "SYN_SENT",
            TcpState::SynReceived => "SYN_RCVD",
            TcpState::Established => "ESTAB",
            TcpState::FinWait1 => "FIN_WAIT1",
            TcpState::FinWait2 => "FIN_WAIT2",
            TcpState::CloseWait => "CLOSE_WAIT",
            TcpState::Closing => "CLOSING",
            TcpState::LastAck => "LAST_ACK",
            TcpState::TimeWait => "TIME_WAIT",
            TcpState::DeleteTcb => "DELETE_TCB",
            TcpState::Unknown => "UNKNOWN",
        }
    }

    /// Whether the connection currently carries application data.
    pub fn is_established(self) -> bool {
        matches!(self, TcpState::Established)
    }

    /// Whether the socket is a listening socket.
    pub fn is_listening(self) -> bool {
        matches!(self, TcpState::Listen)
    }

    /// Whether the per-connection extended statistics API can be queried for
    /// a connection in this state. Windows only tracks counters for connections
    /// that have a live four-tuple, so transient states are excluded.
    pub fn supports_estats(self) -> bool {
        matches!(
            self,
            TcpState::Established
                | TcpState::SynSent
                | TcpState::SynReceived
                | TcpState::FinWait1
                | TcpState::FinWait2
                | TcpState::CloseWait
                | TcpState::Closing
                | TcpState::LastAck
        )
    }
}

/// State of a connection, covering both transports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConnState {
    Tcp(TcpState),
    /// A UDP socket with no peer recorded by the OS.
    UdpBound,
    /// A UDP flow observed live by the ETW layer.
    UdpFlow,
}

impl ConnState {
    pub fn label(self) -> &'static str {
        match self {
            ConnState::Tcp(s) => s.label(),
            ConnState::UdpBound => "bound",
            ConnState::UdpFlow => "flow",
        }
    }
}

/// A socket address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Endpoint {
    pub addr: IpAddr,
    pub port: u16,
}

impl Endpoint {
    pub fn new(addr: IpAddr, port: u16) -> Self {
        Self { addr, port }
    }

    /// `addr:port`, bracketing IPv6 literals.
    pub fn format(&self) -> String {
        match self.addr {
            IpAddr::V4(a) => format!("{a}:{}", self.port),
            IpAddr::V6(a) => format!("[{a}]:{}", self.port),
        }
    }
}

/// Identity of the process owning a socket.
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub exe: Option<PathBuf>,
    pub parent: Option<String>,
}

impl ProcessInfo {
    pub fn new(pid: u32, name: impl Into<String>) -> Self {
        Self {
            pid,
            name: name.into(),
            exe: None,
            parent: None,
        }
    }
}

/// GeoIP country for a remote address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Country {
    /// ISO 3166-1 alpha-2 code, e.g. `US`.
    pub iso: String,
    /// English country name.
    pub name: String,
}

impl Country {
    /// The country rendered as a flag emoji, e.g. `🇺🇸`.
    pub fn flag(&self) -> String {
        let mut out = String::with_capacity(8);
        for ch in self.iso.chars() {
            // Regional indicator symbols live at U+1F1E6 for 'A'.
            let c = ch.to_ascii_uppercase();
            if !c.is_ascii_alphabetic() {
                continue;
            }
            let cp = 0x1F1E6 + (c as u32 - 'A' as u32);
            out.push(char::from_u32(cp).unwrap_or('?'));
        }
        out
    }
}

/// A single row in the connections table.
#[derive(Debug, Clone)]
pub struct Connection {
    pub proto: Proto,
    pub kind: RowKind,
    pub local: Endpoint,
    /// The peer. `None` for UDP sockets, which the OS does not track.
    pub remote: Option<Endpoint>,
    pub state: ConnState,
    pub pid: u32,
    pub process: Option<ProcessInfo>,
    /// Cumulative bytes sent / received since the connection was first seen.
    pub tx_bytes: u64,
    pub rx_bytes: u64,
    /// Bytes per second over the last sampling interval.
    pub tx_rate: u64,
    pub rx_rate: u64,
    /// Hostname from the Windows DNS cache, or the TLS SNI when capture is on.
    pub hostname: Option<String>,
    pub country: Option<Country>,
    /// Application protocol guess, e.g. `HTTPS`, `QUIC`, `DNS`.
    pub app_proto: Option<String>,
    /// TLS server name indication, when the npcap feature is enabled.
    pub sni: Option<String>,
    /// TLS application protocol negotiation result.
    pub alpn: Option<String>,
    /// HTTP `Host` header, when the npcap feature is enabled.
    pub http_host: Option<String>,
    pub first_seen: Instant,
    pub last_seen: Instant,
}

impl Connection {
    /// Stable identity used to merge a connection across sampling ticks.
    pub fn key(&self) -> FlowKey {
        FlowKey {
            proto: self.proto,
            local_addr: self.local.addr,
            local_port: self.local.port,
            remote_addr: self.remote.map(|e| e.addr),
            remote_port: self.remote.map(|e| e.port),
        }
    }

    /// Total bytes per second across both directions.
    pub fn total_rate(&self) -> u64 {
        self.tx_rate.saturating_add(self.rx_rate)
    }

    /// Whether this row was derived from live traffic rather than the socket
    /// table.
    pub fn is_flow(&self) -> bool {
        self.kind == RowKind::Flow
    }
}

/// Stable identity of a connection or flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FlowKey {
    pub proto: Proto,
    pub local_addr: IpAddr,
    pub local_port: u16,
    pub remote_addr: Option<IpAddr>,
    pub remote_port: Option<u16>,
}

impl FlowKey {
    /// A key that ignores the peer, used to match ETW traffic against a socket
    /// table entry whose remote endpoint is unknown (UDP).
    pub fn local_only(proto: Proto, addr: IpAddr, port: u16) -> Self {
        Self {
            proto,
            local_addr: addr,
            local_port: port,
            remote_addr: None,
            remote_port: None,
        }
    }
}
