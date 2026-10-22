//! Country lookup for remote addresses using the MaxMind GeoLite2 database.
//!
//! The database is a build-time artefact (`assets/GeoLite2-Country.mmdb`) but
//! is also looked up next to the executable and in `%LOCALAPPDATA%`, so a
//! portable build can be updated without recompiling.

use std::net::IpAddr;
use std::path::{Path, PathBuf};

use maxminddb::geoip2;
use maxminddb::Reader;

use crate::model::Country as CountryInfo;

/// A country lookup table.
pub struct GeoIp {
    /// The database reader, when the database could be loaded.
    pub reader: Option<Reader<Vec<u8>>>,
    /// Where the database was loaded from, for display.
    pub source: Option<PathBuf>,
    /// Why the database is unavailable.
    pub error: Option<String>,
}

impl GeoIp {
    /// Open the database, searching the usual locations.
    pub fn open() -> Self {
        for path in candidate_paths() {
            if !path.exists() {
                continue;
            }
            match Reader::open_readfile(&path) {
                Ok(reader) => {
                    return Self {
                        reader: Some(reader),
                        source: Some(path),
                        error: None,
                    };
                }
                Err(e) => {
                    let message = format!("could not open {}: {e}", path.display());
                    return Self {
                        reader: None,
                        source: Some(path),
                        error: Some(message),
                    };
                }
            }
        }

        Self {
            reader: None,
            source: None,
            error: Some("GeoLite2 country database not found".to_string()),
        }
    }

    /// Open a database at a specific path.
    pub fn open_at(path: &Path) -> Self {
        match Reader::open_readfile(path) {
            Ok(reader) => Self {
                reader: Some(reader),
                source: Some(path.to_path_buf()),
                error: None,
            },
            Err(e) => Self {
                reader: None,
                source: Some(path.to_path_buf()),
                error: Some(format!("could not open {}: {e}", path.display())),
            },
        }
    }

    /// Whether lookups will succeed.
    pub fn is_loaded(&self) -> bool {
        self.reader.is_some()
    }

    /// Look up the country for an address.
    pub fn lookup(&self, addr: &IpAddr) -> Option<CountryInfo> {
        let reader = self.reader.as_ref()?;
        let result = reader.lookup(*addr).ok()?;
        let data: Option<geoip2::Country> = result.decode().ok()?;
        let country = data?.country;

        let iso = country.iso_code?;
        let name = country
            .names
            .english
            .map(|s| s.to_string())
            .unwrap_or_else(|| iso.to_string());

        Some(CountryInfo {
            iso: iso.to_string(),
            name,
        })
    }
}

/// Where to look for the database, most specific first.
fn candidate_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // Baked in at build time by `build.rs`.
    if let Some(path) = option_env!("NETTRACK_GEOIP_PATH") {
        paths.push(PathBuf::from(path));
    }

    // Next to the executable, for portable deployments.
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        paths.push(dir.join("assets").join("GeoLite2-Country.mmdb"));
        paths.push(dir.join("GeoLite2-Country.mmdb"));
    }

    // Per-user cache, used when the app fetches the database at run time.
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        paths.push(
            PathBuf::from(local)
                .join("nettrack")
                .join("GeoLite2-Country.mmdb"),
        );
    }

    paths
}
