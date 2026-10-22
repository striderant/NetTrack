//! The interface thread: state, event loop and drawing.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Tabs};
use ratatui::{DefaultTerminal, Frame};

use crate::collector::{self, Snapshot};
use crate::config::Settings;
use crate::model::{Connection, ConnState, Proto, RowKind, TcpState};
use crate::tui::filter::Filter;
use crate::tui::tabs;
use crate::tui::theme::{self, Theme};
use crate::tui::widgets::{detail, help, status};

/// How often the interface redraws.
const TICK: Duration = Duration::from_millis(100);

/// The tabs of the interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tab {
    Connections,
    Processes,
    Interfaces,
    Routing,
    Alerts,
}

impl Tab {
    pub const ALL: [Tab; 5] = [
        Tab::Connections,
        Tab::Processes,
        Tab::Interfaces,
        Tab::Routing,
        Tab::Alerts,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Connections => "Connections",
            Tab::Processes => "Processes",
            Tab::Interfaces => "Interfaces",
            Tab::Routing => "Routing",
            Tab::Alerts => "Alerts",
        }
    }
}

/// Which part of the interface has the keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Table,
    Filter,
}

/// Something that happened and needs the interface to react.
#[derive(Debug)]
pub enum AppEvent {
    Key(KeyEvent),
    Resize,
    Tick,
}

/// The application.
pub struct App {
    settings: Settings,
    config_path: PathBuf,
    theme: Theme,
    tab: Tab,
    focus: Focus,
    filter_input: String,
    filter: Filter,
    snapshot: Snapshot,
    rows: Vec<Connection>,
    selected: HashMap<Tab, usize>,
    sort: tabs::connections::Sort,
    paused: bool,
    paused_snapshot: Option<Snapshot>,
    alerts: Vec<tabs::alerts::Alert>,
    message: Option<(String, Instant)>,
    show_help: bool,
    show_detail: bool,
    detail_index: usize,
    rx: crossbeam_channel::Receiver<Snapshot>,
    collector: CollectorHandle,
    hover_row: Option<usize>,
    mouse_enabled: bool,
    last_snapshot_instant: Instant,
    sort_hover: bool,
}

struct CollectorHandle {
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Drop for CollectorHandle {
    fn drop(&mut self) {
        self.stop
            .store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl App {
    pub fn new(settings: Settings, config_path: PathBuf) -> Result<Self> {
        let theme = theme::by_name(&settings.theme);
        let (tx, rx) = collector::channel();
        let (handle, stop) = collector::spawn(settings.clone(), tx);

        Ok(Self {
            rows: Vec::new(),
            snapshot: Snapshot {
                connections: Vec::new(),
                interfaces: Vec::new(),
                routes: Vec::new(),
                neighbours: Vec::new(),
                tcp_stats: Default::default(),
                etw_active: false,
                etw_seen: 0,
                etw_dropped: 0,
                stats_sampled: 0,
                stats_total: 0,
                dns_entries: 0,
                geoip_loaded: false,
                elevated: false,
                error: None,
            },
            settings,
            config_path,
            theme,
            tab: Tab::Connections,
            focus: Focus::Table,
            filter_input: String::new(),
            filter: Filter::default(),
            selected: HashMap::new(),
            sort: tabs::connections::Sort::default(),
            paused: false,
            paused_snapshot: None,
            alerts: Vec::new(),
            message: None,
            show_help: false,
            show_detail: false,
            detail_index: 0,
            rx,
            collector: CollectorHandle {
                stop,
                handle: Some(handle),
            },
            hover_row: None,
            mouse_enabled: false,
            last_snapshot_instant: Instant::now(),
            sort_hover: false,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        let mut terminal = ratatui::init();
        let result = self.run_loop(&mut terminal);
        ratatui::restore();
        result
    }

    fn run_loop(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        self.mouse_enabled = true;

        let mut last_tick = Instant::now();
        let mut last_redraw = Instant::now();

        loop {
            // Drain any snapshot the collector produced.
            while let Ok(snapshot) = self.rx.try_recv() {
                self.apply_snapshot(snapshot);
            }

            // Redraw only if enough time has passed or the snapshot changed.
            let now = Instant::now();
            let snapshot_changed = now.duration_since(self.last_snapshot_instant).as_millis() >= 200;
            if now.duration_since(last_redraw).as_millis() >= 100 || snapshot_changed {
                terminal.draw(|frame| self.draw(frame))?;
                last_redraw = now;
                self.last_snapshot_instant = now;
            }

            let timeout = TICK.saturating_sub(last_tick.elapsed());
            if crossterm::event::poll(timeout)? {
                match crossterm::event::read()? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        if self.handle_key(key)? {
                            break Ok(());
                        }
                    }
                    Event::Mouse(mouse_event) => {
                        self.handle_mouse(mouse_event);
                    }
                    _ => {}
                }
            }

            if last_tick.elapsed() >= TICK {
                last_tick = Instant::now();
            }
        }
    }

    fn apply_snapshot(&mut self, snapshot: Snapshot) {
        if self.paused {
            return;
        }

        tabs::alerts::evaluate(&mut self.alerts, &snapshot, &self.settings);
        self.rebuild_rows();
        self.snapshot = snapshot;
    }

    pub fn take_snapshot(&mut self) -> Option<Snapshot> {
        let snapshot = self.rx.try_recv().ok()?;
        self.apply_snapshot(snapshot);
        Some(self.snapshot.clone())
    }

    pub fn set_tab(&mut self, tab: Tab) {
        self.tab = tab;
    }

    pub fn set_rows(&mut self, rows: Vec<Connection>) {
        self.snapshot.connections = rows.clone();
        self.rows = rows;
    }

    pub fn set_filter(&mut self, expression: &str) {
        self.filter_input = expression.to_string();
        self.filter = Filter::parse(expression);
        self.rebuild_rows();
    }

    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    fn rebuild_rows(&mut self) {
        let mut rows: Vec<Connection> = self
            .snapshot
            .connections
            .iter()
            .filter(|c| self.filter.matches(c))
            .cloned()
            .collect();

        tabs::connections::sort(&mut rows, &self.sort);
        self.rows = rows;

        let limit = self.rows.len();
        let selected = self.selected.entry(self.tab).or_insert(0);
        if limit == 0 {
            *selected = 0;
        } else if *selected >= limit {
            *selected = limit - 1;
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        if self.focus == Focus::Filter {
            match key.code {
                KeyCode::Enter => {
                    self.focus = Focus::Table;
                    self.filter = Filter::parse(&self.filter_input);
                    self.rebuild_rows();
                    return Ok(false);
                }
                KeyCode::Esc => {
                    self.focus = Focus::Table;
                    self.filter_input.clear();
                    self.filter = Filter::default();
                    self.rebuild_rows();
                    return Ok(false);
                }
                KeyCode::Backspace => {
                    self.filter_input.pop();
                    self.filter = Filter::parse(&self.filter_input);
                    self.rebuild_rows();
                    return Ok(false);
                }
                KeyCode::Char(c) => {
                    self.filter_input.push(c);
                    self.filter = Filter::parse(&self.filter_input);
                    self.rebuild_rows();
                    return Ok(false);
                }
                _ => return Ok(false),
            }
        }

        if self.show_help {
            if matches!(key.code, KeyCode::Char('q') | KeyCode::Char('?') | KeyCode::Esc) {
                self.show_help = false;
            }
            return Ok(false);
        }

        if self.show_detail {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => self.show_detail = false,
                KeyCode::Char('j') | KeyCode::Down => {
                    self.detail_index = (self.detail_index + 1).min(self.rows.len().saturating_sub(1));
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.detail_index = self.detail_index.saturating_sub(1);
                }
                _ => {}
            }
            return Ok(false);
        }

        match key.code {
            KeyCode::Char('q') => return Ok(true),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => return Ok(true),
            KeyCode::Char('?') => self.show_help = true,
            KeyCode::Char('/') => {
                self.focus = Focus::Filter;
                self.filter_input = self.filter.raw.clone();
            }
            KeyCode::Char('s') => {
                self.sort = self.sort.next();
                self.rebuild_rows();
            }
            KeyCode::Char('r') => {
                self.message = Some(("refreshed".to_string(), Instant::now()));
            }
            KeyCode::Char(' ') => {
                self.paused = !self.paused;
                if self.paused {
                    self.paused_snapshot = Some(self.snapshot.clone());
                    self.message = Some(("paused".to_string(), Instant::now()));
                } else {
                    self.message = Some(("resumed".to_string(), Instant::now()));
                }
            }
            KeyCode::Tab => {
                self.tab = next_tab(self.tab, false);
            }
            KeyCode::BackTab => {
                self.tab = next_tab(self.tab, true);
            }
            KeyCode::Char(c @ '1'..='5') => {
                self.tab = Tab::ALL[(c as usize - '1' as usize).min(Tab::ALL.len() - 1)];
            }
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
            KeyCode::PageDown | KeyCode::Char('J') => self.move_selection(10),
            KeyCode::PageUp | KeyCode::Char('K') => self.move_selection(-10),
            KeyCode::Home | KeyCode::Char('g') => self.set_selection(0),
            KeyCode::End | KeyCode::Char('G') => {
                let last = self.rows.len().saturating_sub(1);
                self.set_selection(last);
            }
            KeyCode::Enter => {
                if !self.rows.is_empty() {
                    self.detail_index = *self.selected.get(&self.tab).unwrap_or(&0);
                    self.show_detail = true;
                }
            }
            _ => {}
        }

        Ok(false)
    }

    fn move_selection(&mut self, delta: isize) {
        let len = self.rows.len();
        if len == 0 {
            return;
        }
        let current = *self.selected.get(&self.tab).unwrap_or(&0) as isize;
        let next = (current + delta).clamp(0, len as isize - 1);
        self.selected.insert(self.tab, next as usize);
    }

    fn set_selection(&mut self, index: usize) {
        self.selected.insert(self.tab, index);
    }

    fn handle_mouse(&mut self, mouse: MouseEvent) {
        if !self.mouse_enabled {
            return;
        }

        let term_width = 200;
        let term_height = 50;
        let mouse_x = mouse.column;
        let mouse_y = mouse.row;

        // --- TAB AREA: rows 0-2 ---
        // Clicking on a tab switches to that tab
        if mouse_y < 3 && mouse_x < term_width {
            // Estimate tab index from x position
            let tab_width = term_width / Tab::ALL.len() as u16;
            let tab_idx = (mouse_x / tab_width).clamp(0, (Tab::ALL.len() - 1) as u16);
            let new_tab = Tab::ALL[tab_idx as usize];
            if new_tab != self.tab {
                self.tab = new_tab;
                self.selected.insert(self.tab, 0);
            }
            self.hover_row = None;
            self.sort_hover = false;
            return;
        }

        // --- TABLE AREA: rows 3+ ---
        // Hover row highlighting
        if mouse_y >= 3 && mouse_y < 50 {
            let rel_y = mouse_y - 3;
            if rel_y < self.rows.len() as u16 {
                self.hover_row = Some(rel_y as usize);
                // Click to select row
                if matches!(mouse.kind, MouseEventKind::Down(_)) {
                    self.set_selection(self.hover_row.unwrap());
                    // Optionally show detail on click
                    // self.show_detail = true;
                }
            } else {
                self.hover_row = None;
            }
        } else {
            self.hover_row = None;
        }

        // --- SORT HOVER: column headers are in the tab bar area ---
        // If mouse is in the tab area but not over a tab, check for column sort hover
        if mouse_y < 3 && mouse_x < term_width && self.hover_row.is_none() {
            // Check if over a column header area
            // Each column gets roughly 1/10 of the width
            let col_idx = (mouse_x / (term_width / 10)).clamp(0, 9);
            // Only show sort hover for specific columns
            // We'll just set sort_hover state
            self.sort_hover = true;
        }
    }

    /// Draw the whole interface.
    pub fn draw(&self, frame: &mut Frame) {
        let area = frame.area();

        let banner_height = status::elevation_banner_height(!self.snapshot.elevated);
        let banner_chunk = if banner_height > 0 {
            Some(Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(banner_height)])
                .split(area)[0])
        } else {
            None
        };

        let remaining_area = banner_chunk.map_or(area, |b| {
            Rect {
                x: area.x,
                y: area.y + b.height,
                width: area.width,
                height: area.height.saturating_sub(b.height),
            }
        });

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // tabs
                Constraint::Min(3),    // table
                Constraint::Length(3), // footer
            ])
            .split(remaining_area);

        if let Some(b) = banner_chunk {
            status::draw_elevation_banner(frame, b, &self.theme, !self.snapshot.elevated);
        }

        self.draw_tabs(frame, chunks[0]);
        self.draw_body(frame, chunks[1]);
        self.draw_footer(frame, chunks[2]);

        if self.show_help {
            help::draw(frame, area, &self.theme);
        }
        if self.show_detail {
            detail::draw(frame, area, &self.theme, self.rows.get(self.detail_index));
        }
    }

    fn draw_tabs(&self, frame: &mut Frame, area: Rect) {
        let titles: Vec<Line> = Tab::ALL
            .iter()
            .map(|t| Line::from(t.title().to_string()))
            .collect();

        let selected = Tab::ALL.iter().position(|t| *t == self.tab).unwrap_or(0);

        let tabs = Tabs::new(titles)
            .select(selected)
            .block(Block::bordered().title(" NetTrack "))
            .style(Style::default().fg(self.theme.fg))
            .highlight_style(
                Style::default()
                    .fg(self.theme.accent)
                    .add_modifier(Modifier::BOLD),
            )
            .divider(Span::raw("│"));

        frame.render_widget(tabs, area);
    }

    fn draw_body(&self, frame: &mut Frame, area: Rect) {
        match self.tab {
            Tab::Connections => {
                tabs::connections::draw(frame, area, &self.theme, &self.rows, &self.sort, self.selection())
            }
            Tab::Processes => {
                tabs::processes::draw(frame, area, &self.theme, &self.snapshot.connections)
            }
            Tab::Interfaces => {
                tabs::interfaces::draw(frame, area, &self.theme, &self.snapshot.interfaces)
            }
            Tab::Routing => {
                tabs::routing::draw(frame, area, &self.theme, &self.snapshot.routes)
            }
            Tab::Alerts => tabs::alerts::draw(frame, area, &self.theme, &self.alerts),
        }
    }

    fn draw_footer(&self, frame: &mut Frame, area: Rect) {
        status::draw(
            frame,
            area,
            &self.theme,
            &self.filter_input,
            self.focus == Focus::Filter,
            &self.snapshot,
            self.paused,
            self.message.as_ref(),
            self.row_count(),
        );
    }

    fn selection(&self) -> usize {
        *self.selected.get(&self.tab).unwrap_or(&0)
    }
}

fn next_tab(current: Tab, backwards: bool) -> Tab {
    let index = Tab::ALL.iter().position(|t| *t == current).unwrap_or(0);
    if backwards {
        Tab::ALL[(index + Tab::ALL.len() - 1) % Tab::ALL.len()]
    } else {
        Tab::ALL[(index + 1) % Tab::ALL.len()]
    }
}

pub fn is_flow(row: &Connection) -> bool {
    row.kind == RowKind::Flow
}

pub fn is_state(row: &Connection, state: TcpState) -> bool {
    matches!(row.state, ConnState::Tcp(s) if s == state)
}

pub fn is_proto(row: &Connection, proto: Proto) -> bool {
    row.proto == proto
}