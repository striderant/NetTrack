//! Mapping ports to well-known service names.

/// A well-known port and the service that usually owns it.
pub struct Service {
    pub port: u16,
    pub name: &'static str,
    /// The application protocol this port usually carries.
    pub app_proto: Option<&'static str>,
}

/// The subset of the IANA registry that is worth showing.
pub const SERVICES: &[Service] = &[
    Service { port: 20, name: "ftp-data", app_proto: None },
    Service { port: 21, name: "ftp", app_proto: Some("FTP") },
    Service { port: 22, name: "ssh", app_proto: Some("SSH") },
    Service { port: 23, name: "telnet", app_proto: None },
    Service { port: 25, name: "smtp", app_proto: Some("SMTP") },
    Service { port: 53, name: "domain", app_proto: Some("DNS") },
    Service { port: 67, name: "dhcps", app_proto: Some("DHCP") },
    Service { port: 68, name: "dhcpc", app_proto: Some("DHCP") },
    Service { port: 69, name: "tftp", app_proto: None },
    Service { port: 80, name: "http", app_proto: Some("HTTP") },
    Service { port: 110, name: "pop3", app_proto: None },
    Service { port: 119, name: "nntp", app_proto: None },
    Service { port: 123, name: "ntp", app_proto: Some("NTP") },
    Service { port: 135, name: "epmap", app_proto: None },
    Service { port: 137, name: "netbios-ns", app_proto: Some("NetBIOS") },
    Service { port: 138, name: "netbios-dgm", app_proto: Some("NetBIOS") },
    Service { port: 139, name: "netbios-ssn", app_proto: Some("NetBIOS") },
    Service { port: 143, name: "imap", app_proto: None },
    Service { port: 161, name: "snmp", app_proto: Some("SNMP") },
    Service { port: 162, name: "snmptrap", app_proto: Some("SNMP") },
    Service { port: 389, name: "ldap", app_proto: Some("LDAP") },
    Service { port: 443, name: "https", app_proto: Some("HTTPS") },
    Service { port: 445, name: "microsoft-ds", app_proto: Some("SMB") },
    Service { port: 465, name: "smtps", app_proto: Some("SMTP") },
    Service { port: 514, name: "syslog", app_proto: None },
    Service { port: 587, name: "submission", app_proto: Some("SMTP") },
    Service { port: 631, name: "ipp", app_proto: None },
    Service { port: 636, name: "ldaps", app_proto: Some("LDAPS") },
    Service { port: 993, name: "imaps", app_proto: None },
    Service { port: 995, name: "pop3s", app_proto: None },
    Service { port: 1194, name: "openvpn", app_proto: Some("OpenVPN") },
    Service { port: 1433, name: "ms-sql-s", app_proto: Some("MSSQL") },
    Service { port: 1701, name: "l2tp", app_proto: Some("L2TP") },
    Service { port: 1723, name: "pptp", app_proto: Some("PPTP") },
    Service { port: 1812, name: "radius", app_proto: Some("RADIUS") },
    Service { port: 1900, name: "ssdp", app_proto: Some("SSDP") },
    Service { port: 3128, name: "http-proxy", app_proto: Some("HTTP") },
    Service { port: 3306, name: "mysql", app_proto: Some("MySQL") },
    Service { port: 3389, name: "ms-wbt-server", app_proto: Some("RDP") },
    Service { port: 3478, name: "stun", app_proto: Some("STUN") },
    Service { port: 5060, name: "sip", app_proto: Some("SIP") },
    Service { port: 5061, name: "sips", app_proto: Some("SIP") },
    Service { port: 51820, name: "wireguard", app_proto: Some("WireGuard") },
    Service { port: 5432, name: "postgresql", app_proto: Some("PostgreSQL") },
    Service { port: 5900, name: "vnc", app_proto: Some("VNC") },
    Service { port: 5985, name: "wsman", app_proto: None },
    Service { port: 6379, name: "redis", app_proto: Some("Redis") },
    Service { port: 8000, name: "http-alt", app_proto: Some("HTTP") },
    Service { port: 8080, name: "http-proxy", app_proto: Some("HTTP") },
    Service { port: 8443, name: "https-alt", app_proto: Some("HTTPS") },
    Service { port: 8888, name: "http-alt", app_proto: Some("HTTP") },
    Service { port: 9000, name: "cslistener", app_proto: None },
    Service { port: 9090, name: "zeus-admin", app_proto: None },
    Service { port: 9200, name: "elasticsearch", app_proto: Some("HTTP") },
    Service { port: 9300, name: "elasticsearch", app_proto: None },
    Service { port: 11211, name: "memcached", app_proto: Some("Memcached") },
    Service { port: 27017, name: "mongodb", app_proto: Some("MongoDB") },
];

/// The service name for a port, if it is a well-known one.
pub fn service_name(port: u16) -> Option<&'static str> {
    SERVICES.iter().find(|s| s.port == port).map(|s| s.name)
}

/// The application protocol usually carried on a port.
pub fn app_proto(port: u16) -> Option<&'static str> {
    SERVICES.iter().find(|s| s.port == port).and_then(|s| s.app_proto)
}

/// Guess the application protocol for a connection.
///
/// UDP 443 is QUIC in practice, and UDP 53 carries DNS, so the transport
/// matters as much as the port.
pub fn guess_app_proto(is_udp: bool, port: u16) -> Option<&'static str> {
    if is_udp {
        return match port {
            53 => Some("DNS"),
            443 => Some("QUIC"),
            5353 => Some("mDNS"),
            3478 => Some("STUN"),
            1900 => Some("SSDP"),
            51820 => Some("WireGuard"),
            _ => None,
        };
    }

    match port {
        443 => Some("HTTPS"),
        _ => app_proto(port),
    }
}
