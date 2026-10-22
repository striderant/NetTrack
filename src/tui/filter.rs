//! The filter bar.
//!
//! Filters use a small, vim-flavoured language:
//!
//! ```text
//! proto:tcp state:estab port:443 process:chrome host:github 192.168.
//! ```
//!
//! A `key:value` term matches the named field, and a bare term matches
//! anywhere in the row. Terms are combined with a logical and.

use crate::model::{ConnState, Connection, Proto, TcpState};

/// One parsed filter term.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Term {
    /// `key:value`
    Field { key: String, value: String },
    /// A bare word, matched against the whole row.
    Free(String),
}

/// A parsed filter.
#[derive(Debug, Clone, Default)]
pub struct Filter {
    pub raw: String,
    pub terms: Vec<Term>,
}

impl Filter {
    /// Parse a filter expression.
    pub fn parse(raw: &str) -> Self {
        let mut terms = Vec::new();
        for token in raw.split_whitespace() {
            // A quoted value keeps its spaces.
            match token.split_once(':') {
                Some((key, value)) => {
                    let key = key.to_ascii_lowercase();
                    let value = value.trim_matches('"').to_string();
                    if key.is_empty() || value.is_empty() {
                        terms.push(Term::Free(token.to_string()));
                    } else {
                        terms.push(Term::Field { key, value });
                    }
                }
                // A bare word matches anywhere in the row.
                None => terms.push(Term::Free(token.to_string())),
            }
        }
        Self {
            raw: raw.to_string(),
            terms,
        }
    }

    /// Whether the filter is empty.
    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    /// Whether a connection passes the filter.
    pub fn matches(&self, conn: &Connection) -> bool {
        self.terms.iter().all(|term| match term {
            Term::Field { key, value } => match_field(conn, key, value),
            Term::Free(text) => free_match(conn, text),
        })
    }
}

/// Match a `key:value` term.
fn match_field(conn: &Connection, key: &str, value: &str) -> bool {
    let value = value.to_ascii_lowercase();

    match key {
        "proto" | "protocol" => conn.proto.label().to_ascii_lowercase().contains(&value),
        "state" => conn.state.label().to_ascii_lowercase().contains(&value),
        "pid" => conn.pid.to_string() == value,
        "process" | "proc" | "app" => conn
            .process
            .as_ref()
            .is_some_and(|p| p.name.to_ascii_lowercase().contains(&value)),
        "port" => {
            let want: u16 = match value.parse() {
                Ok(p) => p,
                Err(_) => return false,
            };
            conn.local.port == want || conn.remote.is_some_and(|r| r.port == want)
        }
        "local" => conn.local.format().to_ascii_lowercase().contains(&value),
        "remote" | "dest" => conn
            .remote
            .is_some_and(|r| r.format().to_ascii_lowercase().contains(&value)),
        "host" | "hostname" => conn
            .hostname
            .as_ref()
            .is_some_and(|h| h.to_ascii_lowercase().contains(&value)),
        "country" => conn
            .country
            .as_ref()
            .is_some_and(|c| c.iso.to_ascii_lowercase().contains(&value)),
        "service" | "app_proto" => conn
            .app_proto
            .as_ref()
            .is_some_and(|a| a.to_ascii_lowercase().contains(&value)),
        "flow" => match value.as_str() {
            "true" | "yes" | "1" => conn.is_flow(),
            "false" | "no" | "0" => !conn.is_flow(),
            _ => false,
        },
        "listen" | "listening" => matches!(conn.state, ConnState::Tcp(TcpState::Listen)),
        "established" | "estab" => {
            matches!(conn.state, ConnState::Tcp(TcpState::Established))
        }
        "ipv6" => conn.local.addr.is_ipv6(),
        "ipv4" => conn.local.addr.is_ipv4(),
        "udp" => conn.proto == Proto::Udp,
        "tcp" => conn.proto == Proto::Tcp,
        _ => false,
    }
}

/// Match a bare term against everything visible in the row.
fn free_match(conn: &Connection, text: &str) -> bool {
    let text = text.to_ascii_lowercase();

    if conn.local.format().to_ascii_lowercase().contains(&text) {
        return true;
    }
    if let Some(remote) = &conn.remote
        && remote.format().to_ascii_lowercase().contains(&text)
    {
        return true;
    }
    if let Some(process) = &conn.process
        && process.name.to_ascii_lowercase().contains(&text)
    {
        return true;
    }
    if let Some(host) = &conn.hostname
        && host.to_ascii_lowercase().contains(&text)
    {
        return true;
    }
    if let Some(country) = &conn.country
        && (country.iso.to_ascii_lowercase().contains(&text)
            || country.name.to_ascii_lowercase().contains(&text))
    {
        return true;
    }
    if let Some(app_proto) = &conn.app_proto
        && app_proto.to_ascii_lowercase().contains(&text)
    {
        return true;
    }

    conn.state.label().to_ascii_lowercase().contains(&text)
        || conn.proto.label().to_ascii_lowercase().contains(&text)
        || conn.pid.to_string() == text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Endpoint, RowKind};
    use std::net::IpAddr;
    use std::time::Instant;

    fn sample() -> Connection {
        Connection {
            proto: Proto::Tcp,
            kind: RowKind::Socket,
            local: Endpoint::new("192.168.1.5".parse::<IpAddr>().unwrap(), 54321),
            remote: Some(Endpoint::new(
                "140.82.121.4".parse::<IpAddr>().unwrap(),
                443,
            )),
            state: ConnState::Tcp(TcpState::Established),
            pid: 4134,
            process: Some(crate::model::ProcessInfo::new(4134, "firefox.exe")),
            tx_bytes: 0,
            rx_bytes: 0,
            tx_rate: 0,
            rx_rate: 0,
            hostname: Some("github.com".to_string()),
            country: None,
            app_proto: Some("HTTPS".to_string()),
            sni: None,
            alpn: None,
            http_host: None,
            first_seen: Instant::now(),
            last_seen: Instant::now(),
        }
    }

    #[test]
    fn matches_fields() {
        let conn = sample();
        assert!(Filter::parse("proto:tcp").matches(&conn));
        assert!(Filter::parse("port:443").matches(&conn));
        assert!(Filter::parse("process:fire").matches(&conn));
        assert!(Filter::parse("host:github").matches(&conn));
        assert!(Filter::parse("state:estab").matches(&conn));
    }

    #[test]
    fn rejects_non_matching() {
        let conn = sample();
        assert!(!Filter::parse("proto:udp").matches(&conn));
        assert!(!Filter::parse("port:80").matches(&conn));
        assert!(!Filter::parse("process:chrome").matches(&conn));
    }

    #[test]
    fn bare_terms_match_anywhere() {
        let conn = sample();
        assert!(Filter::parse("github").matches(&conn));
        assert!(Filter::parse("firefox").matches(&conn));
        assert!(!Filter::parse("nonsense").matches(&conn));
    }

    #[test]
    fn terms_are_combined_with_and() {
        let conn = sample();
        assert!(Filter::parse("proto:tcp port:443").matches(&conn));
        assert!(!Filter::parse("proto:tcp port:80").matches(&conn));
    }
}
