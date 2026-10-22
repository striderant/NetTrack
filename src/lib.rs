//! NetTrack — a real-time network monitoring TUI for Windows.
//!
//! NetTrack shows which application is talking to which address and port,
//! over TCP and UDP, with live byte rates, process attribution, hostnames and
//! a country for every remote peer.
//!
//! Three sources feed the view:
//!
//! * the OS connection table (`GetExtendedTcpTable` / `GetExtendedUdpTable`),
//!   which is authoritative for what is open right now,
//! * the ETW kernel network provider, which is the only way to learn the
//!   peer of a connectionless UDP socket and to see connections open and
//!   close between two polls, and
//! * optional packet capture, which adds TLS SNI and HTTP host names.

#[cfg(not(windows))]
compile_error!("NetTrack is a Windows-only tool; it uses the Windows IP Helper and ETW APIs.");

pub mod admin;
pub mod cli;
pub mod collector;
pub mod config;
pub mod export;
pub mod model;
pub mod net;
pub mod tui;
pub mod utils;
