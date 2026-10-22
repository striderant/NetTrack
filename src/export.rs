//! Exporting the connection table.
//!
//! The same data the interface shows can be written out as JSON, for
//! scripting, or as CSV, for a spreadsheet.

use std::io::Write;

use serde::Serialize;

use crate::model::{Connection, RowKind};

/// A connection in a form that can be serialised.
#[derive(Debug, Serialize)]
struct ExportRow {
    protocol: &'static str,
    kind: &'static str,
    local_address: String,
    local_port: u16,
    remote_address: Option<String>,
    remote_port: Option<u16>,
    hostname: Option<String>,
    country: Option<String>,
    country_name: Option<String>,
    application_protocol: Option<String>,
    state: String,
    pid: u32,
    process: Option<String>,
    parent: Option<String>,
    executable: Option<String>,
    bytes_sent: u64,
    bytes_received: u64,
    bytes_sent_per_second: u64,
    bytes_received_per_second: u64,
}

fn to_row(conn: &Connection) -> ExportRow {
    ExportRow {
        protocol: conn.proto.label(),
        kind: match conn.kind {
            RowKind::Socket => "socket",
            RowKind::Flow => "flow",
        },
        local_address: conn.local.addr.to_string(),
        local_port: conn.local.port,
        remote_address: conn.remote.map(|r| r.addr.to_string()),
        remote_port: conn.remote.map(|r| r.port),
        hostname: conn.hostname.clone(),
        country: conn.country.as_ref().map(|c| c.iso.clone()),
        country_name: conn.country.as_ref().map(|c| c.name.clone()),
        application_protocol: conn.app_proto.clone(),
        state: conn.state.label().to_string(),
        pid: conn.pid,
        process: conn.process.as_ref().map(|p| p.name.clone()),
        parent: conn.process.as_ref().and_then(|p| p.parent.clone()),
        executable: conn
            .process
            .as_ref()
            .and_then(|p| p.exe.as_ref().map(|e| e.display().to_string())),
        bytes_sent: conn.tx_bytes,
        bytes_received: conn.rx_bytes,
        bytes_sent_per_second: conn.tx_rate,
        bytes_received_per_second: conn.rx_rate,
    }
}

/// Write the connections as JSON.
pub fn write_json<W: Write>(out: &mut W, connections: &[Connection]) -> std::io::Result<()> {
    let rows: Vec<ExportRow> = connections.iter().map(to_row).collect();
    let text = serde_json::to_string_pretty(&rows)
        .map_err(|e| std::io::Error::other(format!("could not serialise: {e}")))?;
    writeln!(out, "{text}")
}

/// Write the connections as CSV.
pub fn write_csv<W: Write>(out: &mut W, connections: &[Connection]) -> std::io::Result<()> {
    writeln!(
        out,
        "protocol,kind,local_address,local_port,remote_address,remote_port,\
         hostname,country,application_protocol,state,pid,process,parent,\
         bytes_sent,bytes_received,bytes_sent_per_second,bytes_received_per_second"
    )?;

    for conn in connections {
        let row = to_row(conn);
        writeln!(
            out,
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            csv_field(row.protocol),
            csv_field(row.kind),
            csv_field(&row.local_address),
            row.local_port,
            csv_field(row.remote_address.as_deref().unwrap_or("")),
            row.remote_port.map(|p| p.to_string()).unwrap_or_default(),
            csv_field(row.hostname.as_deref().unwrap_or("")),
            csv_field(row.country.as_deref().unwrap_or("")),
            csv_field(row.application_protocol.as_deref().unwrap_or("")),
            csv_field(&row.state),
            row.pid,
            csv_field(row.process.as_deref().unwrap_or("")),
            csv_field(row.parent.as_deref().unwrap_or("")),
            row.bytes_sent,
            row.bytes_received,
            row.bytes_sent_per_second,
            row.bytes_received_per_second,
        )?;
    }

    Ok(())
}

/// Quote a value if it contains a comma, a quote or a newline.
fn csv_field(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

/// Write a table for a human to read.
pub fn write_text<W: Write>(out: &mut W, connections: &[Connection]) -> std::io::Result<()> {
    writeln!(
        out,
        "{:<5} {:<22} {:<22} {:<11} {:>7} {:<16} {:>10} {:>10}",
        "PROTO", "LOCAL", "REMOTE", "STATE", "PID", "PROCESS", "UP/s", "DOWN/s"
    )?;

    for conn in connections {
        let remote = conn
            .remote
            .map(|r| r.format())
            .unwrap_or_else(|| "*:*".to_string());
        let process = conn
            .process
            .as_ref()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| format!("PID {}", conn.pid));

        writeln!(
            out,
            "{:<5} {:<22} {:<22} {:<11} {:>7} {:<16} {:>10} {:>10}",
            conn.proto.label(),
            conn.local.format(),
            remote,
            conn.state.label(),
            conn.pid,
            process,
            crate::utils::format::rate(conn.tx_rate),
            crate::utils::format::rate(conn.rx_rate),
        )?;
    }

    Ok(())
}
