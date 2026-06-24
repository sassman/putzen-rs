//! All inputs the pure `update` reacts to.

use crate::caches::model::Cache;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum Msg {
    MoveUp,
    MoveDown,
    ToggleMark,
    MarkDownToCursor,
    CycleSort,
    DrillIn,
    DrillOut,
    ToggleFocus,
    RequestQuit,
    DeletePressed,
    ConfirmDelete,
    CancelDelete,
    ConfirmActiveMark,
    CancelActiveMark,
    FilterStart,
    FilterChar(char),
    FilterBackspace,
    FilterApply,
    FilterCancel,
    MarkAllVisible,
    Tick,
    OverlayDismiss,
    ScanCompleted {
        parent_label: String,
        parent_path: PathBuf,
        children: Vec<Cache>,
    },
    RefreshCompleted {
        path: PathBuf,
        cache: Cache,
    },
    DeleteCompleted {
        freed: u64,
        deleted_count: usize,
        failed_count: usize,
        deleted_indices: Vec<usize>,
    },
    /// Top-level seed scan finished — replaces the empty initial list.
    SeedsLoaded {
        caches: Vec<Cache>,
    },
    /// Streamed from the LoadSeeds worker every few hundred directories so
    /// the spinner can show progress instead of just elapsed seconds.
    ScanProgress {
        folders: usize,
    },
}
