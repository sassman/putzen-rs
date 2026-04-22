pub mod display;
pub mod podium;

pub use display::render_board;

use crate::highscore::display::{inline_hint, render_medals, EarnedMedal, TrackName};
use crate::highscore::podium::{Medal, Podium};
use crate::observer::RunObserver;

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Resolve the on-disk path to `highscores.toml` under the user's config dir.
pub(crate) fn highscores_path() -> std::io::Result<PathBuf> {
    let config_dir = dirs_lite::config_dir().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not determine config directory",
        )
    })?;
    Ok(config_dir.join("putzen").join("highscores.toml"))
}

/// Mirror `Podium::place` bumping logic on earned medals for a given track.
/// When a new medal is placed, existing earned medals shift down one tier
/// and any that fall off the podium are removed.
fn bump_earned_medals(earned: &mut Vec<EarnedMedal>, track: TrackName, new_medal: Medal) {
    match new_medal {
        Medal::Gold => {
            earned.retain(|m| !(m.track == track && m.medal == Medal::Bronze));
            for m in earned.iter_mut() {
                if m.track == track && m.medal == Medal::Silver {
                    m.medal = Medal::Bronze;
                }
            }
            for m in earned.iter_mut() {
                if m.track == track && m.medal == Medal::Gold {
                    m.medal = Medal::Silver;
                }
            }
        }
        Medal::Silver => {
            earned.retain(|m| !(m.track == track && m.medal == Medal::Bronze));
            for m in earned.iter_mut() {
                if m.track == track && m.medal == Medal::Silver {
                    m.medal = Medal::Bronze;
                }
            }
        }
        Medal::Bronze => {
            earned.retain(|m| !(m.track == track && m.medal == Medal::Bronze));
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Highscores {
    #[serde(default)]
    pub single_cleanup: Podium,
    #[serde(default)]
    pub total_run: Podium,
}

impl Highscores {
    /// Load from the real user config directory.
    /// Returns `Self::default()` if the file doesn't exist yet.
    pub fn load() -> std::io::Result<Self> {
        Self::load_from(highscores_path()?)
    }

    /// Load from an explicit path (used by tests and by `load`).
    /// Returns `Self::default()` if the path doesn't exist.
    pub fn load_from(file_path: PathBuf) -> std::io::Result<Self> {
        if file_path.exists() {
            let content = fs::read_to_string(&file_path)?;
            toml::from_str(&content)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
        } else {
            Ok(Self::default())
        }
    }
}

pub struct HighscoreObserver {
    highscores: Highscores,
    earned_medals: Vec<EarnedMedal>,
    file_path: PathBuf,
}

impl HighscoreObserver {
    /// Load highscores from disk or create a new empty set.
    pub fn load() -> std::io::Result<Self> {
        let file_path = highscores_path()?;
        let (highscores, is_first_run) = if file_path.exists() {
            let content = fs::read_to_string(&file_path)?;
            let highscores: Highscores = toml::from_str(&content)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            (highscores, false)
        } else {
            (Highscores::default(), true)
        };

        if is_first_run {
            println!("\u{1F3C6} A wild cleaner appears! Highscore board initialized.");
        }

        Ok(Self {
            highscores,
            earned_medals: Vec::new(),
            file_path,
        })
    }

    /// Load from an explicit path (for testing, no first-run message).
    pub fn load_from(file_path: PathBuf) -> std::io::Result<Self> {
        let highscores = if file_path.exists() {
            let content = fs::read_to_string(&file_path)?;
            toml::from_str(&content)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
        } else {
            Highscores::default()
        };

        Ok(Self {
            highscores,
            earned_medals: Vec::new(),
            file_path,
        })
    }

    fn today() -> String {
        jiff::Zoned::now().date().to_string()
    }

    fn save(&self) -> std::io::Result<()> {
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(&self.highscores).map_err(std::io::Error::other)?;
        fs::write(&self.file_path, content)
    }
}

impl RunObserver for HighscoreObserver {
    fn on_folder_cleaned(&mut self, size: u64) -> Option<String> {
        let medal = self.highscores.single_cleanup.would_place(size)?;
        let date = Self::today();
        self.highscores.single_cleanup.place(size, &date);
        bump_earned_medals(&mut self.earned_medals, TrackName::SingleCleanup, medal);
        self.earned_medals.push(EarnedMedal {
            medal,
            track: TrackName::SingleCleanup,
            size,
        });
        Some(inline_hint())
    }

    fn on_run_complete(&mut self, total: u64) -> Option<String> {
        // Check total run highscore (skip if nothing was cleaned)
        if total > 0 {
            if let Some(medal) = self.highscores.total_run.would_place(total) {
                let date = Self::today();
                self.highscores.total_run.place(total, &date);
                bump_earned_medals(&mut self.earned_medals, TrackName::TotalRun, medal);
                self.earned_medals.push(EarnedMedal {
                    medal,
                    track: TrackName::TotalRun,
                    size: total,
                });
            }
        }

        // Save to disk (best effort — don't fail the run)
        let _ = self.save();

        render_medals(&self.earned_medals)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observer::RunObserver;

    #[test]
    fn first_cleanup_returns_hint() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("highscores.toml");
        let mut observer = HighscoreObserver::load_from(path).unwrap();

        let hint = observer.on_folder_cleaned(1024);
        assert!(hint.is_some());
        assert!(hint.unwrap().contains("new highscore!"));
    }

    #[test]
    fn small_cleanup_after_big_one_no_hint_when_podium_full() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("highscores.toml");
        let mut observer = HighscoreObserver::load_from(path).unwrap();

        observer.on_folder_cleaned(3000);
        observer.on_folder_cleaned(2000);
        observer.on_folder_cleaned(1000);

        // Podium full, 500 is too small
        let hint = observer.on_folder_cleaned(500);
        assert!(hint.is_none());
    }

    #[test]
    fn on_run_complete_renders_medals() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("highscores.toml");
        let mut observer = HighscoreObserver::load_from(path).unwrap();

        observer.on_folder_cleaned(1_073_741_824); // 1 GiB
        let output = observer.on_run_complete(1_073_741_824);
        assert!(output.is_some());
        let text = output.unwrap();
        assert!(text.contains("NEW HIGHSCORE"));
        assert!(text.contains("Gold"));
    }

    #[test]
    fn saves_and_reloads_highscores() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("highscores.toml");

        // First run
        {
            let mut observer = HighscoreObserver::load_from(path.clone()).unwrap();
            observer.on_folder_cleaned(5000);
            observer.on_run_complete(5000);
        }

        // Second run — should load saved data
        {
            let mut observer = HighscoreObserver::load_from(path).unwrap();
            // 5000 is gold, so 3000 should place silver
            let hint = observer.on_folder_cleaned(3000);
            assert!(hint.is_some());
        }
    }

    #[test]
    fn many_increasing_cleanups_produce_at_most_three_medals_per_track() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("highscores.toml");
        let mut observer = HighscoreObserver::load_from(path).unwrap();

        // Simulate 10 folders of increasing size — each beats the previous gold
        for i in 1..=10 {
            observer.on_folder_cleaned(i * 1000);
        }

        let output = observer.on_run_complete(55_000);
        let text = output.unwrap();

        // Should have at most 3 single-cleanup medals + 1 total-run medal
        assert_eq!(text.matches("Single cleanup").count(), 3);
        assert_eq!(text.matches("Total run").count(), 1);

        // Exactly 1 gold, 1 silver, 1 bronze for single cleanup
        assert_eq!(text.matches("Gold \u{00B7} Single").count(), 1);
        assert_eq!(text.matches("Silver \u{00B7} Single").count(), 1);
        assert_eq!(text.matches("Bronze \u{00B7} Single").count(), 1);
    }

    #[test]
    fn first_run_creates_file_on_save() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("highscores.toml");
        assert!(!path.exists());
        let mut observer = HighscoreObserver::load_from(path.clone()).unwrap();
        observer.on_folder_cleaned(1000);
        observer.on_run_complete(1000);
        assert!(path.exists());
    }

    #[test]
    fn highscores_load_from_returns_default_when_file_missing() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("does_not_exist.toml");
        let highscores = Highscores::load_from(path).unwrap();
        assert!(highscores.single_cleanup.gold.is_none());
        assert!(highscores.total_run.gold.is_none());
    }

    #[test]
    fn highscores_load_from_parses_existing_file() {
        // Write a file via the observer so we know the format is correct.
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("highscores.toml");
        {
            let mut observer = HighscoreObserver::load_from(path.clone()).unwrap();
            observer.on_folder_cleaned(42_000);
            observer.on_run_complete(42_000);
        }

        let highscores = Highscores::load_from(path).unwrap();
        assert_eq!(
            highscores.single_cleanup.gold.as_ref().unwrap().size,
            42_000
        );
        assert_eq!(highscores.total_run.gold.as_ref().unwrap().size, 42_000);
    }

    #[test]
    fn highscores_load_from_rejects_malformed_toml() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("highscores.toml");
        fs::write(&path, "this is not toml = = {").unwrap();
        let err = Highscores::load_from(path).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    }
}
