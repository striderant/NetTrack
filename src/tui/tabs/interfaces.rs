//! The interfaces tab: per-adapter traffic counters.

use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Cell, Row, Table, TableState};
use ratatui::Frame;

use crate::net::system::Interface;
use crate::tui::theme::Theme;
use crate::utils::format;

/// Draw the interfaces table.
pub fn draw(frame: &mut Frame, area: Rect, theme: &Theme, interfaces: &[Interface]) {
    let header = Row::new([
        Cell::from("Index"),
        Cell::from("Name"),
        Cell::from("Status"),
        Cell::from("Type"),
        Cell::from("Speed"),
        Cell::from("MTU"),
        Cell::from("Rx Bytes"),
        Cell::from("Tx Bytes"),
        Cell::from("Rx Pkt"),
        Cell::from("Tx Pkt"),
        Cell::from("Errors"),
        Cell::from("Addresses"),
    ])
    .style(
        Style::default()
            .fg(theme.header_fg)
            .bg(theme.header_bg)
            .add_modifier(Modifier::BOLD),
    );

    let rows: Vec<Row> = interfaces
        .iter()
        .map(|iface| {
            let status_style = if iface.is_up {
                Style::default().fg(theme.established)
            } else {
                Style::default().fg(theme.closed)
            };

            let kind = if iface.is_loopback {
                "loopback"
            } else if iface.is_virtual {
                "virtual"
            } else {
                "physical"
            };

            Row::new([
                Cell::from(iface.index.to_string()),
                Cell::from(iface.name.clone()),
                Cell::from(Span::styled(
                    if iface.is_up { "up" } else { "down" },
                    status_style,
                )),
                Cell::from(kind),
                Cell::from(format_speed(iface.speed)),
                Cell::from(iface.mtu.to_string()),
                Cell::from(format::bytes(iface.rx_bytes)),
                Cell::from(format::bytes(iface.tx_bytes)),
                Cell::from(iface.rx_packets.to_string()),
                Cell::from(iface.tx_packets.to_string()),
                Cell::from(Span::styled(
                    format!("{}/{}", iface.rx_errors, iface.tx_errors),
                    Style::default().fg(if iface.rx_errors + iface.tx_errors > 0 {
                        theme.warn
                    } else {
                        theme.dim
                    }),
                )),
                Cell::from(
                    iface
                        .addresses
                        .iter()
                        .map(|a| a.to_string())
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(6),
            Constraint::Min(14),
            Constraint::Length(7),
            Constraint::Length(9),
            Constraint::Length(10),
            Constraint::Length(6),
            Constraint::Length(11),
            Constraint::Length(11),
            Constraint::Length(11),
            Constraint::Length(11),
            Constraint::Length(9),
            Constraint::Min(16),
        ],
    )
    .header(header)
    .block(
        Block::bordered()
            .title(" Interfaces ")
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

/// Format a link speed.
fn format_speed(bits_per_second: u64) -> String {
    if bits_per_second == 0 {
        return "-".to_string();
    }
    let mbps = bits_per_second as f64 / 1_000_000.0;
    if mbps >= 1000.0 {
        format!("{:.1} Gb/s", mbps / 1000.0)
    } else {
        format!("{mbps:.0} Mb/s")
    }
}
