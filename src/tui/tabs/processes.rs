//! The processes tab: per-process connection and bandwidth totals.

use std::collections::HashMap;

use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Cell, Row, Table, TableState};
use ratatui::Frame;

use crate::model::Connection;
use crate::tui::theme::Theme;
use crate::utils::format;

/// Totals for one process.
#[derive(Debug, Clone)]
struct ProcessTotal {
    pid: u32,
    name: String,
    parent: Option<String>,
    connections: usize,
    listeners: usize,
    tx_rate: u64,
    rx_rate: u64,
    remote_hosts: Vec<String>,
}

/// Draw the processes table.
pub fn draw(frame: &mut Frame, area: Rect, theme: &Theme, connections: &[Connection]) {
    let totals = aggregate(connections);

    let header = Row::new([
        Cell::from("PID"),
        Cell::from("Process"),
        Cell::from("Parent"),
        Cell::from("Conns"),
        Cell::from("Listen"),
        Cell::from("↑ Rate"),
        Cell::from("↓ Rate"),
        Cell::from("Peers"),
    ])
    .style(
        Style::default()
            .fg(theme.header_fg)
            .bg(theme.header_bg)
            .add_modifier(Modifier::BOLD),
    );

    let mut rows: Vec<Row> = totals
        .iter()
        .map(|total| {
            Row::new([
                Cell::from(total.pid.to_string()),
                Cell::from(total.name.clone()),
                Cell::from(total.parent.clone().unwrap_or_else(|| "-".to_string())),
                Cell::from(total.connections.to_string()),
                Cell::from(total.listeners.to_string()),
                Cell::from(Span::styled(
                    format::rate(total.tx_rate),
                    Style::default().fg(theme.up),
                )),
                Cell::from(Span::styled(
                    format::rate(total.rx_rate),
                    Style::default().fg(theme.down),
                )),
                Cell::from(total.remote_hosts.join(", ")),
            ])
        })
        .collect();

    // Biggest talker first.
    rows.sort_by_key(|_| 0);

    let table = Table::new(
        rows,
        [
            Constraint::Length(7),
            Constraint::Min(20),
            Constraint::Min(16),
            Constraint::Length(7),
            Constraint::Length(7),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Min(20),
        ],
    )
    .header(header)
    .block(
        Block::bordered()
            .title(" Processes ")
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

/// Group the connections by process.
fn aggregate(connections: &[Connection]) -> Vec<ProcessTotal> {
    let mut map: HashMap<u32, ProcessTotal> = HashMap::new();

    for conn in connections {
        let entry = map.entry(conn.pid).or_insert_with(|| ProcessTotal {
            pid: conn.pid,
            name: conn
                .process
                .as_ref()
                .map_or_else(|| format!("PID {}", conn.pid), |p| p.name.clone()),
            parent: conn.process.as_ref().and_then(|p| p.parent.clone()),
            connections: 0,
            listeners: 0,
            tx_rate: 0,
            rx_rate: 0,
            remote_hosts: Vec::new(),
        });

        entry.connections += 1;
        entry.tx_rate = entry.tx_rate.saturating_add(conn.tx_rate);
        entry.rx_rate = entry.rx_rate.saturating_add(conn.rx_rate);

        if matches!(conn.state, crate::model::ConnState::Tcp(crate::model::TcpState::Listen)) {
            entry.listeners += 1;
        }

        if let Some(host) = &conn.hostname
            && !entry.remote_hosts.contains(host)
        {
            entry.remote_hosts.push(host.clone());
        } else if let Some(remote) = &conn.remote {
            let text = remote.format();
            if !entry.remote_hosts.contains(&text) {
                entry.remote_hosts.push(text);
            }
        }
    }

    let mut totals: Vec<ProcessTotal> = map.into_values().collect();
    totals.sort_by(|a, b| {
        b.tx_rate
            .saturating_add(b.rx_rate)
            .cmp(&a.tx_rate.saturating_add(a.rx_rate))
            .then_with(|| a.name.cmp(&b.name))
    });
    totals
}
