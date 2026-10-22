//! The status bar: filter input on the left, counters on the right.
//!
//! When not elevated, a warning banner is shown because UDP peers (YouTube/QUIC)
//! and cross-user process details require Administrator rights.

use std::time::Instant;

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::collector::Snapshot;
use crate::tui::theme::Theme;
use crate::utils::format;

/// Draw the status bar.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    filter: &str,
    editing_filter: bool,
    snapshot: &Snapshot,
    paused: bool,
    message: Option<&(String, Instant)>,
    row_count: usize,
) {
    let chunks = Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([Constraint::Min(30), Constraint::Length(58)])
        .split(area);

    // The filter line.
    let filter_text = if editing_filter {
        Line::from(vec![
            Span::styled("/", Style::default().fg(theme.accent)),
            Span::raw(filter.to_string()),
            Span::styled("▏", Style::default().fg(theme.accent)),
        ])
    } else if filter.is_empty() {
        Line::from(Span::styled(
            "press / to filter",
            Style::default().fg(theme.dim),
        ))
    } else {
        Line::from(vec![
            Span::styled("filter: ", Style::default().fg(theme.dim)),
            Span::styled(filter.to_string(), Style::default().fg(theme.warn)),
        ])
    };

    let filter_block = Block::bordered().title(" Filter ").border_style(Style::default().fg(
        if editing_filter { theme.accent } else { theme.border },
    ));

    frame.render_widget(Paragraph::new(filter_text).block(filter_block), chunks[0]);

    // The counters.
    let mut tx = 0u64;
    let mut rx = 0u64;
    let mut established = 0usize;
    let mut listeners = 0usize;
    let mut flows = 0usize;

    for conn in &snapshot.connections {
        tx = tx.saturating_add(conn.tx_rate);
        rx = rx.saturating_add(conn.rx_rate);
        match conn.state {
            crate::model::ConnState::Tcp(crate::model::TcpState::Established) => established += 1,
            crate::model::ConnState::Tcp(crate::model::TcpState::Listen) => listeners += 1,
            _ => {}
        }
        if conn.is_flow() {
            flows += 1;
        }
    }

    let mut parts: Vec<Span> = Vec::new();
    parts.push(Span::styled(
        format!("{} rows", row_count),
        Style::default().fg(theme.fg),
    ));
    parts.push(Span::styled(" · ", Style::default().fg(theme.dim)));
    parts.push(Span::styled(
        format!("{established} est"),
        Style::default().fg(theme.established),
    ));
    parts.push(Span::styled(" · ", Style::default().fg(theme.dim)));
    parts.push(Span::styled(
        format!("{listeners} listen"),
        Style::default().fg(theme.listen),
    ));
    if flows > 0 {
        parts.push(Span::styled(" · ", Style::default().fg(theme.dim)));
        parts.push(Span::styled(
            format!("{flows} flows"),
            Style::default().fg(theme.flow),
        ));
    }
    parts.push(Span::styled(" · ", Style::default().fg(theme.dim)));
    parts.push(Span::styled(
        format!("↑{}", format::rate(tx)),
        Style::default().fg(theme.up),
    ));
    parts.push(Span::raw(" "));
    parts.push(Span::styled(
        format!("↓{}", format::rate(rx)),
        Style::default().fg(theme.down),
    ));

    let mut right: Vec<Span> = Vec::new();
    // ETW status with seen/dropped counts.
    let etw_style = if snapshot.etw_active {
        theme.established
    } else if snapshot.elevated {
        theme.warn
    } else {
        theme.dim
    };
    right.push(Span::styled(
        format!(
            "ETW {} ({} seen{} dropped)",
            if snapshot.etw_active { "active" } else { "off" },
            snapshot.etw_seen,
            if snapshot.etw_dropped > 0 {
                format!(", {} dropped", snapshot.etw_dropped)
            } else {
                String::new()
            }
        ),
        etw_style,
    ));
    right.push(Span::styled(" · ", Style::default().fg(theme.dim)));
    right.push(Span::styled(
        format!("stats {}/{}", snapshot.stats_sampled, snapshot.stats_total),
        Style::default().fg(theme.dim),
    ));
    if !snapshot.elevated {
        right.push(Span::styled(" · ", Style::default().fg(theme.dim)));
        right.push(Span::styled(
            "NOT ELEVATED",
            Style::default()
                .fg(theme.error)
                .add_modifier(Modifier::BOLD),
        ));
    }
    if paused {
        right.push(Span::styled(" · ", Style::default().fg(theme.dim)));
        right.push(Span::styled(
            "PAUSED",
            Style::default()
                .fg(theme.warn)
                .add_modifier(Modifier::BOLD),
        ));
    }
    if let Some((text, at)) = message
        && at.elapsed().as_secs() < 3
    {
        right.push(Span::styled(" · ", Style::default().fg(theme.dim)));
        right.push(Span::styled(text.clone(), Style::default().fg(theme.accent)));
    }

    let status_block = Block::bordered()
        .title(" Status ")
        .border_style(Style::default().fg(theme.border));

    let mut lines = vec![Line::from(parts)];
    lines.push(Line::from(right));
    frame.render_widget(Paragraph::new(lines).block(status_block), chunks[1]);
}

/// Draw an elevation warning banner when not running as Administrator.
///
/// Returns the height of the banner (0 if not shown).
pub fn elevation_banner_height(show: bool) -> u16 {
    if show { 1 } else { 0 }
}

/// Render the elevation warning banner line, if not elevated.
pub fn draw_elevation_banner(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    show: bool,
) {
    if !show {
        return;
    }

    let text = "NOT ELEVATED — run as Administrator to see UDP peers (YouTube/QUIC), connect/disconnect events, and per-connection TCP stats";
    let banner = Paragraph::new(Span::styled(
        text,
        Style::default()
            .fg(theme.error)
            .bg(theme.bg)
            .add_modifier(Modifier::BOLD),
    ));
    frame.render_widget(banner, area);
}
