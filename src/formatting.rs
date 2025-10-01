use chrono::{DateTime, Local};
use std::time::SystemTime;

pub fn format_optional_time(time: Option<SystemTime>) -> String {
    match time {
        Some(value) => format_system_time(value),
        None => "No disponible".to_string(),
    }
}

pub fn format_system_time(time: SystemTime) -> String {
    let datetime: DateTime<Local> = time.into();
    datetime.format("%Y-%m-%d %H:%M:%S %Z").to_string()
}

pub fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["bytes", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit_index = 0;

    while value >= 1024.0 && unit_index < UNITS.len() - 1 {
        value /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} bytes", bytes)
    } else {
        format!("{value:.2} {} ({} bytes)", UNITS[unit_index], bytes)
    }
}
