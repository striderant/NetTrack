//! The alerts tab.
//!
//! The heuristics are deliberately conservative: they only fire on things
//! that are worth a second look, and each one can be turned off in the
//! configuration file.

use std::collections::HashMap;
use std::time::Instant;

use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Cell, Row, Table, TableState};
use ratatui::Frame;

use crate::collector::Snapshot;
use crate::config::Settings;
use crate::model::{ConnState, Connection, TcpState};
use crate::tui::theme::Theme;

/// How serious an alert is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

impl Severity {
    pub fn label(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Warning => "warn",
            Severity::Critical => "crit",
        }
    }
}

/// A single alert.
#[derive(Debug, Clone)]
pub struct Alert {
    pub severity: Severity,
    pub kind: String,
    pub message: String,
    pub at: Instant,
    /// The identity of the thing that raised the alert, used to avoid
    /// repeating it on every tick.
    pub fingerprint: String,
}

/// Raise any alerts the snapshot warrants.
///
/// The function is called on every snapshot, so it has to be idempotent: an
/// alert is only appended the first time its fingerprint is seen.
pub fn evaluate(alerts: &mut Vec<Alert>, snapshot: &Snapshot, settings: &Settings) {
    let known: HashMap<String, ()> = alerts
        .iter()
        .map(|a| (a.fingerprint.clone(), ()))
        .collect();

    let mut fresh = Vec::new();

    if settings.alert_new_listeners {
        for conn in &snapshot.connections {
            if !is_new_listener(conn) {
                continue;
            }
            let fingerprint = format!("listener:{}:{}", conn.pid, conn.local.port);
            if known.contains_key(&fingerprint) {
                continue;
            }
            fresh.push(Alert {
                severity: Severity::Info,
                kind: "new listener".to_string(),
                message: format!(
                    "{} opened a listening socket on {}:{}",
                    process_name(conn),
                    conn.local.addr,
                    conn.local.port
                ),
                at: Instant::now(),
                fingerprint,
            });
        }
    }

    if settings.alert_new_country {
        for conn in &snapshot.connections {
            let Some(country) = &conn.country else {
                continue;
            };
            // Only consider outbound connections to a remote peer.
            let Some(remote) = &conn.remote else {
                continue;
            };
            if is_local(&remote.addr) {
                continue;
            }
            let fingerprint = format!("country:{}:{}", conn.pid, country.iso);
            if known.contains_key(&fingerprint) {
                continue;
            }
            fresh.push(Alert {
                severity: Severity::Info,
                kind: "new country".to_string(),
                message: format!(
                    "{} reached {} ({})",
                    process_name(conn),
                    country.name,
                    country.iso
                ),
                at: Instant::now(),
                fingerprint,
            });
        }
    }

    if settings.alert_bandwidth_spike {
        for conn in &snapshot.connections {
            let total = conn.total_rate();
            if total < settings.bandwidth_alert_bps {
                continue;
            }
            let fingerprint = format!("bandwidth:{}", conn.pid);
            if known.contains_key(&fingerprint) {
                continue;
            }
            fresh.push(Alert {
                severity: Severity::Warning,
                kind: "bandwidth".to_string(),
                message: format!(
                    "{} is pushing {} ({} / {})",
                    process_name(conn),
                    crate::utils::format::rate(total),
                    conn.remote.map(|r| r.format()).unwrap_or_default(),
                    conn.proto.label()
                ),
                at: Instant::now(),
                fingerprint,
            });
        }
    }

    alerts.extend(fresh);
    alerts.sort_by(|a, b| b.at.cmp(&a.at));

    // Keep the list from growing without bound.
    if alerts.len() > 500 {
        alerts.truncate(500);
    }
}

/// Whether a connection is a listening socket on a non-standard port.
fn is_new_listener(conn: &Connection) -> bool {
    if !matches!(conn.state, ConnState::Tcp(TcpState::Listen)) {
        return false;
    }
    // Only ports above 1024 are interesting; the well-known ones are expected
    // to be bound by the system.
    conn.local.port > 1024
}

/// Whether an address is on the local machine.
fn is_local(addr: &std::net::IpAddr) -> bool {
    match addr {
        std::net::IpAddr::V4(v4) => {
            v4.is_loopback() || v4.is_private() || v4.is_link_local() || v4.is_unspecified()
        }
        std::net::IpAddr::V6(v6) => {
            v6.is_loopback() || v6.is_unspecified() || (v6.segments()[0] & 0xfe00) == 0xfc00
        }
    }
}

fn process_name(conn: &Connection) -> String {
    conn.process
        .as_ref()
        .map_or_else(|| format!("PID {}", conn.pid), |p| p.name.clone())
}

/// Draw the alerts table.
pub fn draw(frame: &mut Frame, area: Rect, theme: &Theme, alerts: &[Alert]) {
    let header = Row::new([
        Cell::from("Time"),
        Cell::from("Severity"),
        Cell::from("Kind"),
        Cell::from("Message"),
    ])
    .style(
        Style::default()
            .fg(theme.header_fg)
            .bg(theme.header_bg)
            .add_modifier(Modifier::BOLD),
    );

    let rows: Vec<Row> = alerts
        .iter()
        .map(|alert| {
            let severity_style = match alert.severity {
                Severity::Info => Style::default().fg(theme.listen),
                Severity::Warning => Style::default().fg(theme.warn),
                Severity::Critical => Style::default().fg(theme.error),
            };

            Row::new([
                Cell::from(elapsed(alert.at)),
                Cell::from(Span::styled(alert.severity.label(), severity_style)),
                Cell::from(alert.kind.clone()),
                Cell::from(alert.message.clone()),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(7),
            Constraint::Length(9),
            Constraint::Length(14),
            Constraint::Min(30),
        ],
    )
    .header(header)
    .block(
        Block::bordered()
            .title(" Alerts ")
            .border_style(Style::default().fg(theme.border)),
    )
    .row_highlight_style(
        Style::default()
            .bg(theme.selected_bg)
            .fg(theme.selected_fg)
            .add_modifier(Modifier::BOLD),
    );

    let mut state = TableState::default();
    frame.render_stateful_widget(table, area, &mut state);
}

/// Format the age of an alert.
fn elapsed(at: Instant) -> String {
    let secs = at.elapsed().as_secs();
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else {
        format!("{}h", secs / 3600)
    }
}
