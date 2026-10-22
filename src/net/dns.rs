//! Reading the Windows DNS resolver cache.
//!
//! The cache is consulted instead of issuing reverse-DNS queries, so the tool
//! never generates network traffic of its own and never blocks the UI thread
//! on a slow resolver.
//!
//! The cache is read by parsing `ipconfig /displaydns`, which is a stable
//! text format and works on every supported Windows release. The read happens
//! on a background thread because spawning the process costs a few hundred
//! milliseconds.

use std::collections::HashMap;
use std::net::IpAddr;
use std::process::Command;

/// A cached name for an address.
#[derive(Debug, Clone)]
pub struct DnsEntry {
    pub name: String,
    /// Seconds left before the entry expires.
    pub ttl: u32,
}

/// The resolver cache, indexed by address.
#[derive(Debug, Default)]
pub struct DnsCache {
    entries: HashMap<IpAddr, Vec<DnsEntry>>,
    /// Set when the cache could not be read.
    pub error: Option<String>,
}

impl DnsCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Re-read the cache. Failures are recorded rather than propagated, since
    /// hostnames are a nicety and not worth interrupting the monitor for.
    pub fn refresh(&mut self) {
        match read_cache() {
            Ok(entries) => {
                self.entries = entries;
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
    }

    /// The cached names for an address.
    pub fn lookup(&self, addr: &IpAddr) -> &[DnsEntry] {
        self.entries.get(addr).map_or(&[], |v| v.as_slice())
    }

    /// The most likely name for an address.
    pub fn name(&self, addr: &IpAddr) -> Option<&str> {
        self.lookup(addr).first().map(|e| e.name.as_str())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Run `ipconfig /displaydns` and parse the result.
///
/// The output looks like this:
///
/// ```text
/// Record Name . . . . . . . . . . . : github.com
///     Record Type . . . . . . . . . : A (Host)
///     A (Host) Record . . . . . . . : 140.82.121.4
///     Time To Live  . . . . . . . . : 57
/// ```
///
/// A record may list several addresses, and one address may be reached through
/// several names.
fn read_cache() -> Result<HashMap<IpAddr, Vec<DnsEntry>>, String> {
    let output = Command::new("ipconfig")
        .arg("/displaydns")
        .output()
        .map_err(|e| format!("could not run ipconfig: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "ipconfig /displaydns exited with {}",
            output.status
        ));
    }

    let text = String::from_utf8_lossy(&output.stdout);
    Ok(parse(&text))
}

/// Parse the `ipconfig /displaydns` output.
///
/// The output looks like this:
///
/// ```text
/// Record Name . . . . . . . . . . . : github.com
///     Record Type . . . . . . . . . : A (Host)
///     A (Host) Record . . . . . . . : 140.82.121.4
///     Time To Live  . . . . . . . . : 57
/// ```
///
/// A record may list several addresses, and one address may be reached through
/// several names. The time to live follows the address it belongs to, so it
/// is applied to the entry that was read last.
pub fn parse(text: &str) -> HashMap<IpAddr, Vec<DnsEntry>> {
    let mut out: HashMap<IpAddr, Vec<DnsEntry>> = HashMap::new();

    let mut name: Option<String> = None;
    // The address that the most recent time to live belongs to.
    let mut last_address: Option<IpAddr> = None;

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // `Record Name . . . . . . . . . . . : github.com`
        if let Some(value) = field(line, "Record Name") {
            name = Some(value);
            last_address = None;
            continue;
        }

        // `Time To Live  . . . . . . . . : 57`
        if let Some(value) = field(line, "Time To Live") {
            let ttl = value.trim().parse().unwrap_or(0);
            if let (Some(address), Some(name)) = (last_address, name.as_ref())
                && let Some(entries) = out.get_mut(&address)
                && let Some(entry) = entries.iter_mut().find(|e| &e.name == name)
            {
                entry.ttl = ttl;
            }
            continue;
        }

        // `A (Host) Record . . . . . . . : 140.82.121.4`
        let Some(value) = address_record(line) else {
            continue;
        };
        let Some(name) = name.clone() else {
            continue;
        };

        if let Ok(ip) = value.parse::<IpAddr>() {
            out.entry(ip).or_default().push(DnsEntry {
                name,
                ttl: 0,
            });
            last_address = Some(ip);
        }
    }

    out
}

/// Pull the value out of a `Label . . . : value` line.
fn field(line: &str, label: &str) -> Option<String> {
    let rest = line.strip_prefix(label)?.trim_start();
    // The label is followed by a run of `.` padding and then a colon.
    let colon = rest.find(':')?;
    Some(rest[colon + 1..].trim().to_string())
}

/// Extract the address from an address record line.
///
/// Record types that carry an address are `A (Host)`, `AAAA (IPv6)`,
/// `CNAME (Alias)` and `PTR (Name)`. Only the literal-address ones are
/// usable for reverse lookups.
fn address_record(line: &str) -> Option<String> {
    for label in ["A (Host) Record", "AAAA (IPv6) Record", "A Record", "AAAA Record"] {
        if let Some(value) = field(line, label) {
            return Some(value);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
Windows IP Configuration

Record Name . . . . . . . . . . . : github.com
    Record Type . . . . . . . . . : A (Host)
    A (Host) Record . . . . . . . : 140.82.121.4
    Time To Live  . . . . . . . . : 57
    Length . . . . . . . . . . . . : 28

Record Name . . . . . . . . . . . : example.org
    Record Type . . . . . . . . . : A (Host)
    A (Host) Record . . . . . . . : 93.184.216.34
    Time To Live  . . . . . . . . : 120
";

    #[test]
    fn parses_records() {
        let cache = parse(SAMPLE);
        let names = cache
            .get(&"140.82.121.4".parse().unwrap())
            .expect("github.com entry");
        assert_eq!(names[0].name, "github.com");
        assert_eq!(names[0].ttl, 57);
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn ignores_non_address_records() {
        let cache = parse("Record Name . . . . . . . . . . . : x.test\n    Record Type . . . . . . . . . : CNAME (Alias)\n    CNAME (Alias) . . . . . . . . : y.test\n");
        assert!(cache.is_empty());
    }
}
