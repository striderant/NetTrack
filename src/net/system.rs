//! System-wide network information: interfaces, routes, neighbours and
//! firewall state.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use windows::Win32::NetworkManagement::IpHelper::{
    FreeMibTable, GetAdaptersAddresses, GetIfTable2, GetIpForwardTable2, GetIpNetTable2,
    GetTcpStatistics, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER,
    GAA_FLAG_SKIP_MULTICAST, IP_ADAPTER_ADDRESSES_LH, MIB_IF_TABLE2, MIB_IPFORWARD_TABLE2,
    MIB_IPNET_TABLE2, MIB_TCPSTATS_LH,
};
use windows::Win32::Networking::WinSock::{
    AF_INET, AF_INET6, AF_UNSPEC, ADDRESS_FAMILY, IN6_ADDR, SOCKADDR, SOCKADDR_INET,
};

/// Statistics for one network interface.
#[derive(Debug, Clone)]
pub struct Interface {
    pub index: u32,
    pub name: String,
    pub description: String,
    /// Link speed in bits per second.
    pub speed: u64,
    pub mtu: u32,
    pub oper_status: u32,
    /// True when the interface is up and carrying traffic.
    pub is_up: bool,
    pub is_loopback: bool,
    pub is_virtual: bool,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_packets: u64,
    pub tx_packets: u64,
    pub rx_errors: u64,
    pub tx_errors: u64,
    pub rx_discards: u64,
    pub tx_discards: u64,
    pub addresses: Vec<IpAddr>,
}

impl Interface {
    /// Whether the interface is a virtual adapter, which is usually noise.
    pub fn is_noise(&self) -> bool {
        self.is_virtual || self.is_loopback
    }
}

/// A row of the routing table.
#[derive(Debug, Clone)]
pub struct Route {
    pub destination: IpAddr,
    pub prefix_length: u8,
    pub next_hop: IpAddr,
    pub interface_index: u32,
    pub metric: u32,
}

/// A row of the neighbour (ARP / NDP) cache.
#[derive(Debug, Clone)]
pub struct Neighbour {
    pub address: IpAddr,
    pub link_layer_address: Vec<u8>,
    pub interface_index: u32,
    pub state: u32,
}

/// Global TCP counters.
#[derive(Debug, Clone, Copy, Default)]
pub struct TcpStats {
    pub active_opens: u32,
    pub passive_opens: u32,
    pub failed_attempts: u32,
    pub resets: u32,
    pub segments_sent: u32,
    pub segments_received: u32,
    pub retransmitted_segments: u32,
}

/// Read the interface table.
pub fn interfaces() -> Result<Vec<Interface>, String> {
    let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
    let status = unsafe { GetIfTable2(&mut table) };
    if status.0 != 0 {
        return Err(format!("GetIfTable2 failed with 0x{:08x}", status.0));
    }
    if table.is_null() {
        return Err("GetIfTable2 returned an empty table".to_string());
    }

    // The table is allocated by the API and has to be released again.
    let guard = MibTableGuard(table as *mut std::ffi::c_void);

    let addresses = adapter_addresses().unwrap_or_default();

    let header = unsafe { &*table };
    let mut out = Vec::with_capacity(header.NumEntries as usize);

    for i in 0..header.NumEntries as usize {
        let Some(row) = header.Table.get(i) else { continue };

        let description = wide_to_string(&row.Description);
        let is_loopback = row.Type == 24; // IF_TYPE_SOFTWARE_LOOPBACK
        let is_virtual = is_virtual_adapter(&description);

        let local_addresses: Vec<IpAddr> = addresses
            .iter()
            .filter(|a| a.index == row.InterfaceIndex)
            .flat_map(|a| a.addresses.iter().copied())
            .collect();

        out.push(Interface {
            index: row.InterfaceIndex,
            name: wide_to_string(&row.Alias),
            description,
            speed: row.TransmitLinkSpeed,
            mtu: row.Mtu,
            oper_status: row.OperStatus.0 as u32,
            is_up: row.OperStatus.0 == 1, // IF_OPER_STATUS_UP
            is_loopback,
            is_virtual,
            rx_bytes: row.InOctets,
            tx_bytes: row.OutOctets,
            rx_packets: row.InUcastPkts + row.InNUcastPkts,
            tx_packets: row.OutUcastPkts + row.OutNUcastPkts,
            rx_errors: row.InErrors,
            tx_errors: row.OutErrors,
            rx_discards: row.InDiscards,
            tx_discards: row.OutDiscards,
            addresses: local_addresses,
        });
    }

    drop(guard);
    Ok(out)
}

/// Read the routing table.
pub fn routes() -> Result<Vec<Route>, String> {
    let mut table: *mut MIB_IPFORWARD_TABLE2 = std::ptr::null_mut();
    let status = unsafe { GetIpForwardTable2(AF_UNSPEC, &mut table) };
    if status.0 != 0 {
        return Err(format!(
            "GetIpForwardTable2 failed with 0x{:08x}",
            status.0
        ));
    }
    if table.is_null() {
        return Err("GetIpForwardTable2 returned an empty table".to_string());
    }

    let guard = MibTableGuard(table as *mut std::ffi::c_void);

    let header = unsafe { &*table };
    let mut out = Vec::with_capacity(header.NumEntries as usize);

    for i in 0..header.NumEntries as usize {
        let Some(row) = header.Table.get(i) else { continue };
        out.push(Route {
            destination: sockaddr_inet_to_ip(&row.DestinationPrefix.Prefix).unwrap_or(IpAddr::from([0, 0, 0, 0])),
            prefix_length: row.DestinationPrefix.PrefixLength,
            next_hop: sockaddr_inet_to_ip(&row.NextHop).unwrap_or(IpAddr::from([0, 0, 0, 0])),
            interface_index: row.InterfaceIndex,
            metric: row.Metric,
        });
    }

    drop(guard);
    Ok(out)
}

/// Read the neighbour cache.
pub fn neighbours() -> Result<Vec<Neighbour>, String> {
    let mut table: *mut MIB_IPNET_TABLE2 = std::ptr::null_mut();
    let status = unsafe { GetIpNetTable2(AF_UNSPEC, &mut table) };
    if status.0 != 0 {
        return Err(format!("GetIpNetTable2 failed with 0x{:08x}", status.0));
    }
    if table.is_null() {
        return Err("GetIpNetTable2 returned an empty table".to_string());
    }

    let guard = MibTableGuard(table as *mut std::ffi::c_void);

    let header = unsafe { &*table };
    let mut out = Vec::with_capacity(header.NumEntries as usize);

    for i in 0..header.NumEntries as usize {
        let Some(row) = header.Table.get(i) else { continue };
        let Some(address) = sockaddr_inet_to_ip(&row.Address) else {
            continue;
        };
        let length = (row.PhysicalAddressLength as usize).min(row.PhysicalAddress.len());
        out.push(Neighbour {
            address,
            link_layer_address: row.PhysicalAddress[..length].to_vec(),
            interface_index: row.InterfaceIndex,
            state: row.State.0 as u32,
        });
    }

    drop(guard);
    Ok(out)
}

/// Read the global TCP counters.
pub fn tcp_stats() -> Result<TcpStats, String> {
    let mut stats = MIB_TCPSTATS_LH::default();
    let status = unsafe { GetTcpStatistics(&mut stats) };
    if status != 0 {
        return Err(format!("GetTcpStatistics failed with 0x{status:08x}"));
    }

    Ok(TcpStats {
        active_opens: stats.dwActiveOpens,
        passive_opens: stats.dwPassiveOpens,
        failed_attempts: stats.dwAttemptFails,
        resets: stats.dwEstabResets,
        segments_sent: stats.dwOutSegs,
        segments_received: stats.dwInSegs,
        retransmitted_segments: stats.dwRetransSegs,
    })
}

/// A minimal view of an adapter's addresses.
pub struct AdapterAddresses {
    pub index: u32,
    pub name: String,
    pub addresses: Vec<IpAddr>,
}

/// Read the addresses of every adapter.
pub fn adapter_addresses() -> Result<Vec<AdapterAddresses>, String> {
    let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER;
    let mut size: u32 = 16 * 1024;
    let mut buffer = vec![0u8; size as usize];

    let status = unsafe {
        GetAdaptersAddresses(
            AF_UNSPEC.0 as u32,
            flags,
            None,
            Some(buffer.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH),
            &mut size,
        )
    };

    if status != 0 {
        return Err(format!(
            "GetAdaptersAddresses failed with 0x{status:08x}"
        ));
    }

    let mut out = Vec::new();
    let mut current = buffer.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH;
    let mut guard = 0;

    while !current.is_null() && guard < 256 {
        guard += 1;
        let adapter = unsafe { &*current };

        let mut addresses = Vec::new();
        let mut unicast = adapter.FirstUnicastAddress;
        let mut uguard = 0;
        while !unicast.is_null() && uguard < 256 {
            uguard += 1;
            let entry = unsafe { &*unicast };
            if let Some(ip) = sockaddr_to_ip(entry.Address.lpSockaddr) {
                addresses.push(ip);
            }
            unicast = entry.Next;
        }

        out.push(AdapterAddresses {
            index: unsafe { adapter.Anonymous1.Anonymous.IfIndex },
            name: unsafe { adapter.FriendlyName.to_string().unwrap_or_default() },
            addresses,
        });

        current = adapter.Next;
    }

    Ok(out)
}

/// Whether an adapter is a virtual one, based on its description.
fn is_virtual_adapter(description: &str) -> bool {
    const MARKERS: &[&str] = &[
        "virtual",
        "hyper-v",
        "vmware",
        "virtualbox",
        "vethernet",
        "loopback",
        "pseudo",
        "tap-",
        "tailscale",
        "zerotier",
        "wireguard",
        "wintun",
        "npcap",
    ];
    let lower = description.to_ascii_lowercase();
    MARKERS.iter().any(|m| lower.contains(m))
}

/// Convert a `SOCKADDR_INET` into an address.
fn sockaddr_inet_to_ip(addr: &SOCKADDR_INET) -> Option<IpAddr> {
    // SAFETY: the union is read according to the family it reports.
    unsafe {
        match addr.si_family {
            AF_INET => {
                let v4 = addr.Ipv4.sin_addr;
                Some(IpAddr::V4(Ipv4Addr::from(v4.S_un.S_addr)))
            }
            AF_INET6 => {
                let v6 = addr.Ipv6.sin6_addr;
                Some(IpAddr::V6(Ipv6Addr::from(in6_bytes(v6))))
            }
            _ => None,
        }
    }
}

/// Convert a `SOCKADDR` into an address.
fn sockaddr_to_ip(addr: *const SOCKADDR) -> Option<IpAddr> {
    if addr.is_null() {
        return None;
    }
    // SAFETY: the pointer comes from the adapter list and is valid.
    unsafe {
        let addr = &*addr;
        match addr.sa_family {
            AF_INET => {
                // The IPv4 address sits in the second half of the data blob.
                let mut bytes = [0u8; 4];
                std::ptr::copy_nonoverlapping(addr.sa_data.as_ptr() as *const u8, bytes.as_mut_ptr(), 4);
                Some(IpAddr::V4(Ipv4Addr::from(bytes)))
            }
            AF_INET6 => {
                let mut bytes = [0u8; 16];
                std::ptr::copy_nonoverlapping(addr.sa_data.as_ptr() as *const u8, bytes.as_mut_ptr(), 16);
                Some(IpAddr::V6(Ipv6Addr::from(bytes)))
            }
            _ => None,
        }
    }
}

/// The sixteen bytes of an IPv6 address.
fn in6_bytes(addr: IN6_ADDR) -> [u8; 16] {
    // SAFETY: `IN6_ADDR` is a plain wrapper around sixteen bytes.
    unsafe {
        let bytes = &*(std::ptr::addr_of!(addr) as *const [u8; 16]);
        *bytes
    }
}

/// Releases a table allocated by one of the IP Helper functions.
struct MibTableGuard(*mut std::ffi::c_void);

impl Drop for MibTableGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                FreeMibTable(self.0);
            }
        }
    }
}

/// Convert a UTF-16 string from the OS into a Rust `String`.
fn wide_to_string(text: &[u16]) -> String {
    let end = text.iter().position(|&c| c == 0).unwrap_or(text.len());
    String::from_utf16_lossy(&text[..end])
}

/// Keep the `ADDRESS_FAMILY` import used.
const _: ADDRESS_FAMILY = AF_UNSPEC;
