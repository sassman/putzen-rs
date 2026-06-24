//! `putzen caches` — interactive cache cleanup TUI.

pub mod defaults;
pub mod format;
pub mod model;
pub mod scan;
pub mod tui;

use std::io;
use std::path::PathBuf;

pub struct CachesArgs {
    pub roots: Vec<PathBuf>,
    pub floor: Option<String>,
    pub dry_run: bool,
    pub yes: bool,
}

pub fn run(args: CachesArgs) -> io::Result<()> {
    use std::time::{Duration, Instant, SystemTime};

    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("HOME is not set"))?;

    let seeds = select_seeds(&home, &args.roots);

    let floor = args
        .floor
        .as_deref()
        .map(parse_duration)
        .transpose()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?
        .unwrap_or(Duration::from_secs(7 * 86_400));

    // Start with an empty list + a visible spinner.  The actual seed scan
    // runs on a worker (Effect::LoadSeeds) so the TUI is responsive
    // immediately even when HOME contains huge cache trees.
    let state = tui::State {
        now: SystemTime::now(),
        all: Vec::new(),
        sort: model::Sort::Score,
        marks: model::MarkSet::default(),
        cursor: 0,
        files_cursor: 0,
        floor: model::FloorPolicy { floor },
        focus_right: false,
        stack: Vec::new(),
        stack_labels: Vec::new(),
        quit: false,
        modal: tui::Modal::None,
        dry_run: args.dry_run,
        yes_mode: args.yes,
        total_freed: 0,
        filter: None,
        loading: Some(tui::Loading {
            label: "scanning folders".into(),
            frame: 0,
            started: Instant::now(),
            folders: Some(0),
        }),
        overlay: None,
        level_dirty: false,
        drill_paths: Vec::new(),
        cursor_stack: Vec::new(),
    };

    let initial_effects = vec![tui::Effect::LoadSeeds { seeds }];

    let mut term = tui::enter_tui()?;
    let loop_result = tui::run_loop(&mut term, state, initial_effects);
    let (final_state, total_freed) = match loop_result {
        Ok(out) => out,
        Err(e) => {
            let _ = tui::leave_tui(&mut term);
            return Err(e);
        }
    };
    let _ = final_state;

    tui::leave_tui(&mut term)?;

    #[cfg(feature = "highscore-board")]
    if !args.dry_run && total_freed > 0 {
        use crate::RunObserver;
        let mut obs = crate::HighscoreObserver::load()?;
        if let Some(medal) = obs.on_run_complete(total_freed) {
            println!("{medal}");
        }
    }

    #[cfg(not(feature = "highscore-board"))]
    let _ = total_freed;

    Ok(())
}

/// Accepts a duration like "24h", "7d", "2w", or "1y". Returns Err on parse failure.
pub fn parse_duration(s: &str) -> Result<std::time::Duration, String> {
    use std::time::Duration;
    let (num, unit) = s.split_at(s.len().saturating_sub(1));
    let n: u64 = num.parse().map_err(|_| format!("bad duration `{s}`"))?;
    match unit {
        "h" => Ok(Duration::from_secs(n * 3_600)),
        "d" => Ok(Duration::from_secs(n * 86_400)),
        "w" => Ok(Duration::from_secs(n * 7 * 86_400)),
        "y" => Ok(Duration::from_secs(n * 365 * 86_400)),
        _ => Err(format!("bad duration unit in `{s}`, expected h|d|w|y")),
    }
}

#[cfg(test)]
mod parse_duration_tests {
    use super::*;
    #[test]
    fn parses_hours_days_years() {
        assert_eq!(parse_duration("24h").unwrap().as_secs(), 24 * 3600);
        assert_eq!(parse_duration("7d").unwrap().as_secs(), 7 * 86_400);
        assert_eq!(parse_duration("1y").unwrap().as_secs(), 365 * 86_400);
    }
    #[test]
    fn rejects_garbage() {
        assert!(parse_duration("hello").is_err());
        assert!(parse_duration("7x").is_err());
    }
}

/// Resolve a HOME-relative path string against `$HOME`. Absolute paths
/// (`/...`) pass through unchanged.
pub fn resolve_path(home: &std::path::Path, raw: &str) -> std::path::PathBuf {
    if raw.starts_with('/') {
        std::path::PathBuf::from(raw)
    } else {
        home.join(raw)
    }
}

/// Pick the seed set for the scan: `--root` values when given, otherwise
/// the built-in defaults resolved against `home`. The two are alternatives
/// rather than complementary — passing `--root` is the user telling us
/// "scan this tree, not the usual ones".
pub fn select_seeds(home: &std::path::Path, roots: &[PathBuf]) -> Vec<PathBuf> {
    if roots.is_empty() {
        defaults::defaults()
            .map(|r| resolve_path(home, r.path))
            .collect()
    } else {
        roots.to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn resolve_home_relative() {
        let home = PathBuf::from("/u/sven");
        assert_eq!(
            resolve_path(&home, ".cargo/registry"),
            PathBuf::from("/u/sven/.cargo/registry")
        );
    }

    #[test]
    fn resolve_absolute_passthrough() {
        let home = PathBuf::from("/u/sven");
        assert_eq!(
            resolve_path(&home, "/var/cache"),
            PathBuf::from("/var/cache")
        );
    }

    #[test]
    fn select_seeds_no_roots_uses_defaults() {
        let home = PathBuf::from("/u/sven");
        let seeds = select_seeds(&home, &[]);
        assert!(!seeds.is_empty(), "default seeds must be populated");
        assert!(
            seeds.iter().any(|p| p.starts_with(&home)),
            "default seeds resolve under $HOME"
        );
    }

    #[test]
    fn select_seeds_with_roots_replaces_defaults() {
        let home = PathBuf::from("/u/sven");
        let roots = vec![PathBuf::from("/tmp/scratch"), PathBuf::from("/var/cache")];
        let seeds = select_seeds(&home, &roots);
        assert_eq!(seeds, roots, "--root replaces, never extends");
    }
}
