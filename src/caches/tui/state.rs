//! TUI application state.

use super::filter::Filter;
use crate::caches::model::{Cache, FloorPolicy, MarkSet, Sort};
use std::path::PathBuf;
use std::time::SystemTime;

/// Frames of the loading spinner glyph, advanced once per event-loop idle tick.
pub const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Transient overlay shown after a delete pass completes. Dismissed
/// automatically after 2 s via `Effect::EmitAfter`.
pub struct Overlay {
    pub outcome: RunOutcome,
}

/// Outcome of a real or dry-run cache deletion pass.
pub struct RunOutcome {
    pub freed: u64,
    pub deleted: usize,
    /// Items the cleaner returned an `Err` for. `0` on dry runs.
    pub failed: usize,
    pub dry_run: bool,
}

/// Visual state of a background scan in progress.
pub struct Loading {
    /// Human label of the cache being scanned — shown in the spinner modal.
    pub label: String,
    /// Spinner animation frame index into `SPINNER_FRAMES`.
    pub frame: usize,
    /// When the scan started; used to render elapsed time when no per-task
    /// progress signal is available.
    pub started: std::time::Instant,
    /// `Some(n)` when the worker streams a folder-count via `ScanProgress`
    /// (the LoadSeeds startup scan). `None` for spinners that don't carry a
    /// progress signal, in which case the view falls back to elapsed time.
    pub folders: Option<usize>,
}

impl Loading {
    /// Advance the spinner one frame, wrapping at the end of the glyph cycle.
    pub fn update_frame(&mut self) {
        self.frame = (self.frame + 1) % SPINNER_FRAMES.len();
    }

    /// Current spinner glyph.
    pub fn glyph(&self) -> &'static str {
        SPINNER_FRAMES[self.frame]
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Modal {
    #[default]
    None,
    DeleteConfirm,
    ActiveMark(Vec<usize>),
    FilterEdit,
}

pub struct State {
    pub now: SystemTime,
    pub all: Vec<Cache>,
    pub sort: Sort,
    pub marks: MarkSet,
    pub cursor: usize,
    pub files_cursor: usize,
    pub floor: FloorPolicy,
    pub focus_right: bool,
    pub stack: Vec<Vec<Cache>>, // drill-in: parent levels saved here
    pub stack_labels: Vec<String>,
    pub quit: bool,
    pub modal: Modal,
    pub dry_run: bool,
    pub yes_mode: bool,
    /// Bytes freed across all deletion passes in this session.
    pub total_freed: u64,
    /// When `Some`, a less/vim-style `/` filter is active (possibly being
    /// edited). When `None`, no filter is applied.
    pub filter: Option<Filter>,
    /// `Some` while a background drill-in scan is running; drives the
    /// spinner modal.
    pub loading: Option<Loading>,
    /// `Some` for ~2 s after a delete pass completes; draws the result
    /// overlay until `Msg::OverlayDismiss` is received.
    pub overlay: Option<Overlay>,
    /// Set to true whenever something was successfully deleted at the
    /// current drill level. Reset on drill in/out. When we drill out and
    /// this was true, the parent's row for the cache we're leaving is
    /// re-scanned to reflect the smaller size.
    pub level_dirty: bool,
    /// Path stack parallel to `stack` so we know which entry in the
    /// restored parent corresponds to the cache we just drilled out of.
    /// Pushed on `drill_into`, popped on `drill_out`.
    pub drill_paths: Vec<PathBuf>,
    /// Cursor positions parallel to `stack`. On `drill_into` we save the
    /// current cursor; on `drill_out` we restore it (then clamp), so the
    /// user lands back on the row they were on instead of at the top.
    pub cursor_stack: Vec<usize>,
}

impl State {
    pub fn sorted_indices(&self) -> Vec<usize> {
        let mut idx: Vec<usize> = (0..self.all.len()).collect();
        if let Some(f) = &self.filter {
            idx.retain(|&i| f.is_visible(&self.all[i].path));
        }
        match self.sort {
            Sort::Score => idx.sort_by(|&a, &b| {
                self.all[b]
                    .score(self.now)
                    .partial_cmp(&self.all[a].score(self.now))
                    .unwrap()
            }),
            Sort::Size => idx.sort_by(|&a, &b| self.all[b].size_bytes.cmp(&self.all[a].size_bytes)),
            Sort::Age => idx.sort_by(|&a, &b| {
                let aa = self.all[a].age(self.now).map(|d| d.as_secs());
                let bb = self.all[b].age(self.now).map(|d| d.as_secs());
                match (aa, bb) {
                    (Some(x), Some(y)) => y.cmp(&x), // descending: older first
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                }
            }),
        }
        idx
    }

    pub(crate) fn clamp_cursor_to_visible(&mut self) {
        let n = self.sorted_indices().len();
        if n == 0 {
            self.cursor = 0;
        } else if self.cursor >= n {
            self.cursor = n - 1;
        }
    }

    pub fn drill_into(&mut self, children: Vec<Cache>) {
        let parent = std::mem::replace(&mut self.all, children);
        self.cursor_stack.push(self.cursor);
        self.stack.push(parent);
        self.cursor = 0;
        self.marks.clear(); // marks are index-keyed; reset on level change
        self.level_dirty = false;
    }

    pub fn drill_out(&mut self) {
        let _ = self.drill_out_with_path();
    }

    /// Same as `drill_out` but also returns the path of the cache we just
    /// left (for the event loop to trigger a refresh). Returns `None`
    /// when already at the top level.
    pub fn drill_out_with_path(&mut self) -> Option<PathBuf> {
        // Only pop drill_paths / cursor_stack when we actually pop the
        // stack — otherwise calling drill_out twice at the top level would
        // silently desync them from stack.len().
        if let Some(parent) = self.stack.pop() {
            self.all = parent;
            // Restore the cursor the user had when they drilled in. Clamp
            // against the new visible set in case a refresh shifted things.
            self.cursor = self.cursor_stack.pop().unwrap_or(0);
            self.marks.clear();
            self.stack_labels.pop();
            self.level_dirty = false;
            let popped = self.drill_paths.pop();
            self.clamp_cursor_to_visible();
            popped
        } else {
            None
        }
    }
}
