//! The payload structures carried by the ETW network providers.
//!
//! These come from `ntwmi.h` and are **byte packed**: the `ConnectionId`
//! field is a pointer-sized value that follows the port fields, which are
//! only 16 bits wide, so natural alignment would insert padding and shift
//! every following field. Getting `repr(packed)` wrong silently produces
//! garbage addresses, so the layouts below are checked by unit tests.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use crate::model::Endpoint;

/// `WMI_UDP_V4` — 28 bytes.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct WmiUdpV4 {
    pub process_id: u32,
    pub transfer_size: u32,
    pub destination_address: [u8; 4],
    pub source_address: [u8; 4],
    pub destination_port: u16,
    pub source_port: u16,
    pub sequence_number: u32,
    pub connection_id: usize,
}

/// `WMI_UDP_V6` — 52 bytes.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct WmiUdpV6 {
    pub process_id: u32,
    pub transfer_size: u32,
    pub destination_address: [u8; 16],
    pub source_address: [u8; 16],
    pub destination_port: u16,
    pub source_port: u16,
    pub sequence_number: u32,
    pub connection_id: usize,
}

/// `WMI_TCPIP_V4` — 28 bytes. Used for receive, disconnect, reconnect,
/// retransmit and copy events.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct WmiTcpIpV4 {
    pub process_id: u32,
    pub transfer_size: u32,
    pub destination_address: [u8; 4],
    pub source_address: [u8; 4],
    pub destination_port: u16,
    pub source_port: u16,
    pub sequence_number: u32,
    pub connection_id: usize,
}

/// `WMI_TCPIP_SEND_V4` — 36 bytes. Send events carry timing fields.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct WmiTcpIpSendV4 {
    pub process_id: u32,
    pub transfer_size: u32,
    pub destination_address: [u8; 4],
    pub source_address: [u8; 4],
    pub destination_port: u16,
    pub source_port: u16,
    pub start_time: u32,
    pub end_time: u32,
    pub sequence_number: u32,
    pub connection_id: usize,
}

/// `WMI_TCPIP_CONNECT_V4` — 44 bytes. Connect and accept events.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct WmiTcpIpConnectV4 {
    pub process_id: u32,
    pub transfer_size: u32,
    pub destination_address: [u8; 4],
    pub source_address: [u8; 4],
    pub destination_port: u16,
    pub source_port: u16,
    pub mss: u16,
    pub sack_option: u16,
    pub timestamp_option: u16,
    pub window_scale_option: u16,
    pub receive_window: u32,
    pub receive_window_scale: i16,
    pub send_window_scale: i16,
    pub sequence_number: u32,
    pub connection_id: usize,
}

/// `WMI_TCPIP_V6` — 52 bytes.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct WmiTcpIpV6 {
    pub process_id: u32,
    pub transfer_size: u32,
    pub destination_address: [u8; 16],
    pub source_address: [u8; 16],
    pub destination_port: u16,
    pub source_port: u16,
    pub sequence_number: u32,
    pub connection_id: usize,
}

/// `WMI_TCPIP_SEND_V6` — 60 bytes.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct WmiTcpIpSendV6 {
    pub process_id: u32,
    pub transfer_size: u32,
    pub destination_address: [u8; 16],
    pub source_address: [u8; 16],
    pub destination_port: u16,
    pub source_port: u16,
    pub start_time: u32,
    pub end_time: u32,
    pub sequence_number: u32,
    pub connection_id: usize,
}

/// `WMI_TCPIP_CONNECT_V6` — 68 bytes.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct WmiTcpIpConnectV6 {
    pub process_id: u32,
    pub transfer_size: u32,
    pub destination_address: [u8; 16],
    pub source_address: [u8; 16],
    pub destination_port: u16,
    pub source_port: u16,
    pub mss: u16,
    pub sack_option: u16,
    pub timestamp_option: u16,
    pub window_scale_option: u16,
    pub receive_window: u32,
    pub receive_window_scale: i16,
    pub send_window_scale: i16,
    pub sequence_number: u32,
    pub connection_id: usize,
}

/// `WMI_TCPIP_FAIL` — 4 bytes.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct WmiTcpIpFail {
    pub protocol: u16,
    pub failure_code: u16,
}

/// The provider guid of the kernel TCP/IP provider.
pub const TCPIP_PROVIDER: windows::core::GUID = windows::core::GUID::from_u128(
    0x9a280ac0_c8e0_11d1_84e2_00c04fb98aa2,
);

/// The provider guid of the kernel UDP/IP provider.
pub const UDPIP_PROVIDER: windows::core::GUID = windows::core::GUID::from_u128(
    0xbf3a50c5_a9c9_4988_a005_2df0b7c80f80,
);

/// The `EVENT_TRACE_TYPE_SEND` opcode.
pub const EVENT_TRACE_TYPE_SEND: u8 = 0x0A;
/// The `EVENT_TRACE_TYPE_RECEIVE` opcode.
pub const EVENT_TRACE_TYPE_RECEIVE: u8 = 0x0B;
/// The `EVENT_TRACE_TYPE_CONNFAIL` opcode.
pub const EVENT_TRACE_TYPE_CONNFAIL: u8 = 0x11;
/// The `EVENT_TRACE_TYPE_CONNECT` opcode.
pub const EVENT_TRACE_TYPE_CONNECT: u8 = 0x12;
/// The `EVENT_TRACE_TYPE_DISCONNECT` opcode.
pub const EVENT_TRACE_TYPE_DISCONNECT: u8 = 0x13;
/// The `EVENT_TRACE_TYPE_RECONNECT` opcode.
pub const EVENT_TRACE_TYPE_RECONNECT: u8 = 0x14;
/// The `EVENT_TRACE_TYPE_RETRANSMIT` opcode.
pub const EVENT_TRACE_TYPE_RETRANSMIT: u8 = 0x15;
/// The `EVENT_TRACE_TYPE_ACCEPT` opcode.
pub const EVENT_TRACE_TYPE_ACCEPT: u8 = 0x16;
/// The `EVENT_TRACE_TYPE_COPY` opcode.
pub const EVENT_TRACE_TYPE_COPY: u8 = 0x17;
/// The IPv6 send opcode.
pub const EVENT_TRACE_TYPE_SEND_IPV6: u8 = 0x1A;
/// The IPv6 receive opcode.
pub const EVENT_TRACE_TYPE_RECEIVE_IPV6: u8 = 0x1B;

/// Read a packed struct out of the raw event payload.
pub fn read_packed<T: Copy>(data: &[u8]) -> Option<T> {
    if data.len() < std::mem::size_of::<T>() {
        return None;
    }
    // SAFETY: the size check above guarantees the buffer is long enough, and
    // every payload struct is plain data with no padding requirements beyond
    // what `repr(packed)` describes.
    let mut value = std::mem::MaybeUninit::<T>::uninit();
    unsafe {
        std::ptr::copy_nonoverlapping(
            data.as_ptr(),
            value.as_mut_ptr() as *mut u8,
            std::mem::size_of::<T>(),
        );
        Some(value.assume_init())
    }
}

/// Build an endpoint from a packed IPv4 address and a network-order port.
pub fn v4_endpoint(addr: [u8; 4], port: u16) -> Endpoint {
    Endpoint::new(IpAddr::V4(Ipv4Addr::from(addr)), port)
}

/// Build an endpoint from a packed IPv6 address and a network-order port.
pub fn v6_endpoint(addr: [u8; 16], port: u16) -> Endpoint {
    Endpoint::new(IpAddr::V6(Ipv6Addr::from(addr)), port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_sizes_match_the_sdk() {
        assert_eq!(std::mem::size_of::<WmiUdpV4>(), 28);
        assert_eq!(std::mem::size_of::<WmiUdpV6>(), 52);
        assert_eq!(std::mem::size_of::<WmiTcpIpV4>(), 28);
        assert_eq!(std::mem::size_of::<WmiTcpIpSendV4>(), 36);
        assert_eq!(std::mem::size_of::<WmiTcpIpConnectV4>(), 44);
        assert_eq!(std::mem::size_of::<WmiTcpIpV6>(), 52);
        assert_eq!(std::mem::size_of::<WmiTcpIpSendV6>(), 60);
        assert_eq!(std::mem::size_of::<WmiTcpIpConnectV6>(), 68);
        assert_eq!(std::mem::size_of::<WmiTcpIpFail>(), 4);
    }

    #[test]
    fn reads_a_packed_udp_record() {
        // A UDP send record: process 1234, 42 bytes, 1.2.3.4 -> 5.6.7.8:53.
        let mut buf = Vec::new();
        buf.extend_from_slice(&1234u32.to_le_bytes());
        buf.extend_from_slice(&42u32.to_le_bytes());
        buf.extend_from_slice(&[5, 6, 7, 8]);
        buf.extend_from_slice(&[1, 2, 3, 4]);
        buf.extend_from_slice(&53u16.to_be_bytes());
        buf.extend_from_slice(&5353u16.to_be_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&0usize.to_le_bytes());

        let record: WmiUdpV4 = read_packed(&buf).expect("record");
        assert_eq!(record.process_id, 1234);
        assert_eq!(record.transfer_size, 42);
        assert_eq!(record.destination_port.to_be(), 53);
        assert_eq!(record.source_port.to_be(), 5353);
        assert_eq!(record.destination_address, [5, 6, 7, 8]);
        assert_eq!(record.source_address, [1, 2, 3, 4]);
    }
}
