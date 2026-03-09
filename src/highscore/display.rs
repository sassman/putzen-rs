use crate::highscore::podium::Medal;
use crate::HumanReadable;

/// The name of a highscore track, used in display output.
#[derive(Debug, Clone, Copy)]
pub enum TrackName {
    SingleCleanup,
    TotalRun,
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

/// Render a single medal banner.
fn render_medal(earned: &EarnedMedal) -> String {
    let size = (earned.size as usize).as_human_readable();
    format!(
        "\n   \u{2500}\u{2500}\u{2500}\u{2500} \u{2605} NEW HIGHSCORE \u{2605} \u{2500}\u{2500}\u{2500}\u{2500}\n     {} {} \u{00B7} {}\n          {}\n   \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}",
        earned.medal.emoji(),
        earned.medal.label(),
        earned.track,
        size,
    )
}

/// Render all earned medals into a single display string.
pub fn render_medals(medals: &[EarnedMedal]) -> Option<String> {
    if medals.is_empty() {
        return None;
    }
    let output: String = medals.iter().map(render_medal).collect();
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
    fn render_empty_returns_none() {
        assert!(render_medals(&[]).is_none());
    }

    #[test]
    fn inline_hint_contains_trophy() {
        let hint = inline_hint();
        assert!(hint.contains("new highscore!"));
    }
}
