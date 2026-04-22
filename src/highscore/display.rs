use crate::highscore::podium::Medal;
use crate::HumanReadable;

/// The name of a highscore track, used in display output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackName {
    SingleCleanup,
    TotalRun,
}

impl TrackName {
    pub fn sort_key(self) -> u8 {
        match self {
            TrackName::SingleCleanup => 0,
            TrackName::TotalRun => 1,
        }
    }
}

impl std::fmt::Display for TrackName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrackName::SingleCleanup => write!(f, "Single cleanup"),
            TrackName::TotalRun => write!(f, "Total run"),
        }
    }
}

/// A record of a new medal earned during the current run.
pub struct EarnedMedal {
    pub medal: Medal,
    pub track: TrackName,
    pub size: u64,
}

impl Medal {
    pub fn emoji(&self) -> &'static str {
        match self {
            Medal::Gold => "\u{1F947}",   // 🥇
            Medal::Silver => "\u{1F948}", // 🥈
            Medal::Bronze => "\u{1F949}", // 🥉
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Medal::Gold => "Gold",
            Medal::Silver => "Silver",
            Medal::Bronze => "Bronze",
        }
    }
}

/// Banner header line, e.g. `   ──── ★ NEW HIGHSCORE ★ ────`.
fn banner_header(title: &str) -> String {
    format!(
        "   \u{2500}\u{2500}\u{2500}\u{2500} \u{2605} {} \u{2605} \u{2500}\u{2500}\u{2500}\u{2500}",
        title
    )
}

/// Horizontal rule used as the bottom of a banner / separator between slots.
fn banner_rule() -> &'static str {
    "   \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}"
}

/// Render a single medal banner.
fn render_medal(earned: &EarnedMedal) -> String {
    let size = (earned.size as usize).as_human_readable();
    format!(
        "\n{}\n     {} {} \u{00B7} {}\n          {}\n{}",
        banner_header("NEW HIGHSCORE"),
        earned.medal.emoji(),
        earned.medal.label(),
        earned.track,
        size,
        banner_rule(),
    )
}

/// Render all earned medals into a single display string.
/// Medals are sorted by track (Single cleanup first, then Total run),
/// then by medal rank (Gold, Silver, Bronze) within each track.
pub fn render_medals(medals: &[EarnedMedal]) -> Option<String> {
    if medals.is_empty() {
        return None;
    }
    let mut sorted: Vec<&EarnedMedal> = medals.iter().collect();
    sorted.sort_by_key(|m| (m.track.sort_key(), m.medal.sort_key()));
    let output: String = sorted.iter().map(|m| render_medal(m)).collect();
    Some(output)
}

/// Return the inline hint string for a new highscore.
pub fn inline_hint() -> String {
    "\u{1F3C6} new highscore!".to_string() // 🏆 new highscore!
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_single_gold_medal() {
        let medals = vec![EarnedMedal {
            medal: Medal::Gold,
            track: TrackName::SingleCleanup,
            size: 2_684_354_560, // 2.5 GiB
        }];
        let output = render_medals(&medals).unwrap();
        assert!(output.contains("NEW HIGHSCORE"));
        assert!(output.contains("Gold"));
        assert!(output.contains("Single cleanup"));
        assert!(output.contains("2.5GiB"));
    }

    #[test]
    fn render_multiple_medals() {
        let medals = vec![
            EarnedMedal {
                medal: Medal::Gold,
                track: TrackName::SingleCleanup,
                size: 2_684_354_560,
            },
            EarnedMedal {
                medal: Medal::Silver,
                track: TrackName::TotalRun,
                size: 1_073_741_824,
            },
        ];
        let output = render_medals(&medals).unwrap();
        // Should contain both medals
        assert!(output.contains("Gold"));
        assert!(output.contains("Silver"));
    }

    #[test]
    fn render_medals_sorted_by_track_then_rank() {
        let medals = vec![
            EarnedMedal {
                medal: Medal::Gold,
                track: TrackName::SingleCleanup,
                size: 3_000_000_000,
            },
            EarnedMedal {
                medal: Medal::Bronze,
                track: TrackName::SingleCleanup,
                size: 500_000_000,
            },
            EarnedMedal {
                medal: Medal::Silver,
                track: TrackName::SingleCleanup,
                size: 2_000_000_000,
            },
            EarnedMedal {
                medal: Medal::Gold,
                track: TrackName::TotalRun,
                size: 5_500_000_000,
            },
        ];
        let output = render_medals(&medals).unwrap();
        let gold_pos = output.find("Gold \u{00B7} Single").unwrap();
        let silver_pos = output.find("Silver \u{00B7} Single").unwrap();
        let bronze_pos = output.find("Bronze \u{00B7} Single").unwrap();
        let total_pos = output.find("Gold \u{00B7} Total").unwrap();
        assert!(gold_pos < silver_pos);
        assert!(silver_pos < bronze_pos);
        assert!(bronze_pos < total_pos);
    }

    #[test]
    fn render_empty_returns_none() {
        assert!(render_medals(&[]).is_none());
    }

    #[test]
    fn inline_hint_contains_trophy() {
        let hint = inline_hint();
        assert!(hint.contains("new highscore!"));
    }
}
