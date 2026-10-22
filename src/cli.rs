//! Command line handling.
//!
//! NetTrack is a terminal interface first, but it can also print a single
//! snapshot and exit, which makes it usable from a script and easy to check
//! from a terminal that is not interactive.

use std::path::PathBuf;

/// The parsed command line.
#[derive(Debug, Clone)]
pub struct Args {
    /// Print one snapshot and exit instead of starting the interface.
    pub headless: bool,
    /// Print the snapshot as JSON.
    pub json: bool,
    /// Do not ask for administrator rights.
    pub no_elevation: bool,
    /// Diagnose ETW connectivity (print ETW events seen).
    pub diag_etw: bool,
    /// Use a different configuration file.
    pub config: Option<PathBuf>,
    /// How many snapshots to collect before printing.
    pub samples: usize,
    /// Print the help text.
    pub help: bool,
    /// Print the version.
    pub version: bool,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            headless: false,
            json: false,
            no_elevation: false,
            diag_etw: false,
            config: None,
            samples: 1,
            help: false,
            version: false,
        }
    }
}

/// Parse the arguments.
pub fn parse<I: IntoIterator<Item = String>>(args: I) -> Result<Args, String> {
    let mut parsed = Args::default();
    let mut iter = args.into_iter().skip(1).peekable();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-h" | "--help" => parsed.help = true,
            "-V" | "--version" => parsed.version = true,
            "-H" | "--headless" => parsed.headless = true,
            "-j" | "--json" => parsed.json = true,
            "--no-elevation" => parsed.no_elevation = true,
            "--diag-etw" => parsed.diag_etw = true,
            "-c" | "--config" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "--config needs a path".to_string())?;
                parsed.config = Some(PathBuf::from(value));
            }
            "-n" | "--samples" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "--samples needs a number".to_string())?;
                parsed.samples = value
                    .parse()
                    .map_err(|_| format!("{value} is not a number of samples"))?;
            }
            other if other.starts_with('-') => {
                return Err(format!("unknown option {other}"));
            }
            // A bare argument is taken as the configuration file.
            other => parsed.config = Some(PathBuf::from(other)),
        }
    }

    Ok(parsed)
}

/// The help text.
pub const HELP: &str = "\
NetTrack - a real-time network monitor for Windows

Usage: nettrack [options]

Options:
  -H, --headless        print one snapshot and exit instead of drawing the
                        interface
  -j, --json            print the snapshot as JSON (implies --headless)
  -n, --samples <n>     collect this many samples before printing
  -c, --config <path>   use this configuration file
      --no-elevation    do not ask for administrator rights
  -h, --help            show this text
  -V, --version         show the version

Keys, in the interface:
  q          quit
  Tab        next tab          1-5  jump to a tab
  j/k        move up and down   J/K   page up and down
  g/G        first and last row
  Enter      connection details
  /          filter            Esc   clear the filter
  s          change the sort column
  Space      pause and resume
  ?          help
";
