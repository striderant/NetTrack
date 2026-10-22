//! NetTrack entry point.

use std::io::Write;
use std::time::Duration;

use anyhow::Result;

use nettrack::admin::{self, Elevation};
use nettrack::cli::{self, Args};
use nettrack::collector::{self, Collector};
use nettrack::config::{Settings, default_config_path};
use nettrack::export;
use nettrack::tui::app::App;

fn main() -> Result<()> {
    let args = match cli::parse(std::env::args()) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("nettrack: {message}");
            eprintln!("try `nettrack --help`");
            std::process::exit(2);
        }
    };

    if args.help {
        print!("{}", cli::HELP);
        return Ok(());
    }

    if args.version {
        println!("nettrack {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    init_tracing();

    // The interface has to be DPI aware before anything is drawn.
    admin::set_dpi_awareness();
    admin::set_utf8_codepage();

    let config_path = args
        .config
        .clone()
        .unwrap_or_else(default_config_path);
    let (settings, warning) = Settings::load(&config_path);
    if let Some(warning) = warning {
        eprintln!("nettrack: {warning}");
    }

    // Live UDP peers and cross-user process details need administrator rights.
    if !args.no_elevation {
        match admin::ensure_elevated() {
            Elevation::AlreadyElevated => {}
            Elevation::Relaunched => return Ok(()),
            Elevation::Failed => {
                eprintln!(
                    "nettrack: could not request administrator rights; \
                     live UDP peers and cross-user process details will be unavailable"
                );
            }
        }
    }

    if args.headless || args.json || args.diag_etw {
        return run_headless(settings, &args);
    }

    let mut app = App::new(settings, config_path)?;

    // The terminal is restored before the error is reported, otherwise the
    // message would be printed into the alternate screen and lost.
    let result = app.run();

    if let Err(e) = &result {
        eprintln!("nettrack: {e:#}");
    }
    result
}

/// Print one snapshot and exit.
fn run_headless(settings: Settings, args: &Args) -> Result<()> {
    if args.diag_etw {
        return diag_etw(settings);
    }

    let mut collector = Collector::new(settings);

    // Collect a few samples so the byte rates are not all zero.
    let mut snapshot = collector.collect();
    for _ in 1..args.samples.max(1) {
        std::thread::sleep(Duration::from_millis(1000));
        snapshot = collector.collect();
    }

    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());

    if args.json {
        export::write_json(&mut out, &snapshot.connections)?;
    } else {
        export::write_text(&mut out, &snapshot.connections)?;
    }

    out.flush()?;
    Ok(())
}

/// Diagnose ETW connectivity: start the kernel network provider and report
/// how many events are received.
fn diag_etw(settings: Settings) -> Result<()> {
    use std::time::Instant;

    let mut collector = Collector::new(settings);

    // Wait a few seconds for ETW events to arrive.
    let deadline = Instant::now() + std::time::Duration::from_secs(5);
    while Instant::now() < deadline {
        collector.drain_etw();
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    let snapshot = collector.collect();

    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());

    writeln!(
        out,
        "ETW diagnostic: active={}, seen={}, dropped={}",
        snapshot.etw_active, snapshot.etw_seen, snapshot.etw_dropped
    )?;
    writeln!(
        out,
        "elevated={}, connections={}",
        snapshot.elevated, snapshot.connections.len()
    )?;

    if !snapshot.elevated {
        writeln!(
            out,
            "\nNOTE: Run as Administrator to see UDP peers (YouTube/QUIC) and ETW events."
        )?;
    }

    out.flush()?;
    Ok(())
}

/// Set up logging. Logs go to a file rather than the terminal, because the
/// terminal is taken over by the interface.
fn init_tracing() {
    use tracing_subscriber::EnvFilter;

    let filter = EnvFilter::try_from_env("NETTRACK_LOG")
        .unwrap_or_else(|_| EnvFilter::new("nettrack=info,warn"));

    let path = std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("nettrack")
        .join("nettrack.log");

    if let Some(parent) = path.parent()
        && std::fs::create_dir_all(parent).is_ok()
        && let Ok(file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
    {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(file)
            .with_ansi(false)
            .try_init();
    }
}
