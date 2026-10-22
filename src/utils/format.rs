//! Formatting helpers for the table and the status bar.

/// Format a byte count with a binary unit suffix.
pub fn bytes(value: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
    if value < 1024 {
        return format!("{value} B");
    }
    let mut v = value as f64;
    let mut unit = 0;
    while v >= 1024.0 && unit < UNITS.len() - 1 {
        v /= 1024.0;
        unit += 1;
    }
    if v >= 100.0 {
        format!("{:.0} {}", v, UNITS[unit])
    } else if v >= 10.0 {
        format!("{:.1} {}", v, UNITS[unit])
    } else {
        format!("{:.2} {}", v, UNITS[unit])
    }
}

/// Format a byte rate.
pub fn rate(value: u64) -> String {
    if value == 0 {
        return "-".to_string();
    }
    format!("{}/s", bytes(value))
}

/// Format a duration in seconds as a compact human string.
pub fn duration(secs: u64) -> String {
    match secs {
        0..=59 => format!("{secs}s"),
        60..=3599 => format!("{}m{:02}s", secs / 60, secs % 60),
        _ => format!("{}h{:02}m", secs / 3600, (secs % 3600) / 60),
    }
}

/// Truncate a string to `max` characters, appending an ellipsis when cut.
pub fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    if max <= 1 {
        return "…".to_string();
    }
    let mut out: String = text.chars().take(max - 1).collect();
    out.push('…');
    out
}
