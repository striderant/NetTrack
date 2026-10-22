//! The routing tab: the IPv4 and IPv6 routing tables.

use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Cell, Row, Table, TableState};
use ratatui::Frame;

use crate::net::system::Route;
use crate::tui::theme::Theme;

/// Draw the routing table.
pub fn draw(frame: &mut Frame, area: Rect, theme: &Theme, routes: &[Route]) {
    let header = Row::new([
        Cell::from("Destination"),
        Cell::from("Prefix"),
        Cell::from("Next hop"),
        Cell::from("Iface"),
        Cell::from("Metric"),
    ])
    .style(
        Style::default()
            .fg(theme.header_fg)
            .bg(theme.header_bg)
            .add_modifier(Modifier::BOLD),
    );

    let rows: Vec<Row> = routes
        .iter()
        .map(|route| {
            Row::new([
                Cell::from(route.destination.to_string()),
                Cell::from(format!("/{}", route.prefix_length)),
                Cell::from(route.next_hop.to_string()),
                Cell::from(route.interface_index.to_string()),
                Cell::from(route.metric.to_string()),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Min(18),
            Constraint::Length(9),
            Constraint::Min(18),
            Constraint::Length(7),
            Constraint::Length(8),
        ],
    )
    .header(header)
    .block(
        Block::bordered()
            .title(" Routing ")
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
