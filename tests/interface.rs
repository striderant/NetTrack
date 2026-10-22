//! Interface tests.
//!
//! The interface is rendered into ratatui's test backend, so the whole
//! drawing path is exercised without needing a terminal.

use std::time::{Duration, Instant};

use ratatui::Terminal;
use ratatui::backend::TestBackend;

use nettrack::config::Settings;
use nettrack::model::{
    ConnState, Connection, Endpoint, ProcessInfo, Proto, RowKind, TcpState,
};
use nettrack::tui::app::{App, Tab};

/// Render the interface and return it as text.
fn render(app: &App, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("a test terminal");
    terminal
        .draw(|frame| app.draw(frame))
        .expect("the interface should draw");
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>()
}

fn sample_connection() -> Connection {
    Connection {
        proto: Proto::Tcp,
        kind: RowKind::Socket,
        local: Endpoint::new("192.168.1.5".parse().unwrap(), 54321),
        remote: Some(Endpoint::new("140.82.121.4".parse().unwrap(), 443)),
        state: ConnState::Tcp(TcpState::Established),
        pid: 4134,
        process: Some(ProcessInfo::new(4134, "firefox.exe")),
        tx_bytes: 12_400,
        rx_bytes: 3100,
        tx_rate: 12_400,
        rx_rate: 3100,
        hostname: Some("github.com".to_string()),
        country: Some(nettrack::model::Country {
            iso: "US".to_string(),
            name: "United States".to_string(),
        }),
        app_proto: Some("HTTPS".to_string()),
        sni: None,
        alpn: None,
        http_host: None,
        first_seen: Instant::now(),
        last_seen: Instant::now(),
    }
}

#[test]
fn the_interface_renders_every_tab() {
    let settings = Settings::default();
    let mut app = App::new(settings, std::env::temp_dir().join("nettrack-test.toml"))
        .expect("the application should start");

    // Give the collector a moment to produce a first snapshot.
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        while app.take_snapshot().is_some() {}
        std::thread::sleep(Duration::from_millis(50));
    }

    for tab in Tab::ALL {
        app.set_tab(tab);
        let text = render(&app, 160, 40);
        assert!(
            !text.trim().is_empty(),
            "the {tab:?} tab should render something"
        );
        assert!(
            text.contains(tab.title()),
            "the {tab:?} tab should show its title"
        );
    }
}

#[test]
fn the_connections_table_shows_the_connection_details() {
    let settings = Settings::default();
    let mut app = App::new(settings, std::env::temp_dir().join("nettrack-test.toml"))
        .expect("the application should start");

    app.set_rows(vec![sample_connection()]);

    let text = render(&app, 200, 30);
    assert!(text.contains("firefox.exe"), "the process name should show");
    assert!(text.contains("192.168.1.5"), "the local address should show");
    assert!(text.contains("140.82.121.4"), "the remote address should show");
    assert!(text.contains("github.com"), "the hostname should show");
    assert!(text.contains("ESTAB"), "the state should show");
    assert!(text.contains("TCP"), "the protocol should show");
}

#[test]
fn the_filter_narrows_the_table() {
    let settings = Settings::default();
    let mut app = App::new(settings, std::env::temp_dir().join("nettrack-test.toml"))
        .expect("the application should start");

    let mut matching = sample_connection();
    let mut other = sample_connection();
    other.pid = 999;
    other.process = Some(ProcessInfo::new(999, "chrome.exe"));
    other.remote = Some(Endpoint::new("1.1.1.1".parse().unwrap(), 53));
    other.hostname = Some("one.one.one.one".to_string());

    app.set_rows(vec![matching.clone(), other]);
    assert_eq!(app.row_count(), 2);

    app.set_filter("process:firefox");
    assert_eq!(app.row_count(), 1);

    app.set_filter("process:chrome");
    assert_eq!(app.row_count(), 1);

    app.set_filter("process:does-not-exist");
    assert_eq!(app.row_count(), 0);

    app.set_filter("");
    assert_eq!(app.row_count(), 2);
}
