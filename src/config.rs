//! User configuration, persisted as TOML next to the executable.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Default location of the configuration file: `%APPDATA%\nettrack\config.toml`.
pub fn default_config_path() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("nettrack").join("config.toml")
}

/// All tunables for the monitor.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// How often the connection table is re-read, in milliseconds.
    pub refresh_rate_ms: u64,
    /// How often the Windows DNS cache is re-read, in milliseconds.
    pub dns_refresh_rate_ms: u64,
    /// Show IPv6 endpoints.
    pub show_ipv6: bool,
    /// Resolve remote addresses through the Windows DNS cache.
    pub resolve_hostnames: bool,
    /// Annotate remote addresses with a country using the GeoLite2 database.
    pub geoip_enabled: bool,
    /// Maximum number of connections polled for extended TCP statistics.
    pub stats_max_connections: usize,
    /// Number of worker threads used to poll extended TCP statistics.
    pub stats_workers: usize,
    /// Keep flows visible for this long after the last packet, in seconds.
    pub flow_idle_timeout_secs: u64,
    /// Retain closed connections in the table for this long, in seconds.
    pub keep_closed_secs: u64,
    /// Name of the colour theme to use.
    pub theme: String,
    /// Show the extended statistics columns when available.
    pub show_estats: bool,
    /// Emit an alert when a process starts listening on a non-standard port.
    pub alert_new_listeners: bool,
    /// Emit an alert the first time a process talks to a new country.
    pub alert_new_country: bool,
    /// Emit an alert when a connection attempt fails repeatedly.
    pub alert_conn_fail: bool,
    /// Emit an alert when a process transfers an unusual amount of data.
    pub alert_bandwidth_spike: bool,
    /// Bytes per second above which the bandwidth alert fires.
    pub bandwidth_alert_bps: u64,
    /// Connections per second above which the connection-failure alert fires.
    pub conn_fail_alert_rate: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            refresh_rate_ms: 1_000,
            dns_refresh_rate_ms: 30_000,
            show_ipv6: true,
            resolve_hostnames: true,
            geoip_enabled: true,
            stats_max_connections: 1024,
            stats_workers: 4,
            flow_idle_timeout_secs: 120,
            keep_closed_secs: 30,
            theme: "catppuccin-mocha".to_string(),
            show_estats: true,
            alert_new_listeners: true,
            alert_new_country: true,
            alert_conn_fail: true,
            alert_bandwidth_spike: true,
            bandwidth_alert_bps: 10 * 1024 * 1024,
            conn_fail_alert_rate: 20,
        }
    }
}

impl Settings {
    /// Load the configuration file, falling back to defaults when it is absent
    /// or unreadable.
    pub fn load(path: &Path) -> (Self, Option<String>) {
        match std::fs::read_to_string(path) {
            Ok(text) => match toml::from_str::<Settings>(&text) {
                Ok(settings) => (settings, None),
                Err(e) => (
                    Settings::default(),
                    Some(format!("invalid config at {}: {e}", path.display())),
                ),
            },
            Err(_) => (Settings::default(), None),
        }
    }

    /// Write the configuration back to disk.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, text)
    }
}
