//! Pure formatters: human-readable bytes, durations, and display labels.

use std::time::{Duration, SystemTime};

/// Format a `SystemTime` as an absolute `YYYY-MM-DD` date in the system local zone.
pub fn human_date(t: SystemTime) -> String {
    let ts: jiff::Timestamp = t.try_into().unwrap_or(jiff::Timestamp::UNIX_EPOCH);
    let zoned = ts.to_zoned(jiff::tz::TimeZone::system());
    zoned.strftime("%Y-%m-%d").to_string()
}

pub fn human_size(bytes: u64) -> String {
    // 1024-based (IEC binary) units, matching the project-wide `HumanReadable`
    // trait in src/lib.rs. Decimal SI (1000-based) is reserved for human_count.
    // Single decimal place for values < 10 of a unit; integer for >= 10.
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut n = bytes as f64;
    let mut idx = 0;
    while n >= 1024.0 && idx < UNITS.len() - 1 {
        n /= 1024.0;
        idx += 1;
    }
    if idx == 0 {
        format!("{} {}", bytes, UNITS[idx])
    } else if n < 10.0 {
        format!("{:.1} {}", n, UNITS[idx])
    } else {
        format!("{:.0} {}", n, UNITS[idx])
    }
}

pub fn human_age(cold: Duration) -> String {
    let secs = cold.as_secs();
    const MIN:  u64 = 60;
    const HOUR: u64 = 60 * MIN;
    const DAY:  u64 = 24 * HOUR;
    const MO:   u64 = 30 * DAY;
    const YEAR: u64 = 365 * DAY;
    if secs >= YEAR { format!("{}y", secs / YEAR) }
    else if secs >= MO { format!("{}mo", secs / MO) }
    else if secs >= DAY { format!("{}d", secs / DAY) }
    else if secs >= HOUR { format!("{}h", secs / HOUR) }
    else if secs >= MIN { format!("{}m", secs / MIN) }
    else { format!("{}s", secs) }
}

pub fn label_from_path(path: &str) -> String {
    let last = path.rsplit('/').next().unwrap_or(path);
    last.trim_start_matches('.').to_string()
}

/// Format an integer count with `.` thousands separators (European style).
pub fn human_int(n: u64) -> String {
    let s = n.to_string();
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i).is_multiple_of(3) {
            out.push('.');
        }
        out.push(*b as char);
    }
    out
}

/// Format a positive floating-point count with k / M / G / T suffixes.
/// Sub-1000 values render as integers. Used for "impact points" (size × age).
pub fn human_count(n: f64) -> String {
    const UNITS: [&str; 5] = ["", "k", "M", "G", "T"];
    let mut n = n.max(0.0);
    let mut idx = 0;
    while n >= 1000.0 && idx < UNITS.len() - 1 {
        n /= 1000.0;
        idx += 1;
    }
    if idx == 0 {
        format!("{:.0}", n)
    } else {
        format!("{:.1} {}", n, UNITS[idx])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn human_size_bytes() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(1023), "1023 B");
    }
    #[test]
    fn human_size_kib() {
        assert_eq!(human_size(1024), "1.0 KiB");
        assert_eq!(human_size(1536), "1.5 KiB");
    }
    #[test]
    fn human_size_gib() {
        assert_eq!(human_size(2_684_354_560), "2.5 GiB");
    }

    #[test]
    fn human_age_buckets() {
        assert_eq!(human_age(Duration::from_secs(30)),       "30s");
        assert_eq!(human_age(Duration::from_secs(90)),       "1m");
        assert_eq!(human_age(Duration::from_secs(3600)),     "1h");
        assert_eq!(human_age(Duration::from_secs(86_400)),   "1d");
        assert_eq!(human_age(Duration::from_secs(7_776_000)),"3mo");
        assert_eq!(human_age(Duration::from_secs(2 * 365 * 86_400)), "2y");
    }

    #[test]
    fn human_date_formats_unix_epoch() {
        let t = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let s = human_date(t);
        // 2023-11-14 in UTC; ok if rendered local — tolerate ±1 day across zones.
        assert!(s.starts_with("2023-") || s.starts_with("2024-"), "got: {s}");
        assert_eq!(s.len(), 10, "expected YYYY-MM-DD, got: {s}");
    }

    #[test]
    fn human_int_thousands_dotted() {
        assert_eq!(human_int(0),         "0");
        assert_eq!(human_int(999),       "999");
        assert_eq!(human_int(1_000),     "1.000");
        assert_eq!(human_int(47_218),    "47.218");
        assert_eq!(human_int(1_000_000), "1.000.000");
    }

    #[test]
    fn human_count_buckets() {
        assert_eq!(human_count(0.0),         "0");
        assert_eq!(human_count(42.4),        "42");
        assert_eq!(human_count(999.0),       "999");
        assert_eq!(human_count(1_000.0),     "1.0 k");
        assert_eq!(human_count(1_100.0),     "1.1 k");
        assert_eq!(human_count(2_500_000.0), "2.5 M");
        assert_eq!(human_count(1.5e9),       "1.5 G");
    }

    #[test]
    fn label_strips_leading_dot() {
        assert_eq!(label_from_path(".npm"), "npm");
        assert_eq!(label_from_path("Library/Caches/Homebrew"), "Homebrew");
        assert_eq!(label_from_path("go/pkg/mod"), "mod");
        assert_eq!(label_from_path("/var/cache"), "cache");
    }
}
