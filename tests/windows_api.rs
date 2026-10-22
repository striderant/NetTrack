//! Integration tests against the real Windows APIs.
//!
//! These need a Windows host, but they do not need administrator rights: the
//! socket table and the process list are readable by any user.

use std::net::IpAddr;

use nettrack::model::{ConnState, Proto, TcpState};
use nettrack::net::proc::ProcessCache;
use nettrack::net::sockets;
use nettrack::utils::format;
use nettrack::utils::ports;

#[test]
fn reads_the_socket_table() {
    let entries = sockets::collect(true).expect("the connection table should be readable");

    // The test process itself is listening on nothing, but the machine has at
    // least a handful of sockets open, so the table cannot be empty.
    assert!(
        !entries.is_empty(),
        "expected at least one socket on the machine"
    );

    for entry in &entries {
        assert!(
            matches!(entry.proto, Proto::Tcp | Proto::Udp),
            "unexpected protocol"
        );
        assert_ne!(entry.local.port, 0, "a socket cannot be bound to port zero");

        match entry.proto {
            Proto::Tcp => {
                // Only TCP sockets carry a peer and a state.
                assert!(entry.remote.is_some(), "a TCP socket has a peer");
                assert!(matches!(entry.state, ConnState::Tcp(_)));
            }
            Proto::Udp => {
                // Windows does not record the peer of a UDP socket, which is
                // exactly why the ETW layer is needed.
                assert!(entry.remote.is_none(), "a UDP socket has no peer");
                assert_eq!(entry.state, ConnState::UdpBound);
            }
        }
    }
}

#[test]
fn resolves_the_current_process() {
    let mut cache = ProcessCache::new();
    let pid = std::process::id();

    let info = cache.get(pid).expect("the current process should be found");
    assert_eq!(info.pid, pid);
    assert!(
        !info.name.is_empty(),
        "the current process should have a name"
    );
}

#[test]
fn unknown_pids_resolve_to_nothing() {
    let cache = ProcessCache::new();
    // Process id zero is the system idle process, which does exist. A pid
    // beyond the configured range does not.
    assert!(cache.get(0x7fff_fff0).is_none());
}

#[test]
fn tcp_states_round_trip_through_the_raw_values() {
    let all = [
        TcpState::Closed,
        TcpState::Listen,
        TcpState::SynSent,
        TcpState::SynReceived,
        TcpState::Established,
        TcpState::FinWait1,
        TcpState::FinWait2,
        TcpState::CloseWait,
        TcpState::Closing,
        TcpState::LastAck,
        TcpState::TimeWait,
        TcpState::DeleteTcb,
    ];

    for state in all {
        assert_eq!(TcpState::from_raw(state.raw()), state);
    }

    assert_eq!(TcpState::from_raw(0), TcpState::Unknown);
    assert_eq!(TcpState::from_raw(99), TcpState::Unknown);
}

#[test]
fn only_live_states_support_extended_statistics() {
    assert!(TcpState::Established.supports_estats());
    assert!(TcpState::CloseWait.supports_estats());

    // A listener has no four-tuple to look up, and a socket in TIME_WAIT is
    // already gone.
    assert!(!TcpState::Listen.supports_estats());
    assert!(!TcpState::TimeWait.supports_estats());
    assert!(!TcpState::Closed.supports_estats());
}

#[test]
fn formats_byte_counts() {
    assert_eq!(format::bytes(0), "0 B");
    assert_eq!(format::bytes(512), "512 B");
    assert_eq!(format::bytes(1024), "1.00 KiB");
    assert_eq!(format::bytes(1024 * 1024), "1.00 MiB");
    assert_eq!(format::bytes(1024 * 1024 * 1024), "1.00 GiB");

    assert_eq!(format::rate(0), "-");
    assert_eq!(format::rate(2048), "2.00 KiB/s");
}

#[test]
fn guesses_the_application_protocol() {
    assert_eq!(ports::guess_app_proto(false, 443), Some("HTTPS"));
    assert_eq!(ports::guess_app_proto(false, 53), Some("DNS"));
    assert_eq!(ports::guess_app_proto(true, 53), Some("DNS"));
    // UDP 443 is QUIC in practice, which the port alone does not say.
    assert_eq!(ports::guess_app_proto(true, 443), Some("QUIC"));
    assert_eq!(ports::guess_app_proto(true, 5353), Some("mDNS"));
    assert_eq!(ports::guess_app_proto(false, 12345), None);
}

#[test]
fn the_dns_cache_parser_handles_the_ipconfig_format() {
    let sample = "\
Windows IP Configuration

Record Name . . . . . . . . . . . : example.com
    Record Type . . . . . . . . . : A (Host)
    A (Host) Record . . . . . . . : 93.184.216.34
    Time To Live  . . . . . . . . : 120
    Length . . . . . . . . . . . . : 28

Record Name . . . . . . . . . . . : other.test
    Record Type . . . . . . . . . : A (Host)
    A (Host) Record . . . . . . . : 203.0.113.7
    Time To Live  . . . . . . . . : 30
";

    let cache = nettrack::net::dns::parse(sample);

    let address: IpAddr = "93.184.216.34".parse().unwrap();
    let entries = cache.get(&address).expect("example.com should be cached");
    assert_eq!(entries[0].name, "example.com");
    // The time to live follows the address it belongs to.
    assert_eq!(entries[0].ttl, 120);

    let other: IpAddr = "203.0.113.7".parse().unwrap();
    assert_eq!(cache.get(&other).unwrap()[0].ttl, 30);
}
