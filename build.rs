use std::env;
use std::fs;
use std::path::PathBuf;

/// Mirrors for the MaxMind GeoLite2 country database.
///
/// The official `maxmind/GeoLite2-Database` release assets now require a
/// (free but registered) MaxMind license key, so the community mirror is
/// tried first. All mirrors serve the same upstream file.
const GEOIP_MIRRORS: &[&str] = &[
    "https://github.com/P3TERX/GeoLite.mmdb/raw/download/GeoLite2-Country.mmdb",
    "https://raw.githubusercontent.com/P3TERX/GeoLite.mmdb/download/GeoLite2-Country.mmdb",
];

const GEOIP_FILE: &str = "GeoLite2-Country.mmdb";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    geoip_database();
}

/// Fetch the GeoLite2 country database into `assets/` so the release binary can
/// resolve remote IPs to a country without any network access at run time.
///
/// Failure is not fatal: the app degrades to "no country column" and will
/// retry the download on first launch.
fn geoip_database() {
    println!("cargo:rerun-if-changed=assets/{}", GEOIP_FILE);

    if env::var_os("NETTRACK_SKIP_GEOIP").is_some() {
        println!("cargo:warning=NETTRACK_SKIP_GEOIP set; skipping GeoLite2 download");
        return;
    }

    let dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let assets = dir.join("assets");
    let target = assets.join(GEOIP_FILE);

    if target.exists() {
        println!(
            "cargo:rustc-env=NETTRACK_GEOIP_PATH={}",
            target.display()
        );
        return;
    }

    if fs::create_dir_all(&assets).is_err() {
        println!("cargo:warning=could not create assets/; GeoLite2 will be fetched at run time");
        return;
    }

    for mirror in GEOIP_MIRRORS {
        match download(mirror) {
            Ok(bytes) if bytes.len() > 100_000 => {
                if fs::write(&target, &bytes).is_ok() {
                    println!("cargo:rustc-env=NETTRACK_GEOIP_PATH={}", target.display());
                    println!("cargo:warning=GeoLite2 country database fetched from {mirror}");
                    return;
                }
                println!("cargo:warning=could not write {}", target.display());
            }
            Ok(_) => println!("cargo:warning={mirror} returned an implausibly small file"),
            Err(e) => println!("cargo:warning=failed to fetch {mirror}: {e}"),
        }
    }

    println!(
        "cargo:warning=GeoLite2 country database unavailable at build time; \
         the app will attempt to download it on first run"
    );
}

fn download(url: &str) -> Result<Vec<u8>, String> {
    // ureq picks its TLS provider explicitly; `native-tls` routes through the
    // Windows certificate store, so no TLS library has to be built.
    let config = ureq::Agent::config_builder()
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .provider(ureq::tls::TlsProvider::NativeTls)
                .build(),
        )
        .build();
    let agent = ureq::Agent::new_with_config(config);

    let mut response = agent.get(url).call().map_err(|e| format!("{e}"))?;

    let mut buf = Vec::new();
    std::io::Read::read_to_end(&mut response.body_mut().as_reader(), &mut buf)
        .map_err(|e| format!("{e}"))?;
    Ok(buf)
}
