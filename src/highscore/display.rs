use crate::highscore::podium::{Medal, Record};
use crate::highscore::Highscores;
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

/// Render one medal slot of the highscore board.
/// `record: None` renders an "open" placeholder.
/// The caller is responsible for any surrounding banner_header / banner_rule.
fn render_board_slot(medal: Medal, record: Option<&Record>) -> String {
    let detail = match record {
        Some(r) => format!(
            "{} \u{00B7} {}",
            (r.size as usize).as_human_readable(),
            r.date
        ),
        None => "(open \u{2014} be the first!)".to_string(),
    };
    format!(
        "     {} {}\n          {}",
        medal.emoji(),
        medal.label(),
        detail,
    )
}

/// Render the full two-track highscore board.
/// Format: per-track banner header, three slots (gold/silver/bronze) each
/// separated by a banner_rule, blank line between tracks.
pub fn render_board(highscores: &Highscores) -> String {
    let mut out = String::new();
    for (track, podium) in [
        (TrackName::SingleCleanup, &highscores.single_cleanup),
        (TrackName::TotalRun, &highscores.total_run),
    ] {
        let title = match track {
            TrackName::SingleCleanup => "SINGLE CLEANUP",
            TrackName::TotalRun => "TOTAL RUN",
        };
        out.push('\n');
        out.push_str(&banner_header(title));
        out.push('\n');
        out.push_str(&render_board_slot(Medal::Gold, podium.gold.as_ref()));
        out.push('\n');
        out.push_str(banner_rule());
        out.push('\n');
        out.push_str(&render_board_slot(Medal::Silver, podium.silver.as_ref()));
        out.push('\n');
        out.push_str(banner_rule());
        out.push('\n');
        out.push_str(&render_board_slot(Medal::Bronze, podium.bronze.as_ref()));
        out.push('\n');
        out.push_str(banner_rule());
        out.push('\n');
    }
    out
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

    #[test]
    fn render_board_slot_populated_contains_size_and_date() {
        let record = Record {
            size: 1_073_741_824, // 1 GiB
            date: "2026-03-15".to_string(),
        };
        let out = render_board_slot(Medal::Gold, Some(&record));
        assert!(out.contains("Gold"));
        assert!(out.contains("1.0GiB"));
        assert!(out.contains("2026-03-15"));
        assert!(!out.contains("open"));
    }

    #[test]
    fn render_board_slot_open_contains_marker() {
        let out = render_board_slot(Medal::Silver, None);
        assert!(out.contains("Silver"));
        assert!(out.contains("open"));
        assert!(out.contains("be the first"));
    }

    use crate::highscore::podium::Podium;

    fn populated_record(size: u64, date: &str) -> Record {
        Record {
            size,
            date: date.to_string(),
        }
    }

    #[test]
    fn render_board_empty_highscores_shows_all_open() {
        let highscores = Highscores::default();
        let out = render_board(&highscores);
        assert!(out.contains("SINGLE CLEANUP"));
        assert!(out.contains("TOTAL RUN"));
        // Six "open" markers — three per track
        assert_eq!(out.matches("(open").count(), 6);
        // No size units should appear when nothing is populated
        assert!(!out.contains("GiB"));
        assert!(!out.contains("MiB"));
        assert!(!out.contains("KiB"));
    }

    #[test]
    fn render_board_fully_populated_shows_all_records() {
        let highscores = Highscores {
            single_cleanup: Podium {
                gold: Some(populated_record(3_000_000_000, "2026-03-15")),
                silver: Some(populated_record(2_000_000_000, "2026-02-01")),
                bronze: Some(populated_record(1_000_000_000, "2026-01-20")),
            },
            total_run: Podium {
                gold: Some(populated_record(5_500_000_000, "2026-03-15")),
                silver: Some(populated_record(3_300_000_000, "2026-02-14")),
                bronze: Some(populated_record(1_100_000_000, "2026-01-10")),
            },
        };
        let out = render_board(&highscores);
        assert!(out.contains("SINGLE CLEANUP"));
        assert!(out.contains("TOTAL RUN"));
        assert_eq!(out.matches("Gold").count(), 2);
        assert_eq!(out.matches("Silver").count(), 2);
        assert_eq!(out.matches("Bronze").count(), 2);
        // Dates appear somewhere in the output
        assert!(out.contains("2026-03-15"));
        assert!(out.contains("2026-01-10"));
        // No open markers when everything is populated
        assert_eq!(out.matches("(open").count(), 0);
    }

    #[test]
    fn render_board_partial_track_mixes_populated_and_open() {
        let highscores = Highscores {
            single_cleanup: Podium {
                gold: Some(populated_record(1_073_741_824, "2026-03-15")),
                silver: None,
                bronze: None,
            },
            ..Default::default()
        };
        let out = render_board(&highscores);
        // Gold is populated → size + date appear
        assert!(out.contains("1.0GiB"));
        assert!(out.contains("2026-03-15"));
        // 5 open markers: silver+bronze of single-cleanup, all 3 of total-run
        assert_eq!(out.matches("(open").count(), 5);
    }
}
