//! All inputs the pure `update` reacts to.

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
    ConfirmQuit,
    CancelQuit,
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
        parent_path: std::path::PathBuf,
        children: Vec<crate::caches::model::Cache>,
    },
    RefreshCompleted {
        path: std::path::PathBuf,
        cache: crate::caches::model::Cache,
    },
    DeleteCompleted {
        freed: u64,
        deleted_count: usize,
        deleted_indices: Vec<usize>,
    },
}
