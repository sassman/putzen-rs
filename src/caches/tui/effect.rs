//! IO descriptions yielded by `update`. The runtime's `EffectRunner` is the
//! only place these are realised; each variant fixes which `Msg` is emitted
//! back into the loop on completion.

use super::Msg;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum Effect {
    /// Enumerate cache subdirectories. On completion: `Msg::ScanCompleted`.
    SpawnScan {
        parent_label: String,
        parent_path: PathBuf,
    },

    /// Re-stat a single cache directory. On completion: `Msg::RefreshCompleted`.
    SpawnRefresh { path: PathBuf },

    /// Delete the given items (real or dry-run). On completion: `Msg::DeleteCompleted`.
    SpawnDelete {
        items: Vec<(usize, PathBuf, u64)>,
        dry_run: bool,
    },

    /// Wait `dur`, then dispatch `msg` into the loop.
    EmitAfter { dur: Duration, msg: Msg },

    /// Run the top-level seed scan that populates the initial cache list.
    /// On completion: `Msg::SeedsLoaded`.  Dispatched once at startup so the
    /// TUI is already drawn (with a spinner) while this work happens.
    LoadSeeds { seeds: Vec<PathBuf> },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variants_construct() {
        let _ = Effect::SpawnScan {
            parent_label: "x".into(),
            parent_path: PathBuf::from("/x"),
        };
        let _ = Effect::SpawnRefresh {
            path: PathBuf::from("/x"),
        };
        let _ = Effect::SpawnDelete {
            items: vec![],
            dry_run: true,
        };
        let _ = Effect::EmitAfter {
            dur: Duration::from_millis(0),
            msg: Msg::Tick,
        };
    }
}
