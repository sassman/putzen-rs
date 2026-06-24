//! Pure formatters: human-readable bytes, durations, and display labels.

use std::path::Path;
use std::time::{Duration, SystemTime};

/// Render `path` with the user's home directory collapsed to `~/`. Falls
/// back to the full `display()` form when the path isn't under `home` or
/// when `home` is `None`.
pub fn tildify(path: &Path, home: Option<&Path>) -> String {
    if let Some(home) = home {
        if let Ok(rest) = path.strip_prefix(home) {
            if rest.as_os_str().is_empty() {
                return "~".to_string();
            }
            return format!("~/{}", rest.display());
        }
    }
    path.display().to_string()
}

/// Format a `SystemTime` as an absolute `YYYY-MM-DD` date in the system local zone.
pub fn human_date(t: SystemTime) -> String {
    let ts: jiff::Timestamp = t.try_into().unwrap_or(jiff::Timestamp::UNIX_EPOCH);
    let zoned = ts.to_zoned(jiff::tz::TimeZone::system());
    zoned.strftime("%Y-%m-%d").to_string()
}

/// Format `bytes` as `(number, unit)` so callers that stack values can
/// right-align number and unit in separate sub-columns.  Number is up to
/// 4 chars (`"1023"`, `"9.9"`, `"999"`); unit is 1 (`"B"`) or 3 (`"KiB"`
/// … `"TiB"`).
pub fn human_size_parts(bytes: u64) -> (String, &'static str) {
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
    let num = if idx == 0 {
        bytes.to_string()
    } else if n < 10.0 {
        format!("{n:.1}")
    } else {
        format!("{n:.0}")
    };
    (num, UNITS[idx])
}

pub fn human_size(bytes: u64) -> String {
    let (num, unit) = human_size_parts(bytes);
    format!("{num} {unit}")
}

/// Truncate `s` to at most `width` display columns, replacing the tail with
/// `…` when it would otherwise overflow. Width is measured by `chars().count()`
/// — fine for the cache-folder names we render, which are ASCII-ish in practice
/// and don't carry wide CJK or grapheme clusters.
pub fn truncate_with_ellipsis(s: &str, width: usize) -> String {
    if s.chars().count() <= width {
        return s.to_string();
    }
    if width == 0 {
        return String::new();
    }
    if width == 1 {
        return "…".to_string();
    }
    let head: String = s.chars().take(width - 1).collect();
    format!("{head}…")
}

/// Pick `singular` when `n == 1`, `plural` otherwise. Trivial helper, but
/// having one place beats `if n == 1 { "x" } else { "xs" }` sprinkled across
/// six call sites — and grep-ing for `pluralize` is easier than chasing
/// inline ternaries.
pub fn pluralize<'a>(n: u64, singular: &'a str, plural: &'a str) -> &'a str {
    if n == 1 {
        singular
    } else {
        plural
    }
}

pub fn human_age(cold: Duration) -> String {
    let secs = cold.as_secs();
    const MIN: u64 = 60;
    const HOUR: u64 = 60 * MIN;
    const DAY: u64 = 24 * HOUR;
    const MO: u64 = 30 * DAY;
    const YEAR: u64 = 365 * DAY;
    if secs >= YEAR {
        format!("{}y", secs / YEAR)
    } else if secs >= MO {
        format!("{}mo", secs / MO)
    } else if secs >= DAY {
        format!("{}d", secs / DAY)
    } else if secs >= HOUR {
        format!("{}h", secs / HOUR)
    } else if secs >= MIN {
        format!("{}m", secs / MIN)
    } else {
        format!("{}s", secs)
    }
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
    use std::path::PathBuf;
    use std::time::Duration;

    #[test]
    fn tildify_collapses_home_prefix() {
        let home = PathBuf::from("/u/sven");
        let p = PathBuf::from("/u/sven/.cargo/registry");
        assert_eq!(tildify(&p, Some(&home)), "~/.cargo/registry");
    }

    #[test]
    fn tildify_home_itself_renders_as_tilde() {
        let home = PathBuf::from("/u/sven");
        assert_eq!(tildify(&home, Some(&home)), "~");
    }

    #[test]
    fn tildify_keeps_outside_paths_intact() {
        let home = PathBuf::from("/u/sven");
        let p = PathBuf::from("/var/cache/something");
        assert_eq!(tildify(&p, Some(&home)), "/var/cache/something");
    }

    #[test]
    fn tildify_without_home_renders_absolute() {
        let p = PathBuf::from("/u/sven/.cargo");
        assert_eq!(tildify(&p, None), "/u/sven/.cargo");
    }

    #[test]
    fn truncate_passes_short_strings_through() {
        assert_eq!(truncate_with_ellipsis("npm", 8), "npm");
    }

    #[test]
    fn truncate_replaces_tail_with_ellipsis() {
        assert_eq!(truncate_with_ellipsis("huggingface-hub", 8), "hugging…");
    }

    #[test]
    fn truncate_degenerate_widths() {
        assert_eq!(truncate_with_ellipsis("abc", 0), "");
        assert_eq!(truncate_with_ellipsis("abc", 1), "…");
        assert_eq!(truncate_with_ellipsis("abc", 3), "abc");
    }

    #[test]
    fn pluralize_picks_singular_for_one() {
        assert_eq!(pluralize(1, "folder", "folders"), "folder");
        assert_eq!(pluralize(0, "folder", "folders"), "folders");
        assert_eq!(pluralize(2, "folder", "folders"), "folders");
        assert_eq!(pluralize(47, "entry", "entries"), "entries");
    }

    #[test]
    fn human_size_parts_keeps_number_and_unit_separate() {
        // Bytes branch: number = raw integer, unit = "B".
        assert_eq!(human_size_parts(713), ("713".into(), "B"));
        assert_eq!(human_size_parts(0), ("0".into(), "B"));
        // KiB branch with one decimal under 10.
        assert_eq!(human_size_parts(2 * 1024 + 512), ("2.5".into(), "KiB"));
        // KiB branch >= 10 → integer.
        assert_eq!(human_size_parts(28 * 1024), ("28".into(), "KiB"));
        // GiB branch.
        assert_eq!(human_size_parts(3 * 1024u64.pow(3)), ("3.0".into(), "GiB"));
    }

    #[test]
    fn human_size_stays_compatible_with_parts_split() {
        // The convenience wrapper still produces "{num} {unit}".
        let (n, u) = human_size_parts(28 * 1024);
        assert_eq!(human_size(28 * 1024), format!("{n} {u}"));
    }

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
        assert_eq!(human_age(Duration::from_secs(30)), "30s");
        assert_eq!(human_age(Duration::from_secs(90)), "1m");
        assert_eq!(human_age(Duration::from_secs(3600)), "1h");
        assert_eq!(human_age(Duration::from_secs(86_400)), "1d");
        assert_eq!(human_age(Duration::from_secs(7_776_000)), "3mo");
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
        assert_eq!(human_int(0), "0");
        assert_eq!(human_int(999), "999");
        assert_eq!(human_int(1_000), "1.000");
        assert_eq!(human_int(47_218), "47.218");
        assert_eq!(human_int(1_000_000), "1.000.000");
    }

    #[test]
    fn human_count_buckets() {
        assert_eq!(human_count(0.0), "0");
        assert_eq!(human_count(42.4), "42");
        assert_eq!(human_count(999.0), "999");
        assert_eq!(human_count(1_000.0), "1.0 k");
        assert_eq!(human_count(1_100.0), "1.1 k");
        assert_eq!(human_count(2_500_000.0), "2.5 M");
        assert_eq!(human_count(1.5e9), "1.5 G");
    }
}
