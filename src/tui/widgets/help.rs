//! The help overlay.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::tui::theme::Theme;

/// Draw the help overlay.
pub fn draw(frame: &mut Frame, area: Rect, theme: &Theme) {
    let width = area.width.saturating_sub(8).min(76);
    let height = area.height.saturating_sub(4).min(30);
    let modal = Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    };

    frame.render_widget(Clear, modal);

    let block = Block::bordered()
        .title(" Help ")
        .border_style(Style::default().fg(theme.accent));

    let key = |k: &'static str| Span::styled(k, Style::default().fg(theme.accent));
    let desc = |d: &'static str| Span::raw(d.to_string());

    let lines = vec![
        Line::from(vec![key("q"), desc("  quit")]),
        Line::from(vec![key("Tab"), desc("  next tab    "), key("1-5"), desc("  jump to tab")]),
        Line::from(vec![key("j/k"), desc("  move down/up    "), key("g/G"), desc("  first/last")]),
        Line::from(vec![key("J/K"), desc("  page down/up")]),
        Line::from(vec![key("Enter"), desc("  connection details")]),
        Line::from(vec![key("/"), desc("  filter    "), key("Esc"), desc("  clear filter")]),
        Line::from(vec![key("s"), desc("  change sort column")]),
        Line::from(vec![key("Space"), desc("  pause and resume")]),
        Line::from(vec![key("r"), desc("  refresh now")]),
        Line::from(vec![key("?"), desc("  this help")]),
        Line::from(""),
        Line::from(Span::styled(
            "Filter language",
            Style::default()
                .fg(theme.warn)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("proto:tcp  state:estab  port:443  process:chrome"),
        Line::from("host:github  country:us  local:192.168.  service:quic"),
        Line::from("flow:true   listen:true   ipv6:true   bare words match anywhere"),
        Line::from(""),
        Line::from(Span::styled(
            "TCP and UDP",
            Style::default()
                .fg(theme.warn)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("TCP rows come from the OS connection table and carry the"),
        Line::from("connection state. UDP rows marked with a triangle are live"),
        Line::from("conversations observed through ETW; plain UDP rows are"),
        Line::from("bound sockets, for which Windows records no peer."),
    ];

    frame.render_widget(
        Paragraph::new(lines).block(block).wrap(Wrap { trim: true }),
        modal,
    );
}
