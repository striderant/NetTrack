//! The connections table: the main view.

use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Cell, Row, Table, TableState};
use ratatui::Frame;

use crate::model::{ConnState, Connection, Proto, RowKind, TcpState};
use crate::tui::theme::Theme;
use crate::utils::format;

/// The column the table is sorted by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortColumn {
    Proto,
    Local,
    Remote,
    State,
    Pid,
    Process,
    Tx,
    Rx,
    Country,
}

impl SortColumn {
    fn title(self) -> &'static str {
        match self {
            SortColumn::Proto => "Proto",
            SortColumn::Local => "Local",
            SortColumn::Remote => "Remote",
            SortColumn::State => "State",
            SortColumn::Pid => "PID",
            SortColumn::Process => "Process",
            SortColumn::Tx => "↑ Rate",
            SortColumn::Rx => "↓ Rate",
            SortColumn::Country => "Geo",
        }
    }
}

/// The sort column and direction.
#[derive(Debug, Clone, Copy)]
pub struct Sort {
    pub column: SortColumn,
    pub ascending: bool,
}

impl Default for Sort {
    fn default() -> Self {
        Self {
            column: SortColumn::Process,
            ascending: true,
        }
    }
}

impl Sort {
    /// Advance to the next column, wrapping around.
    pub fn next(self) -> Self {
        const COLUMNS: [SortColumn; 9] = [
            SortColumn::Proto,
            SortColumn::Local,
            SortColumn::Remote,
            SortColumn::State,
            SortColumn::Pid,
            SortColumn::Process,
            SortColumn::Tx,
            SortColumn::Rx,
            SortColumn::Country,
        ];

        let index = COLUMNS.iter().position(|c| *c == self.column).unwrap_or(0);
        let next = (index + 1) % COLUMNS.len();

        // The first press on a new column sorts ascending, the next flips.
        let ascending = if next == index { !self.ascending } else { true };

        Self {
            column: COLUMNS[next],
            ascending,
        }
    }
}

/// Sort the rows.
pub fn sort(rows: &mut [Connection], sort: &Sort) {
    rows.sort_by(|a, b| {
        let ordering = match sort.column {
            SortColumn::Proto => a.proto.cmp(&b.proto),
            SortColumn::Local => a.local.cmp(&b.local),
            SortColumn::Remote => a.remote.cmp(&b.remote),
            SortColumn::State => a.state.label().cmp(b.state.label()),
            SortColumn::Pid => a.pid.cmp(&b.pid),
            SortColumn::Process => process_name(a).cmp(&process_name(b)),
            SortColumn::Tx => a.tx_rate.cmp(&b.tx_rate),
            SortColumn::Rx => a.rx_rate.cmp(&b.rx_rate),
            SortColumn::Country => country_code(a).cmp(&country_code(b)),
        };

        if sort.ascending { ordering } else { ordering.reverse() }
    });
}

fn process_name(conn: &Connection) -> &str {
    conn.process.as_ref().map_or("", |p| p.name.as_str())
}

fn country_code(conn: &Connection) -> &str {
    conn.country.as_ref().map_or("", |c| c.iso.as_str())
}

/// Draw the connections table.
pub fn draw(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    rows: &[Connection],
    sort: &Sort,
    selected: usize,
) {
    let block = Block::bordered()
        .title(format!(
            " Connections · sorted by {} {} ",
            sort.column.title(),
            if sort.ascending { "▲" } else { "▼" }
        ))
        .border_style(Style::default().fg(theme.border));

    let header = Row::new([
        Cell::from("Proto"),
        Cell::from("Local"),
        Cell::from("Remote"),
        Cell::from("Host"),
        Cell::from("State"),
        Cell::from("PID"),
        Cell::from("Process"),
        Cell::from("↑ Rate"),
        Cell::from("↓ Rate"),
        Cell::from("Geo"),
    ])
    .style(
        Style::default()
            .fg(theme.header_fg)
            .bg(theme.header_bg)
            .add_modifier(Modifier::BOLD),
    );

    let body: Vec<Row> = rows.iter().map(|conn| row_for(conn, theme)).collect();

let table = Table::new(
        body,
        [
            Constraint::Ratio(1, 10),
            Constraint::Ratio(1, 10),
            Constraint::Ratio(1, 10),
            Constraint::Ratio(1, 10),
            Constraint::Ratio(1, 10),
            Constraint::Ratio(1, 10),
            Constraint::Ratio(1, 10),
            Constraint::Ratio(1, 10),
            Constraint::Ratio(1, 10),
            Constraint::Ratio(1, 10),
        ],
    )
    .header(header)
    .block(block)
    .row_highlight_style(
        Style::default()
            .bg(theme.selected_bg)
            .fg(theme.selected_fg)
            .add_modifier(Modifier::BOLD),
    );

    let mut state = TableState::default();
    if !rows.is_empty() {
        state.select(Some(selected.min(rows.len() - 1)));
    }

    frame.render_stateful_widget(table, area, &mut state);
}

/// Build one row.
fn row_for(conn: &Connection, theme: &Theme) -> Row<'static> {
    let proto_style = match conn.proto {
        Proto::Tcp => Style::default().fg(theme.tcp),
        Proto::Udp => Style::default().fg(theme.udp),
    };

    // A flow row is marked so it stands out from the socket rows.
    let proto_text = match conn.kind {
        RowKind::Flow => "▶",
        RowKind::Socket => "",
    };

    let remote_text = match &conn.remote {
        Some(remote) => remote.format(),
        None => "*:*".to_string(),
    };

    let host_text = conn.hostname.clone().unwrap_or_default();
    let process_text = process_name(conn).to_string();

    let geo_text = match &conn.country {
        Some(country) => country.iso.clone(),
        None => String::new(),
    };

    let tx = if conn.tx_rate > 0 {
        format!("{}", format::rate(conn.tx_rate))
    } else {
        "-".to_string()
    };
    let rx = if conn.rx_rate > 0 {
        format!("{}", format::rate(conn.rx_rate))
    } else {
        "-".to_string()
    };

    Row::new([
        Cell::from(Line::from(vec![
            Span::styled(conn.proto.label(), proto_style),
            Span::styled(proto_text, Style::default().fg(theme.flow)),
        ])),
        Cell::from(conn.local.format()),
        Cell::from(remote_text),
        Cell::from(host_text),
        Cell::from(Span::styled(
            conn.state.label(),
            Style::default().fg(theme.state_color(conn.state)),
        )),
        Cell::from(conn.pid.to_string()),
        Cell::from(process_text),
        Cell::from(Span::styled(tx, Style::default().fg(theme.up))),
        Cell::from(Span::styled(rx, Style::default().fg(theme.down))),
        Cell::from(geo_text),
    ])
}

/// The state of a connection, for the detail view.
pub fn describe_state(state: ConnState) -> &'static str {
    match state {
        ConnState::Tcp(TcpState::Established) => "an established connection",
        ConnState::Tcp(TcpState::Listen) => "a listening socket",
        ConnState::Tcp(s) => s.label(),
        ConnState::UdpBound => "a bound UDP socket with no peer recorded",
        ConnState::UdpFlow => "a UDP conversation observed live",
    }
}
