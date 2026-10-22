//! Colour themes.

use ratatui::style::Color;

/// The colours used throughout the interface.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub bg: Color,
    pub fg: Color,
    pub dim: Color,
    pub border: Color,
    pub border_focused: Color,
    pub header_bg: Color,
    pub header_fg: Color,
    pub selected_bg: Color,
    pub selected_fg: Color,
    pub established: Color,
    pub listen: Color,
    pub time_wait: Color,
    pub close_wait: Color,
    pub syn_sent: Color,
    pub closed: Color,
    pub tcp: Color,
    pub udp: Color,
    pub flow: Color,
    pub up: Color,
    pub down: Color,
    pub warn: Color,
    pub error: Color,
    pub accent: Color,
    pub banner_bg: Color,
    pub banner_fg: Color,
}

impl Theme {
    /// The colour of a TCP connection state.
    pub fn state_color(&self, state: crate::model::ConnState) -> Color {
        use crate::model::{ConnState, TcpState};
        match state {
            ConnState::UdpBound => self.udp,
            ConnState::UdpFlow => self.flow,
            ConnState::Tcp(s) => match s {
                TcpState::Established => self.established,
                TcpState::Listen => self.listen,
                TcpState::TimeWait => self.time_wait,
                TcpState::CloseWait => self.close_wait,
                TcpState::SynSent | TcpState::SynReceived => self.syn_sent,
                TcpState::FinWait1 | TcpState::FinWait2 | TcpState::Closing | TcpState::LastAck => {
                    self.time_wait
                }
                TcpState::Closed | TcpState::DeleteTcb => self.closed,
                TcpState::Unknown => self.dim,
            },
        }
    }
}

/// Catppuccin Mocha.
pub const MOCHA: Theme = Theme {
    bg: Color::Rgb(30, 30, 46),
    fg: Color::Rgb(205, 214, 244),
    dim: Color::Rgb(108, 112, 134),
    border: Color::Rgb(49, 50, 68),
    border_focused: Color::Rgb(137, 180, 250),
    header_bg: Color::Rgb(24, 24, 37),
    header_fg: Color::Rgb(245, 224, 220),
    selected_bg: Color::Rgb(69, 71, 90),
    selected_fg: Color::Rgb(236, 239, 244),
    established: Color::Rgb(166, 227, 161),
    listen: Color::Rgb(137, 180, 250),
    time_wait: Color::Rgb(249, 226, 175),
    close_wait: Color::Rgb(250, 179, 135),
    syn_sent: Color::Rgb(243, 139, 168),
    closed: Color::Rgb(108, 112, 134),
    tcp: Color::Rgb(137, 180, 250),
    udp: Color::Rgb(166, 227, 161),
    flow: Color::Rgb(203, 166, 247),
    up: Color::Rgb(166, 227, 161),
    down: Color::Rgb(137, 180, 250),
    warn: Color::Rgb(249, 226, 175),
    error: Color::Rgb(243, 139, 168),
    accent: Color::Rgb(203, 166, 247),
    banner_bg: Color::Rgb(88, 91, 112),
    banner_fg: Color::Rgb(236, 239, 244),
};

/// Dracula.
pub const DRACULA: Theme = Theme {
    bg: Color::Rgb(40, 42, 54),
    fg: Color::Rgb(248, 248, 242),
    dim: Color::Rgb(98, 114, 164),
    border: Color::Rgb(68, 71, 90),
    border_focused: Color::Rgb(189, 147, 249),
    header_bg: Color::Rgb(68, 71, 90),
    header_fg: Color::Rgb(248, 248, 242),
    selected_bg: Color::Rgb(68, 71, 90),
    selected_fg: Color::Rgb(255, 255, 255),
    established: Color::Rgb(80, 250, 123),
    listen: Color::Rgb(139, 233, 253),
    time_wait: Color::Rgb(241, 250, 140),
    close_wait: Color::Rgb(255, 184, 108),
    syn_sent: Color::Rgb(255, 85, 85),
    closed: Color::Rgb(98, 114, 164),
    tcp: Color::Rgb(139, 233, 253),
    udp: Color::Rgb(80, 250, 123),
    flow: Color::Rgb(189, 147, 249),
    up: Color::Rgb(80, 250, 123),
    down: Color::Rgb(139, 233, 253),
    warn: Color::Rgb(241, 250, 140),
    error: Color::Rgb(255, 85, 85),
    accent: Color::Rgb(189, 147, 249),
    banner_bg: Color::Rgb(68, 71, 90),
    banner_fg: Color::Rgb(248, 248, 242),
};

/// Nord.
pub const NORD: Theme = Theme {
    bg: Color::Rgb(46, 52, 64),
    fg: Color::Rgb(216, 222, 233),
    dim: Color::Rgb(76, 86, 106),
    border: Color::Rgb(59, 66, 82),
    border_focused: Color::Rgb(136, 192, 208),
    header_bg: Color::Rgb(59, 66, 82),
    header_fg: Color::Rgb(236, 239, 244),
    selected_bg: Color::Rgb(76, 86, 106),
    selected_fg: Color::Rgb(236, 239, 244),
    established: Color::Rgb(163, 190, 140),
    listen: Color::Rgb(136, 192, 208),
    time_wait: Color::Rgb(235, 203, 139),
    close_wait: Color::Rgb(208, 135, 112),
    syn_sent: Color::Rgb(191, 97, 106),
    closed: Color::Rgb(76, 86, 106),
    tcp: Color::Rgb(136, 192, 208),
    udp: Color::Rgb(163, 190, 140),
    flow: Color::Rgb(129, 161, 193),
    up: Color::Rgb(163, 190, 140),
    down: Color::Rgb(136, 192, 208),
    warn: Color::Rgb(235, 203, 139),
    error: Color::Rgb(191, 97, 106),
    accent: Color::Rgb(129, 161, 193),
    banner_bg: Color::Rgb(76, 86, 106),
    banner_fg: Color::Rgb(236, 239, 244),
};

/// A light theme, for terminals with a bright background.
pub const LIGHT: Theme = Theme {
    bg: Color::Rgb(250, 250, 250),
    fg: Color::Rgb(60, 60, 70),
    dim: Color::Rgb(140, 140, 150),
    border: Color::Rgb(200, 200, 210),
    border_focused: Color::Rgb(60, 110, 200),
    header_bg: Color::Rgb(225, 225, 235),
    header_fg: Color::Rgb(30, 30, 40),
    selected_bg: Color::Rgb(210, 220, 240),
    selected_fg: Color::Rgb(20, 20, 30),
    established: Color::Rgb(30, 140, 60),
    listen: Color::Rgb(40, 100, 200),
    time_wait: Color::Rgb(170, 130, 20),
    close_wait: Color::Rgb(190, 110, 30),
    syn_sent: Color::Rgb(200, 50, 60),
    closed: Color::Rgb(140, 140, 150),
    tcp: Color::Rgb(40, 100, 200),
    udp: Color::Rgb(30, 140, 60),
    flow: Color::Rgb(140, 80, 190),
    up: Color::Rgb(30, 140, 60),
    down: Color::Rgb(40, 100, 200),
    warn: Color::Rgb(170, 130, 20),
    error: Color::Rgb(200, 50, 60),
    accent: Color::Rgb(140, 80, 190),
    banner_bg: Color::Rgb(225, 225, 235),
    banner_fg: Color::Rgb(30, 30, 40),
};

/// Look a theme up by name.
pub fn by_name(name: &str) -> Theme {
    match name.to_ascii_lowercase().as_str() {
        "dracula" => DRACULA,
        "nord" => NORD,
        "light" => LIGHT,
        _ => MOCHA,
    }
}

/// The names of the available themes.
pub const NAMES: &[&str] = &["catppuccin-mocha", "dracula", "nord", "light"];
