use chrono::{Local, TimeZone};

pub const TIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

pub fn curtimes() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as u64
}


pub fn ctshow() -> String {
    Local::now().format(TIME_FORMAT).to_string()
}

pub fn timeshow(t: u64) -> String {
    Local
        .timestamp_opt(t.min(i64::MAX as u64) as i64, 0)
        .single()
        .map(|dt| dt.format(TIME_FORMAT).to_string())
        .unwrap_or_else(|| format!("invalid timestamp {}", t))
}

// &str = "%Y-%m-%d %H:%M:%S";   %Y%m%d
pub fn timefmt(t: u64, fmts: &str) -> String {
    Local
        .timestamp_opt(t.min(i64::MAX as u64) as i64, 0)
        .single()
        .map(|dt| dt.format(fmts).to_string())
        .unwrap_or_else(|| format!("invalid timestamp {}", t))
}


