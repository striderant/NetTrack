//! The connection detail modal.

use ratatui::layout::{Constraint, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::model::Connection;
use crate::tui::theme::Theme;
use crate::utils::format;

/// Draw the detail modal for one connection.
pub fn draw(frame: &mut Frame, area: Rect, theme: &Theme, conn: Option<&Connection>) {
    let Some(conn) = conn else {
        return;
    };

    let width = area.width.saturating_sub(8).min(80);
    let height = area.height.saturating_sub(6).min(24);
    let modal = centered(area, width, height);

    frame.render_widget(Clear, modal);

    let block = Block::bordered()
        .title(format!(
            " {} {} ",
            conn.proto.label(),
            conn.state.label()
        ))
        .border_style(Style::default().fg(theme.accent));

    let mut lines: Vec<Line> = Vec::new();

    lines.push(field(theme, "Local", &conn.local.format()));
    lines.push(field(
        theme,
        "Remote",
        &conn
            .remote
            .map(|r| r.format())
            .unwrap_or_else(|| "*:* (no peer recorded)".to_string()),
    ));
    lines.push(field(
        theme,
        "Hostname",
        conn.hostname.as_deref().unwrap_or("-"),
    ));
    lines.push(field(
        theme,
        "Protocol",
        conn.app_proto.as_deref().unwrap_or("-"),
    ));
    lines.push(field(
        theme,
        "Country",
        &match &conn.country {
            Some(c) => format!("{} {} ({})", c.flag(), c.name, c.iso),
            None => "-".to_string(),
        },
    ));
    lines.push(Line::from(""));

    lines.push(field(theme, "PID", &conn.pid.to_string()));
    lines.push(field(
        theme,
        "Process",
        &conn
            .process
            .as_ref()
            .map_or_else(|| "-".to_string(), |p| p.name.clone()),
    ));
    lines.push(field(
        theme,
        "Parent",
        &conn
            .process
            .as_ref()
            .and_then(|p| p.parent.clone())
            .unwrap_or_else(|| "-".to_string()),
    ));
    lines.push(field(
        theme,
        "Executable",
        &conn
            .process
            .as_ref()
            .and_then(|p| p.exe.as_ref().map(|e| e.display().to_string()))
            .unwrap_or_else(|| "-".to_string()),
    ));
    lines.push(Line::from(""));

    lines.push(field(
        theme,
        "Sent",
        &format!(
            "{} total · {} now",
            format::bytes(conn.tx_bytes),
            format::rate(conn.tx_rate)
        ),
    ));
    lines.push(field(
        theme,
        "Received",
        &format!(
            "{} total · {} now",
            format::bytes(conn.rx_bytes),
            format::rate(conn.rx_rate)
        ),
    ));
    lines.push(Line::from(""));

    if let Some(sni) = &conn.sni {
        lines.push(field(theme, "TLS SNI", sni));
    }
    if let Some(alpn) = &conn.alpn {
        lines.push(field(theme, "ALPN", alpn));
    }
    if let Some(host) = &conn.http_host {
        lines.push(field(theme, "HTTP Host", host));
    }
    lines.push(field(
        theme,
        "Seen for",
        &format!("{:?}", conn.first_seen.elapsed()),
    ));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "esc to close",
        Style::default().fg(theme.dim),
    )));

    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: true }),
        modal,
    );
}

fn field(theme: &Theme, name: &str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{name:<12}"), Style::default().fg(theme.dim)),
        Span::raw(value.to_string()),
    ])
}

/// A rectangle of the given size, centred in `area`.
fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

/// A convenience used by the tests.
pub fn detail_height() -> u16 {
    24
}

/// The constraints used when the modal is laid out.
pub fn layout_constraints() -> [Constraint; 2] {
    [Constraint::Percentage(60), Constraint::Percentage(40)]
}
