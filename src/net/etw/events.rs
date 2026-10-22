//! Live network events, decoded from ETW records.

use std::net::IpAddr;

use crate::model::{Endpoint, Proto};

/// What happened to a flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetEventKind {
    /// A TCP connection was opened.
    Connect,
    /// A TCP connection was accepted.
    Accept,
    /// A TCP connection was closed.
    Disconnect,
    /// A TCP segment had to be sent again.
    Retransmit,
    /// A connection attempt failed.
    Failure,
    /// Payload was sent.
    Send,
    /// Payload was received.
    Receive,
}

/// A single network event.
#[derive(Debug, Clone)]
pub struct NetEvent {
    pub kind: NetEventKind,
    pub proto: Proto,
    pub pid: u32,
    pub local: Endpoint,
    pub remote: Endpoint,
    /// Size of the payload carried by a send or receive event.
    pub size: u64,
    pub is_ipv6: bool,
}

impl NetEvent {
    /// Whether the event carries a peer, which every flow event does.
    pub fn has_peer(&self) -> bool {
        true
    }
}

/// Build an endpoint from raw address bytes.
pub fn endpoint(addr: [u8; 4], port: u16) -> Endpoint {
    Endpoint::new(IpAddr::from(addr), port)
}

/// Build an endpoint from raw IPv6 address bytes.
pub fn endpoint_v6(addr: [u8; 16], port: u16) -> Endpoint {
    Endpoint::new(IpAddr::from(addr), port)
}
