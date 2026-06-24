//! Pure `update(State, Msg) -> (State, Command<Effect, Msg>)`.

use super::command::Command;
use super::effect::Effect;
use super::filter::Filter;
use super::msg::Msg;
use super::state::{Loading, Modal, Overlay, RunOutcome, State};

/// Pure state-transition function.  Each arm returns the next `State` and a
/// `Command` that the runtime drains (synchronous events re-fed through
/// `update`; effects handed to `EffectRunner`).  This function spawns no
/// threads and performs no IO; `state.now` is the runtime's clock for any
/// score/age math.
///
/// **One pragmatic compromise:** the three arms that open the spinner modal
/// (`DrillIn`, `DrillOut` when dirty, `ConfirmDelete`) call
/// `std::time::Instant::now()` to seed `Loading::started`. That field only
/// drives the "Ns elapsed" line in the modal — no control flow depends on
/// it, so the function is observably pure for routing and assertions, but
/// `Loading::started` itself is non-deterministic across runs.
pub fn update(mut state: State, msg: Msg) -> (State, Command<Effect, Msg>) {
    match msg {
        Msg::MoveUp => {
            if state.focus_right {
                state.files_cursor = state.files_cursor.saturating_sub(1);
            } else if state.cursor > 0 {
                state.cursor -= 1;
            }
            (state, Command::done())
        }
        Msg::MoveDown => {
            if state.focus_right {
                let len = state
                    .sorted_indices()
                    .get(state.cursor)
                    .and_then(|&i| state.all.get(i))
                    .map(|c| c.top_files.len())
                    .unwrap_or(0);
                if state.files_cursor + 1 < len {
                    state.files_cursor += 1;
                }
            } else {
                let n = state.sorted_indices().len();
                if state.cursor + 1 < n {
                    state.cursor += 1;
                }
            }
            (state, Command::done())
        }
        Msg::ToggleMark => {
            let visible = state.sorted_indices();
            if let Some(&underlying) = visible.get(state.cursor) {
                let is_active = state.floor.is_active(state.all[underlying].age(state.now));
                if is_active && !state.marks.is_marked(underlying) {
                    state.modal = Modal::ActiveMark(vec![underlying]);
                } else {
                    state.marks.toggle(underlying);
                    if state.cursor + 1 < visible.len() {
                        state.cursor += 1;
                    }
                }
            }
            (state, Command::done())
        }
        Msg::MarkDownToCursor => {
            let visible = state.sorted_indices();
            let take = (state.cursor + 1).min(visible.len());
            let mut active_in_range = Vec::new();
            let mut benign = Vec::new();
            for &underlying in visible.iter().take(take) {
                if state.marks.is_marked(underlying) {
                    continue;
                }
                if state.floor.is_active(state.all[underlying].age(state.now)) {
                    active_in_range.push(underlying);
                } else {
                    benign.push(underlying);
                }
            }
            for i in benign {
                state.marks.marked.insert(i);
            }
            if !active_in_range.is_empty() {
                state.modal = Modal::ActiveMark(active_in_range);
            }
            (state, Command::done())
        }
        Msg::CycleSort => {
            // Pin the cursor to the underlying cache so a sort change feels
            // like a re-ordering of the same list, not a jump back to row 0
            // (which made --root entries look like they had disappeared).
            //
            // Exception: when the user is sitting on row 0 they're typically
            // eyeballing "the worst offender by this metric" — keep them
            // there across sorts so the cursor follows the ranking head,
            // not the cache that happened to be the head a sort ago.
            let was_top = state.cursor == 0;
            let pinned = state.sorted_indices().get(state.cursor).copied();
            state.sort = state.sort.next();
            state.cursor = if was_top {
                0
            } else {
                let visible = state.sorted_indices();
                pinned
                    .and_then(|i| visible.iter().position(|&v| v == i))
                    .unwrap_or(0)
            };
            (state, Command::done())
        }
        Msg::DrillIn => {
            // Ignore drill-in while a background scan/refresh/delete is in
            // flight — overwriting `loading` here would orphan the prior
            // worker's `ScanCompleted` and double-spawn IO.
            if state.loading.is_some() {
                return (state, Command::done());
            }
            let visible = state.sorted_indices();
            let Some(&idx) = visible.get(state.cursor) else {
                return (state, Command::done());
            };
            let parent_label = state.all[idx].label.clone();
            let parent_path = state.all[idx].path.clone();
            state.loading = Some(Loading {
                label: format!("scanning {parent_label}"),
                frame: 0,
                started: std::time::Instant::now(),
                folders: Some(0),
            });
            (
                state,
                Command::effect(Effect::SpawnScan {
                    parent_label,
                    parent_path,
                }),
            )
        }
        Msg::ScanCompleted {
            parent_label,
            parent_path,
            children,
        } => {
            if !children.is_empty() {
                state.stack_labels.push(parent_label);
                state.drill_paths.push(parent_path);
                state.drill_into(children);
            }
            state.loading = None;
            (state, Command::done())
        }
        Msg::ScanProgress { folders } => {
            if let Some(l) = state.loading.as_mut() {
                l.folders = Some(folders);
            }
            (state, Command::done())
        }
        Msg::SeedsLoaded { caches } => {
            // Top-level scan finished.  We are always at the root level when
            // this arrives (it's only fired once at startup), so replace
            // `state.all` directly rather than going through drill_into.
            state.all = caches;
            state.cursor = 0;
            state.loading = None;
            (state, Command::done())
        }
        Msg::DrillOut => {
            // Mirror DrillIn's guard. Drilling out while a delete/scan/refresh
            // is in flight would swap `state.all` for the parent level, and the
            // in-flight worker's `DeleteCompleted`/`RefreshCompleted` would
            // then index into the wrong list.
            if state.loading.is_some() {
                return (state, Command::done());
            }
            let was_dirty = state.level_dirty;
            let popped_path = state.drill_out_with_path();
            if was_dirty {
                if let Some(path) = popped_path {
                    let path_label = path
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| path.display().to_string());
                    state.loading = Some(Loading {
                        label: format!("refreshing {path_label}"),
                        frame: 0,
                        started: std::time::Instant::now(),
                        folders: None,
                    });
                    // Propagate the dirty signal up the stack: the size we're
                    // about to refresh is a child of the level we just
                    // exposed, so any level above it is now stale too.  Set
                    // before returning so the next DrillOut sees it.
                    state.level_dirty = true;
                    return (state, Command::effect(Effect::SpawnRefresh { path }));
                }
            }
            (state, Command::done())
        }
        Msg::RefreshCompleted { path, cache } => {
            if let Some(slot) = state.all.iter_mut().find(|c| c.path == path) {
                *slot = cache;
            }
            state.loading = None;
            (state, Command::done())
        }
        Msg::ToggleFocus => {
            state.focus_right = !state.focus_right;
            state.files_cursor = 0;
            (state, Command::done())
        }
        Msg::RequestQuit => {
            state.quit = true;
            (state, Command::done())
        }
        Msg::DeletePressed => {
            if state.marks.count() == 0 {
                return (state, Command::done());
            }
            state.modal = Modal::DeleteConfirm;
            if state.yes_mode {
                (state, Command::event(Msg::ConfirmDelete))
            } else {
                (state, Command::done())
            }
        }
        Msg::CancelDelete => {
            state.modal = Modal::None;
            (state, Command::done())
        }
        Msg::ConfirmDelete => {
            let to_delete: Vec<(usize, std::path::PathBuf, u64)> = state
                .marks
                .marked
                .iter()
                .filter_map(|&i| state.all.get(i).map(|c| (i, c.path.clone(), c.size_bytes)))
                .collect();
            state.modal = Modal::None;
            state.marks.clear();
            if to_delete.is_empty() {
                return (state, Command::done());
            }
            let count = to_delete.len();
            state.loading = Some(Loading {
                label: format!(
                    "deleting {count} {}",
                    crate::caches::format::pluralize(count as u64, "folder", "folders")
                ),
                frame: 0,
                started: std::time::Instant::now(),
                folders: None,
            });
            let dry_run = state.dry_run;
            (
                state,
                Command::effect(Effect::SpawnDelete {
                    items: to_delete,
                    dry_run,
                }),
            )
        }
        Msg::DeleteCompleted {
            freed,
            deleted_count,
            failed_count,
            deleted_indices,
        } => {
            state.total_freed += freed;
            if !state.dry_run && deleted_count > 0 {
                state.level_dirty = true;
            }
            if !state.dry_run {
                let mut idxs = deleted_indices;
                idxs.sort_unstable_by(|a, b| b.cmp(a));
                for i in idxs {
                    if i < state.all.len() {
                        state.all.remove(i);
                    }
                }
                // Clamp against the visible (filtered) set — `cursor` indexes
                // `sorted_indices()`, not `state.all` directly.
                state.clamp_cursor_to_visible();
            }
            state.loading = None;
            state.overlay = Some(Overlay {
                outcome: RunOutcome {
                    freed,
                    deleted: deleted_count,
                    failed: failed_count,
                    dry_run: state.dry_run,
                },
            });
            (
                state,
                Command::effect(Effect::EmitAfter {
                    dur: std::time::Duration::from_secs(2),
                    msg: Msg::OverlayDismiss,
                }),
            )
        }
        Msg::ConfirmActiveMark => {
            if let Modal::ActiveMark(indices) = std::mem::replace(&mut state.modal, Modal::None) {
                for i in indices {
                    state.marks.marked.insert(i);
                }
                let visible_len = state.sorted_indices().len();
                if state.cursor + 1 < visible_len {
                    state.cursor += 1;
                }
            }
            (state, Command::done())
        }
        Msg::CancelActiveMark => {
            state.modal = Modal::None;
            (state, Command::done())
        }
        Msg::FilterStart => {
            if state.filter.is_none() {
                state.filter = Some(Filter::default());
            }
            state.modal = Modal::FilterEdit;
            (state, Command::done())
        }
        Msg::FilterChar(c) => {
            if matches!(state.modal, Modal::FilterEdit) {
                if let Some(f) = state.filter.as_mut() {
                    f.input.push(c);
                }
            }
            state.clamp_cursor_to_visible();
            (state, Command::done())
        }
        Msg::FilterBackspace => {
            if matches!(state.modal, Modal::FilterEdit) {
                if let Some(f) = state.filter.as_mut() {
                    f.input.pop();
                }
            }
            state.clamp_cursor_to_visible();
            (state, Command::done())
        }
        Msg::FilterApply => {
            state.modal = Modal::None;
            if let Some(f) = state.filter.as_ref() {
                if f.input.is_empty() {
                    state.filter = None;
                }
            }
            state.clamp_cursor_to_visible();
            (state, Command::done())
        }
        Msg::FilterCancel => {
            state.filter = None;
            state.modal = Modal::None;
            state.clamp_cursor_to_visible();
            (state, Command::done())
        }
        Msg::MarkAllVisible => {
            let visible = state.sorted_indices();
            let mut active_in_range = Vec::new();
            for &underlying in &visible {
                if state.marks.is_marked(underlying) {
                    continue;
                }
                if state.floor.is_active(state.all[underlying].age(state.now)) {
                    active_in_range.push(underlying);
                } else {
                    state.marks.marked.insert(underlying);
                }
            }
            if !active_in_range.is_empty() {
                state.modal = Modal::ActiveMark(active_in_range);
            }
            (state, Command::done())
        }
        Msg::Tick => {
            if let Some(l) = state.loading.as_mut() {
                l.update_frame();
            }
            (state, Command::done())
        }
        Msg::OverlayDismiss => {
            state.overlay = None;
            (state, Command::done())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::caches::model::*;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};

    fn cache(label: &str, size: u64, mtime_secs: u64) -> Cache {
        Cache {
            label: label.into(),
            path: PathBuf::from(format!("/x/{label}")),
            size_bytes: size,
            newest_mtime: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(mtime_secs)),
            file_count: 1,
            dir_count: 0,
            top_files: Vec::new(),
            unreadable: 0,
        }
    }

    fn state(items: Vec<Cache>) -> State {
        State {
            now: SystemTime::UNIX_EPOCH + Duration::from_secs(10_000_000),
            all: items,
            sort: Sort::Score,
            marks: MarkSet::default(),
            cursor: 0,
            files_cursor: 0,
            floor: FloorPolicy {
                floor: Duration::from_secs(7 * 86_400),
            },
            focus_right: false,
            stack: Vec::new(),
            stack_labels: Vec::new(),
            quit: false,
            dry_run: false,
            yes_mode: false,
            total_freed: 0,
            modal: Modal::None,
            filter: None,
            loading: None,
            overlay: None,
            level_dirty: false,
            drill_paths: Vec::new(),
            cursor_stack: Vec::new(),
        }
    }

    #[test]
    fn move_up_decrements_and_floors_at_zero() {
        let mut s = state(vec![cache("a", 1, 0), cache("b", 1, 0), cache("c", 1, 0)]);
        s.cursor = 2;
        let (s, c) = update(s, Msg::MoveUp);
        assert!(c.is_done());
        assert_eq!(s.cursor, 1);
        let (s, _) = update(s, Msg::MoveUp);
        assert_eq!(s.cursor, 0);
        let (s, _) = update(s, Msg::MoveUp);
        assert_eq!(s.cursor, 0, "cursor must not underflow");
    }

    #[test]
    fn mark_down_to_cursor_marks_benign_range() {
        // All caches very old (mtime 0, now=10M) → benign per default floor.
        let mut s = state(vec![cache("a", 1, 0), cache("b", 1, 0), cache("c", 1, 0)]);
        s.cursor = 1;
        let (s, c) = update(s, Msg::MarkDownToCursor);
        assert!(c.is_done());
        assert!(s.marks.is_marked(0));
        assert!(s.marks.is_marked(1));
        assert!(!s.marks.is_marked(2));
        assert!(matches!(s.modal, Modal::None));
    }

    #[test]
    fn mark_down_to_cursor_defers_active_rows_to_modal() {
        // Mix of active (recent) and benign (old) rows.
        let mut s = state(vec![
            cache("recent", 1, NOW_SECS - 86_400), // active
            cache("old", 1, 0),                    // benign
        ]);
        s.cursor = 1;
        let (s, c) = update(s, Msg::MarkDownToCursor);
        assert!(c.is_done());
        assert!(s.marks.is_marked(1), "benign row marked immediately");
        assert!(!s.marks.is_marked(0), "active row deferred to modal");
        assert!(matches!(s.modal, Modal::ActiveMark(_)));
    }

    #[test]
    fn filter_backspace_pops_last_char_while_editing() {
        let s = state(vec![cache("a", 1, 0)]);
        let (s, _) = update(s, Msg::FilterStart);
        let (s, _) = update(s, Msg::FilterChar('y'));
        let (s, _) = update(s, Msg::FilterChar('a'));
        let (s, _) = update(s, Msg::FilterChar('r'));
        let (s, _) = update(s, Msg::FilterBackspace);
        let (s, c) = update(s, Msg::FilterBackspace);
        assert!(c.is_done());
        assert_eq!(s.filter.as_ref().unwrap().input, "y");
    }

    #[test]
    fn filter_backspace_is_noop_when_not_in_edit_mode() {
        // FilterApply with input drops the filter from edit mode but keeps it
        // applied.  A backspace in `Modal::None` must NOT mutate input.
        let s = state(vec![cache("a", 1, 0)]);
        let (s, _) = update(s, Msg::FilterStart);
        let (s, _) = update(s, Msg::FilterChar('a'));
        let (s, _) = update(s, Msg::FilterApply);
        assert!(matches!(s.modal, Modal::None));
        let (s, _) = update(s, Msg::FilterBackspace);
        assert_eq!(s.filter.as_ref().unwrap().input, "a", "guarded by modal");
    }

    #[test]
    fn move_down_advances_until_last() {
        let s = state(vec![cache("a", 1, 0), cache("b", 1, 0)]);
        let (s, c) = update(s, Msg::MoveDown);
        assert!(c.is_done());
        assert_eq!(s.cursor, 1);
        let (s, c) = update(s, Msg::MoveDown);
        assert!(c.is_done());
        assert_eq!(s.cursor, 1);
    }

    #[test]
    fn move_down_respects_active_filter_bound() {
        // 3 rows; filter to only "yarn" — `sorted_indices` shrinks to one row.
        // `MoveDown` must not advance the cursor past the visible set.
        let s = state(vec![
            cache("npm", 1, 0),
            cache("yarn", 1, 0),
            cache("bun", 1, 0),
        ]);
        let (s, _) = update(s, Msg::FilterStart);
        let (s, _) = update(s, Msg::FilterChar('y'));
        let (s, _) = update(s, Msg::FilterChar('a'));
        let (s, _) = update(s, Msg::FilterApply);
        assert_eq!(s.cursor, 0);
        let (s, c) = update(s, Msg::MoveDown);
        assert!(c.is_done());
        assert_eq!(s.cursor, 0, "cursor must stay inside the visible set");
    }

    #[test]
    fn cycle_sort_pins_cursor_to_underlying_cache() {
        // Two caches with different scores so Score-sort and Size-sort
        // produce different row orders.  Cursor is on "small" in the
        // Score view; after CycleSort the cursor must still point at
        // "small" (now at a different visible row), not snap to row 0.
        let s = state(vec![
            cache("small", 1024, 9_000_000),
            cache("huge", 1_000_000_000, 9_999_990),
        ]);
        // Score sort (default): huge wins → visible = [huge, small].
        let mut s = s;
        s.cursor = 1; // pointing at "small"
        let underlying_small = s.sorted_indices()[1];
        let (s, c) = update(s, Msg::CycleSort);
        assert!(c.is_done());
        assert_eq!(s.sort, Sort::Size);
        // Size sort: huge still wins → visible = [huge, small].
        let visible = s.sorted_indices();
        assert_eq!(
            visible[s.cursor], underlying_small,
            "cursor must still point at 'small' after sort change"
        );
    }

    #[test]
    fn cycle_sort_keeps_top_row_pinned() {
        // Cursor on row 0 should stay on row 0 across sort cycles —
        // following the ranking head, not the cache that happened to lead
        // under the previous sort.  Tuned so Score and Size genuinely pick
        // different leaders:
        //   A: 10 MiB,  10 days old → score ≈ 100
        //   B: 100 MiB, 0.5 days old → score ≈ 50, but Size leader.
        let mut s = state(vec![
            cache("old_smaller", 10 * 1_048_576, 10_000_000 - 864_000),
            cache("recent_huge", 100 * 1_048_576, 10_000_000 - 43_200),
        ]);
        s.cursor = 0;
        let head_under_score = s.sorted_indices()[0];
        let (s, _) = update(s, Msg::CycleSort); // Score → Size
        let head_under_size = s.sorted_indices()[0];
        assert_ne!(
            head_under_score, head_under_size,
            "fixture must make the two metrics disagree, else the test is vacuous"
        );
        assert_eq!(s.cursor, 0, "row 0 must stay row 0 across sort cycles");
    }

    #[test]
    fn cycle_sort_resets_cursor_when_pin_unreachable() {
        // Sanity: when the previous cursor was out of bounds, fall back to 0.
        let mut s = state(vec![cache("a", 1, 0), cache("b", 1, 0)]);
        s.cursor = 7;
        let (s, _) = update(s, Msg::CycleSort);
        assert_eq!(s.cursor, 0);
    }

    #[test]
    fn quit_without_marks_quits_immediately() {
        let s = state(vec![cache("a", 1, 0)]);
        let (s, c) = update(s, Msg::RequestQuit);
        assert!(c.is_done());
        assert!(s.quit);
        assert!(matches!(s.modal, Modal::None));
    }

    #[test]
    fn quit_with_marks_also_quits_immediately() {
        let mut s = state(vec![cache("a", 1, 0)]);
        s.marks.toggle(0);
        let (s, c) = update(s, Msg::RequestQuit);
        assert!(c.is_done());
        assert!(s.quit);
        assert!(matches!(s.modal, Modal::None));
    }

    #[test]
    fn sorted_indices_score_descending() {
        let s = state(vec![
            cache("small", 1024, 9_000_000),
            cache("huge", 1_000_000_000, 9_999_990),
        ]);
        let idx = s.sorted_indices();
        // size_MB * cold_days: huge wins by a wide margin
        assert_eq!(s.all[idx[0]].label, "huge");
    }

    #[test]
    fn drill_in_replaces_list_pushes_stack() {
        let mut s = state(vec![cache("npm", 100, 0)]);
        s.cursor = 0;
        let children = vec![cache("registry", 10, 0), cache("cache", 5, 0)];
        s.drill_into(children);
        assert_eq!(s.all.len(), 2);
        assert_eq!(s.stack.len(), 1);
        assert_eq!(s.cursor, 0);
    }

    #[test]
    fn drill_out_restores_parent() {
        let mut s = state(vec![cache("npm", 100, 0)]);
        s.drill_into(vec![cache("registry", 10, 0)]);
        s.drill_out();
        assert_eq!(s.all.len(), 1);
        assert_eq!(s.stack.len(), 0);
        assert_eq!(s.all[0].label, "npm");
    }

    #[test]
    fn drill_out_at_top_is_noop() {
        let mut s = state(vec![cache("npm", 100, 0)]);
        s.drill_out();
        assert_eq!(s.all.len(), 1);
        assert_eq!(s.stack.len(), 0);
    }

    #[test]
    fn delete_pressed_opens_modal_when_marks_present() {
        let mut s = state(vec![cache("a", 1, 0)]);
        s.marks.toggle(0);
        let (s, c) = update(s, Msg::DeletePressed);
        assert!(c.is_done());
        assert!(matches!(s.modal, Modal::DeleteConfirm));
    }

    #[test]
    fn delete_pressed_noop_when_no_marks() {
        let s = state(vec![cache("a", 1, 0)]);
        let (s, c) = update(s, Msg::DeletePressed);
        assert!(c.is_done());
        assert!(matches!(s.modal, Modal::None));
    }

    #[test]
    fn cancel_delete_closes_modal() {
        let mut s = state(vec![cache("a", 1, 0)]);
        s.marks.toggle(0);
        let (s, c) = update(s, Msg::DeletePressed);
        assert!(c.is_done());
        let (s, c) = update(s, Msg::CancelDelete);
        assert!(c.is_done());
        assert!(matches!(s.modal, Modal::None));
    }

    #[test]
    fn confirm_delete_with_no_marks_returns_done() {
        let s = state(vec![cache("a", 1, 0)]);
        let (s, cmd) = update(s, Msg::ConfirmDelete);
        assert!(s.loading.is_none());
        assert!(cmd.is_done());
        assert!(matches!(s.modal, Modal::None));
    }

    #[test]
    fn confirm_delete_with_marks_emits_spawn_delete_and_sets_loading() {
        let mut s = state(vec![cache("a", 1, 0)]);
        s.marks.marked.insert(0);
        let (s, cmd) = update(s, Msg::ConfirmDelete);
        assert!(s.loading.is_some());
        assert_eq!(s.marks.count(), 0, "marks cleared on confirm");
        assert!(matches!(
            cmd.effects.as_slice(),
            [Effect::SpawnDelete { .. }]
        ));
    }

    #[test]
    fn delete_completed_real_run_removes_rows_and_accumulates_freed() {
        let mut s = state(vec![cache("a", 1, 0), cache("b", 2, 0), cache("c", 3, 0)]);
        s.dry_run = false;
        s.cursor = 2;
        let (s, cmd) = update(
            s,
            Msg::DeleteCompleted {
                freed: 5,
                deleted_count: 2,
                failed_count: 0,
                deleted_indices: vec![0, 2],
            },
        );
        assert_eq!(s.all.len(), 1);
        assert_eq!(s.all[0].label, "b");
        assert_eq!(
            s.cursor, 0,
            "cursor parked at min(deleted_indices), clamped to survivor"
        );
        assert_eq!(s.total_freed, 5);
        assert!(s.level_dirty);
        assert!(s.overlay.is_some());
        assert!(matches!(
            cmd.effects.as_slice(),
            [Effect::EmitAfter {
                msg: Msg::OverlayDismiss,
                ..
            }]
        ));
    }

    #[test]
    fn delete_completed_dry_run_keeps_rows_intact() {
        let mut s = state(vec![cache("a", 1, 0), cache("b", 2, 0)]);
        s.dry_run = true;
        let (s, cmd) = update(
            s,
            Msg::DeleteCompleted {
                freed: 3,
                deleted_count: 2,
                failed_count: 0,
                deleted_indices: vec![0, 1],
            },
        );
        assert_eq!(s.all.len(), 2, "dry-run leaves rows in view");
        assert_eq!(s.total_freed, 3);
        assert!(!s.level_dirty);
        assert!(s.overlay.is_some());
        assert!(matches!(
            cmd.effects.as_slice(),
            [Effect::EmitAfter {
                msg: Msg::OverlayDismiss,
                ..
            }]
        ));
    }

    #[test]
    fn delete_completed_clamps_cursor_against_visible_under_filter() {
        // 3 rows; filter to only "yarn" — visible set has 1 row, cursor=0.
        // Delete that row from `state.all` (real run). After removal the
        // visible set is empty; cursor must clamp to 0, not point past it.
        let s = state(vec![
            cache("npm", 1, 0),
            cache("yarn", 1, 0),
            cache("bun", 1, 0),
        ]);
        let (s, _) = update(s, Msg::FilterStart);
        let (s, _) = update(s, Msg::FilterChar('y'));
        let (s, _) = update(s, Msg::FilterChar('a'));
        let (mut s, _) = update(s, Msg::FilterApply);
        s.dry_run = false;
        let (s, _) = update(
            s,
            Msg::DeleteCompleted {
                freed: 1,
                deleted_count: 1,
                failed_count: 0,
                deleted_indices: vec![1],
            },
        );
        assert_eq!(s.all.len(), 2);
        assert_eq!(s.sorted_indices().len(), 0, "filter still matches nothing");
        assert_eq!(s.cursor, 0, "cursor clamped against visible bound");
    }

    #[test]
    fn delete_completed_carries_failed_count_to_overlay() {
        let mut s = state(vec![cache("a", 1, 0), cache("b", 1, 0)]);
        s.dry_run = false;
        let (s, _) = update(
            s,
            Msg::DeleteCompleted {
                freed: 1,
                deleted_count: 1,
                failed_count: 1,
                deleted_indices: vec![0],
            },
        );
        let outcome = &s.overlay.as_ref().unwrap().outcome;
        assert_eq!(outcome.failed, 1);
        assert_eq!(outcome.deleted, 1);
    }

    #[test]
    fn delete_completed_sets_overlay_and_emits_dismiss_after_2s() {
        let mut s = state(vec![cache("a", 1, 0)]);
        s.dry_run = false;
        let (s, cmd) = update(
            s,
            Msg::DeleteCompleted {
                freed: 100,
                deleted_count: 1,
                failed_count: 0,
                deleted_indices: vec![0],
            },
        );
        assert!(s.overlay.is_some());
        assert_eq!(s.overlay.as_ref().unwrap().outcome.freed, 100);
        assert!(matches!(
            cmd.effects.as_slice(),
            [Effect::EmitAfter { dur, msg: Msg::OverlayDismiss }] if *dur == std::time::Duration::from_secs(2)
        ));
    }

    #[test]
    fn overlay_dismiss_clears_overlay() {
        let mut s = state(vec![cache("a", 1, 0)]);
        s.overlay = Some(Overlay {
            outcome: RunOutcome {
                freed: 1,
                deleted: 1,
                failed: 0,
                dry_run: false,
            },
        });
        let (s, cmd) = update(s, Msg::OverlayDismiss);
        assert!(s.overlay.is_none());
        assert!(cmd.is_done());
    }

    #[test]
    fn mark_survives_sort_change() {
        // Two caches; their relative score order differs from name order.
        // size_MB * cold_days:
        //   "huge"  = 1000MB * 0.0001d ≈ 0.1
        //   "small" = 0.001MB * 11.57d ≈ 0.01
        // Sorted by Score (default): huge, small.
        // Sorted by Size: huge, small (same).
        // Sorted by Age: small, huge (small is older).
        let s = state(vec![
            cache("small", 1024, 9_000_000),
            cache("huge", 1_000_000_000, 9_999_990),
        ]);
        // Cursor on row 0 = "huge" in Score sort.
        let idx_huge_before = s.sorted_indices()[0];
        let (s, c) = update(s, Msg::ToggleMark);
        assert!(c.is_done());
        // "huge" is ACTIVE (mtime 10s before NOW), so a confirm modal opens.
        let s = if matches!(s.modal, Modal::ActiveMark(_)) {
            let (s, c) = update(s, Msg::ConfirmActiveMark);
            assert!(c.is_done());
            s
        } else {
            s
        };
        assert!(
            s.marks.is_marked(idx_huge_before),
            "after toggling cursor on huge, marks must store huge's underlying index"
        );

        // Switch to Age sort — huge moves to row 1, but should remain marked.
        let (s, c) = update(s, Msg::CycleSort); // Score -> Size
        assert!(c.is_done());
        let (s, c) = update(s, Msg::CycleSort); // Size  -> Age
        assert!(c.is_done());
        let visible = s.sorted_indices();
        let row_of_huge = visible
            .iter()
            .position(|&i| s.all[i].label == "huge")
            .unwrap();
        assert!(
            s.marks.is_marked(visible[row_of_huge]),
            "after sort change, the SAME underlying cache should still be marked"
        );
    }

    #[test]
    fn drill_out_msg_pops_stack() {
        let mut s = state(vec![cache("parent", 100, 0)]);
        s.drill_into(vec![cache("child", 10, 0)]);
        assert_eq!(s.all[0].label, "child");

        let (s, c) = update(s, Msg::DrillOut);
        assert!(c.is_done());
        assert_eq!(s.all.len(), 1);
        assert_eq!(s.all[0].label, "parent");
    }

    #[test]
    fn drill_in_via_scan_enumerates_children() {
        use std::fs;
        let tmp = tempfile::tempdir().unwrap();
        let cache = tmp.path().join("npm");
        fs::create_dir_all(cache.join("a")).unwrap();
        fs::create_dir_all(cache.join("b")).unwrap();

        // Build a State whose cursor points at the cache, then simulate drill-in.
        let mut s = state(vec![Cache {
            label: "npm".into(),
            path: cache.clone(),
            size_bytes: 0,
            newest_mtime: None,
            file_count: 0,
            dir_count: 0,
            top_files: Vec::new(),
            unreadable: 0,
        }]);
        let children = crate::caches::scan::enumerate_seed(&cache);
        s.drill_into(children);
        assert_eq!(s.all.len(), 2);
        assert!(s.stack.len() == 1);
    }

    #[test]
    fn empty_caches_sort_last_under_age() {
        let mut s = state(vec![
            cache("populated", 1024, 0), // very old
            cache("empty", 0, 0),        // we'll null its mtime below
        ]);
        s.all[1].newest_mtime = None;
        s.sort = Sort::Age;
        let idx = s.sorted_indices();
        assert_eq!(
            s.all[idx.last().copied().unwrap()].label,
            "empty",
            "empty caches must land at the bottom under Age sort"
        );
    }

    const NOW_SECS: u64 = 10_000_000;

    #[test]
    fn marking_active_row_opens_active_confirm() {
        let s = state(vec![cache("recent", 1_000_000, NOW_SECS - 86_400)]);
        let (s, c) = update(s, Msg::ToggleMark);
        assert!(c.is_done());
        assert!(matches!(s.modal, Modal::ActiveMark(_)));
        assert_eq!(s.marks.count(), 0);
    }

    #[test]
    fn confirm_active_mark_inserts_and_closes() {
        let s = state(vec![cache("recent", 1_000_000, NOW_SECS - 86_400)]);
        let (s, c) = update(s, Msg::ToggleMark);
        assert!(c.is_done());
        let (s, c) = update(s, Msg::ConfirmActiveMark);
        assert!(c.is_done());
        assert_eq!(s.marks.count(), 1);
        assert!(matches!(s.modal, Modal::None));
    }

    #[test]
    fn cancel_active_mark_closes_without_inserting() {
        let s = state(vec![cache("recent", 1_000_000, NOW_SECS - 86_400)]);
        let (s, c) = update(s, Msg::ToggleMark);
        assert!(c.is_done());
        let (s, c) = update(s, Msg::CancelActiveMark);
        assert!(c.is_done());
        assert_eq!(s.marks.count(), 0);
        assert!(matches!(s.modal, Modal::None));
    }

    #[test]
    fn filter_start_creates_editing_filter() {
        let s = state(vec![cache("a", 1, 0)]);
        let (s, c) = update(s, Msg::FilterStart);
        assert!(c.is_done());
        let f = s.filter.as_ref().unwrap();
        assert!(matches!(s.modal, Modal::FilterEdit));
        assert_eq!(f.input, "");
    }

    #[test]
    fn filter_chars_accumulate() {
        let s = state(vec![cache("a", 1, 0)]);
        let (s, c) = update(s, Msg::FilterStart);
        assert!(c.is_done());
        let (s, c) = update(s, Msg::FilterChar('n'));
        assert!(c.is_done());
        let (s, c) = update(s, Msg::FilterChar('p'));
        assert!(c.is_done());
        let (s, c) = update(s, Msg::FilterChar('m'));
        assert!(c.is_done());
        assert_eq!(s.filter.as_ref().unwrap().input, "npm");
    }

    #[test]
    fn filter_apply_closes_editing() {
        let s = state(vec![cache("a", 1, 0)]);
        let (s, c) = update(s, Msg::FilterStart);
        assert!(c.is_done());
        let (s, c) = update(s, Msg::FilterChar('a'));
        assert!(c.is_done());
        let (s, c) = update(s, Msg::FilterApply);
        assert!(c.is_done());
        assert!(matches!(s.modal, Modal::None));
        let f = s.filter.as_ref().unwrap();
        assert_eq!(f.input, "a");
    }

    #[test]
    fn filter_cancel_drops_filter() {
        let s = state(vec![cache("a", 1, 0)]);
        let (s, c) = update(s, Msg::FilterStart);
        assert!(c.is_done());
        let (s, c) = update(s, Msg::FilterChar('a'));
        assert!(c.is_done());
        let (s, c) = update(s, Msg::FilterCancel);
        assert!(c.is_done());
        assert!(s.filter.is_none());
    }

    #[test]
    fn empty_filter_apply_drops_filter() {
        let s = state(vec![cache("a", 1, 0)]);
        let (s, c) = update(s, Msg::FilterStart);
        assert!(c.is_done());
        let (s, c) = update(s, Msg::FilterApply);
        assert!(c.is_done());
        assert!(s.filter.is_none());
    }

    #[test]
    fn filter_hides_non_matching_rows() {
        let s = state(vec![
            cache("npm", 1, 0),
            cache("yarn", 1, 0),
            cache("bun", 1, 0),
        ]);
        let (s, c) = update(s, Msg::FilterStart);
        assert!(c.is_done());
        let (s, c) = update(s, Msg::FilterChar('y'));
        assert!(c.is_done());
        let visible = s.sorted_indices();
        let labels: Vec<&str> = visible.iter().map(|&i| s.all[i].label.as_str()).collect();
        assert_eq!(labels, ["yarn"]);
    }

    #[test]
    fn mark_all_visible_marks_filtered_rows() {
        let s = state(vec![
            cache("npm", 1, 0),
            cache("yarn", 1, 0),
            cache("bun", 1, 0),
        ]);
        // Filter for "rn" — only "yarn" contains it.
        let (s, c) = update(s, Msg::FilterStart);
        assert!(c.is_done());
        let (s, c) = update(s, Msg::FilterChar('r'));
        assert!(c.is_done());
        let (s, c) = update(s, Msg::FilterChar('n'));
        assert!(c.is_done());
        let (s, c) = update(s, Msg::FilterApply);
        assert!(c.is_done());
        let (s, c) = update(s, Msg::MarkAllVisible);
        assert!(c.is_done());
        assert_eq!(s.marks.count(), 1);
        // Clear filter (Cancel drops the whole Filter struct); now MarkAllVisible
        // covers every row. yarn is already marked, so two new marks land.
        let (s, c) = update(s, Msg::FilterCancel);
        assert!(c.is_done());
        let (s, c) = update(s, Msg::MarkAllVisible);
        assert!(c.is_done());
        assert_eq!(s.marks.count(), 3);
    }

    #[test]
    fn scrolling_right_pane_advances_files_selection() {
        let s = state(vec![Cache {
            label: "x".into(),
            path: PathBuf::from("/x"),
            size_bytes: 0,
            newest_mtime: None,
            file_count: 0,
            dir_count: 0,
            top_files: vec![
                TopFile {
                    name: "a".into(),
                    size_bytes: 1,
                    mtime: None,
                },
                TopFile {
                    name: "b".into(),
                    size_bytes: 1,
                    mtime: None,
                },
                TopFile {
                    name: "c".into(),
                    size_bytes: 1,
                    mtime: None,
                },
            ],
            unreadable: 0,
        }]);
        let (s, c) = update(s, Msg::ToggleFocus);
        assert!(c.is_done());
        assert!(s.focus_right);
        assert_eq!(s.files_cursor, 0);
        let (s, c) = update(s, Msg::MoveDown);
        assert!(c.is_done());
        assert_eq!(s.files_cursor, 1);
        let (s, c) = update(s, Msg::MoveDown);
        assert!(c.is_done());
        assert_eq!(s.files_cursor, 2);
        let (s, c) = update(s, Msg::MoveDown);
        assert!(c.is_done());
        assert_eq!(s.files_cursor, 2);
    }

    #[test]
    fn toggle_focus_resets_scroll() {
        let mut s = state(vec![cache("x", 1, 0)]);
        s.focus_right = true;
        s.files_cursor = 5;
        let (s, c) = update(s, Msg::ToggleFocus);
        assert!(c.is_done());
        assert_eq!(s.files_cursor, 0);
        assert!(!s.focus_right);
    }

    #[test]
    fn drill_in_is_noop_while_loading() {
        let mut s = state(vec![cache("npm", 100, 0)]);
        let started = std::time::Instant::now();
        s.loading = Some(Loading {
            label: "scanning previous".into(),
            frame: 7,
            started,
            folders: None,
        });
        let (s, cmd) = update(s, Msg::DrillIn);
        assert!(cmd.is_done(), "no second scan must be emitted");
        let l = s.loading.as_ref().expect("loading preserved");
        assert_eq!(l.label, "scanning previous");
        assert_eq!(l.frame, 7);
        assert_eq!(l.started, started);
    }

    #[test]
    fn loading_frame_advances() {
        let mut l = Loading {
            label: "x".into(),
            frame: 0,
            started: std::time::Instant::now(),
            folders: None,
        };
        l.update_frame();
        assert_eq!(l.frame, 1);
        for _ in 0..super::super::SPINNER_FRAMES.len() {
            l.update_frame();
        }
        // Wraps around back to 1 after one full cycle from 1.
        assert_eq!(l.frame, 1);
    }

    #[test]
    fn tick_advances_spinner_frame_when_loading() {
        let mut s = state(vec![cache("a", 1, 0)]);
        s.loading = Some(Loading {
            label: "x".into(),
            frame: 0,
            started: std::time::Instant::now(),
            folders: None,
        });
        let (s, c) = update(s, Msg::Tick);
        assert!(c.is_done());
        assert_eq!(s.loading.as_ref().unwrap().frame, 1);
    }

    #[test]
    fn tick_is_noop_when_not_loading() {
        let s = state(vec![cache("a", 1, 0)]);
        let (s, c) = update(s, Msg::Tick);
        assert!(c.is_done());
        assert!(s.loading.is_none());
    }

    #[test]
    fn space_toggle_advances_cursor() {
        let s = state(vec![cache("a", 1, 0), cache("b", 1, 0), cache("c", 1, 0)]);
        let (s, c) = update(s, Msg::ToggleMark);
        assert!(c.is_done());
        assert_eq!(s.cursor, 1, "cursor should advance after Space");
        let (s, c) = update(s, Msg::ToggleMark);
        assert!(c.is_done());
        assert_eq!(s.cursor, 2);
        let (s, c) = update(s, Msg::ToggleMark);
        assert!(c.is_done());
        // Already on last row — should not overflow.
        assert_eq!(s.cursor, 2);
    }

    #[test]
    fn drill_out_with_path_returns_popped_path() {
        let mut s = state(vec![cache("npm", 100, 0)]);
        s.drill_paths.push(std::path::PathBuf::from("/x/npm"));
        s.drill_into(vec![cache("registry", 10, 0)]);
        let popped = s.drill_out_with_path();
        assert_eq!(popped, Some(std::path::PathBuf::from("/x/npm")));
    }

    #[test]
    fn drill_out_with_path_at_top_returns_none() {
        let mut s = state(vec![cache("npm", 100, 0)]);
        assert_eq!(s.drill_out_with_path(), None);
    }

    #[test]
    fn level_dirty_resets_on_drill_in() {
        let mut s = state(vec![cache("npm", 100, 0)]);
        s.level_dirty = true;
        s.drill_into(vec![cache("registry", 10, 0)]);
        assert!(!s.level_dirty);
    }

    #[test]
    fn level_dirty_resets_on_drill_out() {
        let mut s = state(vec![cache("npm", 100, 0)]);
        s.drill_into(vec![cache("registry", 10, 0)]);
        s.level_dirty = true;
        s.drill_out_with_path();
        assert!(!s.level_dirty);
    }

    #[test]
    fn drilldown_delete_drillout_refreshes_parent() {
        // End-to-end: drill into npm, delete a child, drill out — the parent
        // entry in state.all must trigger a refresh against its original path.
        let mut s = state(vec![cache("npm", 100, 0), cache("cargo", 50, 0)]);
        s.cursor = 0;
        s.dry_run = false;

        // 1. DrillIn on "npm" — emits SpawnScan with the cache's path.
        let (s, cmd) = update(s, Msg::DrillIn);
        let parent_path = match cmd.effects.as_slice() {
            [Effect::SpawnScan { parent_path, .. }] => parent_path.clone(),
            other => panic!("expected SpawnScan, got {other:?}"),
        };
        assert_eq!(parent_path, std::path::PathBuf::from("/x/npm"));

        // 2. ScanCompleted with two children — drills in, pushes drill_paths.
        let (s, _) = update(
            s,
            Msg::ScanCompleted {
                parent_label: "npm".into(),
                parent_path: parent_path.clone(),
                children: vec![cache("registry", 60, 0), cache("logs", 40, 0)],
            },
        );
        assert_eq!(s.all.len(), 2);
        assert_eq!(s.stack.len(), 1);
        assert_eq!(s.drill_paths.last(), Some(&parent_path));
        assert!(!s.level_dirty, "fresh level starts clean");

        // 3. Mark + confirm-delete one child.
        let mut s = s;
        s.marks.marked.insert(0);
        let (s, cmd) = update(s, Msg::ConfirmDelete);
        assert!(matches!(
            cmd.effects.as_slice(),
            [Effect::SpawnDelete { .. }]
        ));

        // 4. Worker comes back — delete succeeded, level_dirty must be set.
        let (s, _) = update(
            s,
            Msg::DeleteCompleted {
                freed: 60,
                deleted_count: 1,
                failed_count: 0,
                deleted_indices: vec![0],
            },
        );
        assert!(
            s.level_dirty,
            "DeleteCompleted on a real run must mark the level dirty"
        );
        assert!(s.loading.is_none(), "delete spinner cleared");
        assert!(s.overlay.is_some(), "overlay shown");

        // 5. DrillOut while loading is None — must emit SpawnRefresh for the
        //    original parent path so the top-level npm row gets re-stat'd.
        let (s, cmd) = update(s, Msg::DrillOut);
        match cmd.effects.as_slice() {
            [Effect::SpawnRefresh { path }] => {
                assert_eq!(*path, parent_path, "refresh target must be /x/npm");
            }
            other => panic!("expected SpawnRefresh, got {other:?}"),
        }
        assert_eq!(s.all[0].label, "npm");
        assert!(s.loading.is_some(), "refresh spinner shown");

        // 6. RefreshCompleted with the new (smaller) cache.
        let updated = Cache {
            label: "npm".into(),
            path: parent_path.clone(),
            size_bytes: 40,
            newest_mtime: None,
            file_count: 0,
            dir_count: 0,
            top_files: Vec::new(),
            unreadable: 0,
        };
        let (s, _) = update(
            s,
            Msg::RefreshCompleted {
                path: parent_path,
                cache: updated,
            },
        );
        assert_eq!(
            s.all[0].size_bytes, 40,
            "parent row must reflect post-delete size"
        );
        assert!(s.loading.is_none());
    }

    #[test]
    fn drilldown_delete_propagates_dirty_up_the_full_stack() {
        // 3 levels deep, delete at the bottom, drill out twice — both
        // drill-outs must fire a refresh.  Before the fix, level_dirty was
        // reset by drill_out_with_path on the way up, so the second
        // drill-out (back to the top) silently skipped the re-stat.
        let mut s = state(vec![cache("npm", 100, 0)]);
        // Drill into L1.
        s.drill_paths.push(std::path::PathBuf::from("/x/npm"));
        s.drill_into(vec![cache("registry", 60, 0)]);
        // Drill into L2.
        s.drill_paths
            .push(std::path::PathBuf::from("/x/npm/registry"));
        s.drill_into(vec![cache("v1", 30, 0), cache("v2", 30, 0)]);
        // Delete at L2 — marks level_dirty on this level.
        s.level_dirty = true;

        // L2 → L1: must emit SpawnRefresh for /x/npm/registry.
        let (mut s, cmd) = update(s, Msg::DrillOut);
        assert!(matches!(
            cmd.effects.as_slice(),
            [Effect::SpawnRefresh { path }] if path == &std::path::PathBuf::from("/x/npm/registry")
        ));
        assert!(s.level_dirty, "L1 inherits dirtiness from the propagation");
        assert!(s.loading.is_some());
        // Worker reply — clears loading but keeps level_dirty true.
        s.loading = None;

        // L1 → L0: must ALSO emit SpawnRefresh, now for /x/npm.
        let (s, cmd) = update(s, Msg::DrillOut);
        assert!(matches!(
            cmd.effects.as_slice(),
            [Effect::SpawnRefresh { path }] if path == &std::path::PathBuf::from("/x/npm")
        ));
        assert!(s.loading.is_some());
    }

    #[test]
    fn drill_out_restores_cursor_to_pre_drill_position() {
        // Cursor was on row 3 at the top level; drill in, then back out —
        // we should land back on row 3, not row 0.
        let mut s = state(vec![
            cache("a", 1, 0),
            cache("b", 1, 0),
            cache("c", 1, 0),
            cache("npm", 100, 0),
            cache("e", 1, 0),
        ]);
        s.cursor = 3; // pointing at "npm" (idx 3 in state.all, sort=Score)
        s.drill_into(vec![cache("registry", 10, 0), cache("logs", 5, 0)]);
        assert_eq!(s.cursor, 0, "drill_into resets cursor to 0 in the child");
        s.drill_out();
        assert_eq!(
            s.cursor, 3,
            "drill_out must restore the cursor the user had on the parent"
        );
    }

    #[test]
    fn drill_out_clamps_restored_cursor_to_visible() {
        // Parent had 5 rows, cursor on row 4. While drilled in, a refresh
        // could in principle shrink the parent (we simulate by replacing
        // state.all before drilling out). The restore must clamp instead
        // of leaving cursor out of bounds.
        let mut s = state(vec![cache("a", 1, 0), cache("b", 1, 0)]);
        s.cursor = 1;
        s.drill_into(vec![cache("x", 1, 0)]);
        // Pretend something replaced the parent vec mid-drill (e.g. external
        // edit). When we drill out, the saved cursor (1) is valid against
        // the 1-row replacement only after clamping.
        if let Some(parent) = s.stack.last_mut() {
            *parent = vec![cache("a", 1, 0)];
        }
        s.drill_out();
        assert_eq!(s.cursor, 0, "cursor must clamp into the restored vec");
    }

    #[test]
    fn drill_in_clears_marks() {
        let mut s = state(vec![cache("npm", 100, 0)]);
        s.marks.toggle(0);
        s.drill_into(vec![cache("a", 1, 0)]);
        assert_eq!(s.marks.count(), 0);
    }

    #[test]
    fn drill_in_emits_scan_effect_and_sets_loading() {
        let s = state(vec![cache("npm", 100, 0)]);
        let (s, cmd) = update(s, Msg::DrillIn);
        let l = s.loading.as_ref().expect("loading set");
        assert_eq!(
            l.folders,
            Some(0),
            "drill-in spinner seeds the same folder-count UX as LoadSeeds"
        );
        assert!(matches!(cmd.effects.as_slice(), [Effect::SpawnScan { .. }]));
    }

    #[test]
    fn drill_in_with_empty_list_is_noop() {
        let s = state(Vec::new());
        let (s, cmd) = update(s, Msg::DrillIn);
        assert!(s.loading.is_none());
        assert!(cmd.is_done());
    }

    #[test]
    fn scan_completed_drills_into_children() {
        let s = state(vec![cache("npm", 100, 0)]);
        let (s, cmd) = update(
            s,
            Msg::ScanCompleted {
                parent_label: "npm".into(),
                parent_path: std::path::PathBuf::from("/x/npm"),
                children: vec![cache("registry", 10, 0), cache("cache", 5, 0)],
            },
        );
        assert_eq!(s.all.len(), 2);
        assert_eq!(s.stack.len(), 1);
        assert_eq!(
            s.drill_paths.last().unwrap(),
            &std::path::PathBuf::from("/x/npm")
        );
        assert!(s.loading.is_none());
        assert!(cmd.is_done());
    }

    #[test]
    fn scan_completed_empty_children_just_clears_loading() {
        let mut s = state(vec![cache("npm", 100, 0)]);
        s.loading = Some(Loading {
            label: "scanning npm".into(),
            frame: 0,
            started: std::time::Instant::now(),
            folders: None,
        });
        let (s, cmd) = update(
            s,
            Msg::ScanCompleted {
                parent_label: "npm".into(),
                parent_path: std::path::PathBuf::from("/x/npm"),
                children: vec![],
            },
        );
        assert_eq!(s.all.len(), 1);
        assert!(s.loading.is_none());
        assert!(cmd.is_done());
    }

    #[test]
    fn scan_progress_updates_loading_folder_count() {
        let mut s = state(Vec::new());
        s.loading = Some(Loading {
            label: "scanning caches".into(),
            frame: 0,
            started: std::time::Instant::now(),
            folders: Some(0),
        });
        let (s, cmd) = update(s, Msg::ScanProgress { folders: 1234 });
        assert!(cmd.is_done());
        assert_eq!(s.loading.as_ref().unwrap().folders, Some(1234));
    }

    #[test]
    fn scan_progress_is_noop_when_not_loading() {
        let s = state(vec![cache("a", 1, 0)]);
        let (s, cmd) = update(s, Msg::ScanProgress { folders: 5 });
        assert!(cmd.is_done());
        assert!(s.loading.is_none());
    }

    #[test]
    fn seeds_loaded_replaces_all_and_clears_loading() {
        // Simulates startup: empty list + spinner, then the LoadSeeds worker
        // returns the scanned caches.  state.all is replaced wholesale, the
        // spinner clears, cursor resets to 0.
        let mut s = state(Vec::new());
        s.loading = Some(Loading {
            label: "scanning caches".into(),
            frame: 3,
            started: std::time::Instant::now(),
            folders: None,
        });
        s.cursor = 7; // would be invalid against an empty list
        let (s, cmd) = update(
            s,
            Msg::SeedsLoaded {
                caches: vec![cache("npm", 100, 0), cache("cargo", 50, 0)],
            },
        );
        assert_eq!(s.all.len(), 2);
        assert_eq!(s.cursor, 0);
        assert!(s.loading.is_none());
        assert!(cmd.is_done());
    }

    #[test]
    fn drill_out_when_clean_returns_done() {
        let mut s = state(vec![cache("npm", 100, 0)]);
        s.drill_into(vec![cache("registry", 10, 0)]);
        // level_dirty defaults false after drill_into
        let (s, cmd) = update(s, Msg::DrillOut);
        assert!(cmd.is_done());
        assert_eq!(s.all[0].label, "npm");
    }

    #[test]
    fn drill_out_is_noop_while_loading() {
        // Pressing Esc/Backspace while a scan/delete/refresh is in flight must
        // NOT swap `state.all` — the worker's result Msg would then index into
        // the wrong list.
        let mut s = state(vec![cache("npm", 100, 0)]);
        s.drill_into(vec![cache("registry", 10, 0)]);
        s.loading = Some(Loading {
            label: "deleting 1 cache".into(),
            frame: 0,
            started: std::time::Instant::now(),
            folders: None,
        });
        let (s, cmd) = update(s, Msg::DrillOut);
        assert!(cmd.is_done(), "no refresh effect must be emitted");
        assert_eq!(s.all[0].label, "registry", "stack must not be popped");
        assert_eq!(s.stack.len(), 1);
    }

    #[test]
    fn drill_out_when_dirty_emits_refresh_effect() {
        let mut s = state(vec![cache("npm", 100, 0)]);
        s.drill_paths.push(std::path::PathBuf::from("/x/npm"));
        s.drill_into(vec![cache("registry", 10, 0)]);
        s.level_dirty = true;
        let (s, cmd) = update(s, Msg::DrillOut);
        assert!(s.loading.is_some());
        assert!(matches!(
            cmd.effects.as_slice(),
            [Effect::SpawnRefresh { .. }]
        ));
    }

    #[test]
    fn refresh_completed_replaces_matching_cache() {
        let mut s = state(vec![cache("a", 100, 0), cache("b", 200, 0)]);
        s.loading = Some(Loading {
            label: "x".into(),
            frame: 0,
            started: std::time::Instant::now(),
            folders: None,
        });
        let updated = Cache {
            label: "b".into(),
            path: std::path::PathBuf::from("/x/b"),
            size_bytes: 999,
            newest_mtime: None,
            file_count: 0,
            dir_count: 0,
            top_files: Vec::new(),
            unreadable: 0,
        };
        let (s, cmd) = update(
            s,
            Msg::RefreshCompleted {
                path: std::path::PathBuf::from("/x/b"),
                cache: updated,
            },
        );
        assert_eq!(s.all[1].size_bytes, 999);
        assert!(s.loading.is_none());
        assert!(cmd.is_done());
    }

    #[test]
    fn refresh_completed_unknown_path_clears_loading() {
        let mut s = state(vec![cache("a", 100, 0)]);
        s.loading = Some(Loading {
            label: "x".into(),
            frame: 0,
            started: std::time::Instant::now(),
            folders: None,
        });
        let (s, cmd) = update(
            s,
            Msg::RefreshCompleted {
                path: std::path::PathBuf::from("/x/gone"),
                cache: cache("gone", 1, 0),
            },
        );
        assert_eq!(s.all[0].size_bytes, 100);
        assert!(s.loading.is_none());
        assert!(cmd.is_done());
    }

    #[test]
    fn delete_pressed_with_yes_mode_chains_confirm_event() {
        let mut s = state(vec![cache("a", 1, 0)]);
        s.marks.toggle(0);
        s.yes_mode = true;
        let (s, cmd) = update(s, Msg::DeletePressed);
        assert!(matches!(s.modal, Modal::DeleteConfirm));
        assert!(matches!(cmd.events.as_slice(), [Msg::ConfirmDelete]));
    }

    #[test]
    fn delete_pressed_without_yes_mode_just_opens_modal() {
        let mut s = state(vec![cache("a", 1, 0)]);
        s.marks.toggle(0);
        s.yes_mode = false;
        let (s, cmd) = update(s, Msg::DeletePressed);
        assert!(matches!(s.modal, Modal::DeleteConfirm));
        assert!(cmd.events.is_empty());
        assert!(cmd.effects.is_empty());
    }
}
