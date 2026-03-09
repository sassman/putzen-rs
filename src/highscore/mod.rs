pub mod display;
pub mod podium;

use crate::observer::RunObserver;
use crate::highscore::display::{EarnedMedal, TrackName, render_medals, inline_hint};
use crate::highscore::podium::Podium;

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Highscores {
    #[serde(default)]
    pub single_cleanup: Podium,
    #[serde(default)]
    pub total_run: Podium,
}

pub struct HighscoreObserver {
    highscores: Highscores,
    earned_medals: Vec<EarnedMedal>,
    file_path: PathBuf,
    is_first_run: bool,
}

impl HighscoreObserver {
    /// Load highscores from disk or create a new empty set.
    pub fn load() -> std::io::Result<Self> {
        let file_path = Self::highscores_path()?;
        let (highscores, is_first_run) = if file_path.exists() {
            let content = fs::read_to_string(&file_path)?;
            let highscores: Highscores = toml::from_str(&content)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            (highscores, false)
        } else {
            (Highscores::default(), true)
        };

        Ok(Self {
            highscores,
            earned_medals: Vec::new(),
            file_path,
            is_first_run,
        })
    }

    /// Load from an explicit path (for testing).
    pub fn load_from(file_path: PathBuf) -> std::io::Result<Self> {
        let (highscores, is_first_run) = if file_path.exists() {
            let content = fs::read_to_string(&file_path)?;
            let highscores: Highscores = toml::from_str(&content)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            (highscores, false)
        } else {
            (Highscores::default(), true)
        };

        Ok(Self {
            highscores,
            earned_medals: Vec::new(),
            file_path,
            is_first_run,
        })
    }

    fn highscores_path() -> std::io::Result<PathBuf> {
        let config_dir = dirs_lite::config_dir()
            .ok_or_else(|| std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Could not determine config directory",
            ))?;
        Ok(config_dir.join("putzen").join("highscores.toml"))
    }

    fn today() -> String {
        jiff::Zoned::now().date().to_string()
    }

    fn save(&self) -> std::io::Result<()> {
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(&self.highscores)
            .map_err(std::io::Error::other)?;
        fs::write(&self.file_path, content)
    }
}

impl RunObserver for HighscoreObserver {
    fn on_folder_cleaned(&mut self, size: u64) -> Option<String> {
        let medal = self.highscores.single_cleanup.would_place(size)?;
        let date = Self::today();
        self.highscores.single_cleanup.place(size, &date);
        self.earned_medals.push(EarnedMedal {
            medal,
            track: TrackName::SingleCleanup,
            size,
        });
        Some(inline_hint())
    }

    fn on_run_complete(&mut self, total: u64) -> Option<String> {
        if self.is_first_run {
            println!("\u{1F3C6} A wild cleaner appears! Highscore board initialized.");
        }

        // Check total run highscore (skip if nothing was cleaned)
        if total > 0 {
            if let Some(medal) = self.highscores.total_run.would_place(total) {
                let date = Self::today();
                self.highscores.total_run.place(total, &date);
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
            assert!(observer.is_first_run);
            observer.on_folder_cleaned(5000);
            observer.on_run_complete(5000);
        }

        // Second run — should load saved data
        {
            let mut observer = HighscoreObserver::load_from(path).unwrap();
            assert!(!observer.is_first_run);
            // 5000 is gold, so 3000 should place silver
            let hint = observer.on_folder_cleaned(3000);
            assert!(hint.is_some());
        }
    }

    #[test]
    fn first_run_flag_detected() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("highscores.toml");
        let observer = HighscoreObserver::load_from(path).unwrap();
        assert!(observer.is_first_run);
    }
}
