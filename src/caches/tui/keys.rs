//! Map crossterm KeyEvents to TUI Msg.

use super::Msg;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ModalKind {
    None,
    DeleteConfirm,
    ActiveMark,
    FilterEdit,
}

pub fn key_to_msg(k: KeyEvent, modal: ModalKind, focus_right: bool) -> Option<Msg> {
    use KeyCode::*;
    match modal {
        ModalKind::DeleteConfirm => match (k.code, k.modifiers) {
            (Char('y'), KeyModifiers::NONE) | (Enter, _) => Some(Msg::ConfirmDelete),
            (Char('n'), KeyModifiers::NONE) | (Esc, _) => Some(Msg::CancelDelete),
            _ => None,
        },
        ModalKind::ActiveMark => match (k.code, k.modifiers) {
            (Char('y'), KeyModifiers::NONE) | (Enter, _) => Some(Msg::ConfirmActiveMark),
            (Char('n'), KeyModifiers::NONE) | (Esc, _) => Some(Msg::CancelActiveMark),
            _ => None,
        },
        ModalKind::FilterEdit => match (k.code, k.modifiers) {
            (Enter, _) => Some(Msg::FilterApply),
            (Esc, _) => Some(Msg::FilterCancel),
            (Backspace, _) => Some(Msg::FilterBackspace),
            // Accept printable chars with NONE or SHIFT modifiers (uppercase).
            (Char(c), m)
                if (m == KeyModifiers::NONE || m == KeyModifiers::SHIFT) && !c.is_control() =>
            {
                Some(Msg::FilterChar(c))
            }
            _ => None,
        },
        ModalKind::None if focus_right => match (k.code, k.modifiers) {
            // Right pane focus: only the file-list scrollers + focus toggle
            // + quit are live. Mark/sort/drill/delete are out of scope here.
            (Up, _) | (Char('k'), KeyModifiers::NONE) => Some(Msg::MoveUp),
            (Down, _) | (Char('j'), KeyModifiers::NONE) => Some(Msg::MoveDown),
            (Tab, _) | (BackTab, _) | (Esc, _) => Some(Msg::ToggleFocus),
            (Char('q'), KeyModifiers::NONE) => Some(Msg::RequestQuit),
            _ => None,
        },
        ModalKind::None => match (k.code, k.modifiers) {
            (Up, _) | (Char('k'), KeyModifiers::NONE) => Some(Msg::MoveUp),
            (Down, _) | (Char('j'), KeyModifiers::NONE) => Some(Msg::MoveDown),
            (Right, _) | (Char('l'), KeyModifiers::NONE) | (Enter, _) => Some(Msg::DrillIn),
            (Left, _) | (Char('h'), KeyModifiers::NONE) | (Esc, _) | (Backspace, _) => {
                Some(Msg::DrillOut)
            }
            (Char(' '), _) => Some(Msg::ToggleMark),
            (Char('m'), KeyModifiers::NONE) => Some(Msg::MarkDownToCursor),
            (Char('s'), KeyModifiers::NONE) => Some(Msg::CycleSort),
            (Char('d'), KeyModifiers::NONE) => Some(Msg::DeletePressed),
            (Char('/'), KeyModifiers::NONE) => Some(Msg::FilterStart),
            (Char('*'), _) => Some(Msg::MarkAllVisible),
            (Tab, _) | (BackTab, _) => Some(Msg::ToggleFocus),
            (Char('q'), KeyModifiers::NONE) => Some(Msg::RequestQuit),
            _ => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn k(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn arrows_map_to_movement() {
        assert!(matches!(
            key_to_msg(k(KeyCode::Up), ModalKind::None, false),
            Some(Msg::MoveUp)
        ));
        assert!(matches!(
            key_to_msg(k(KeyCode::Down), ModalKind::None, false),
            Some(Msg::MoveDown)
        ));
    }
    #[test]
    fn vim_keys_map_to_movement() {
        assert!(matches!(
            key_to_msg(k(KeyCode::Char('j')), ModalKind::None, false),
            Some(Msg::MoveDown)
        ));
        assert!(matches!(
            key_to_msg(k(KeyCode::Char('k')), ModalKind::None, false),
            Some(Msg::MoveUp)
        ));
        assert!(matches!(
            key_to_msg(k(KeyCode::Char('l')), ModalKind::None, false),
            Some(Msg::DrillIn)
        ));
        assert!(matches!(
            key_to_msg(k(KeyCode::Char('h')), ModalKind::None, false),
            Some(Msg::DrillOut)
        ));
    }
    #[test]
    fn enter_is_drill_in() {
        assert!(matches!(
            key_to_msg(k(KeyCode::Enter), ModalKind::None, false),
            Some(Msg::DrillIn)
        ));
    }
    #[test]
    fn esc_is_drill_out() {
        assert!(matches!(
            key_to_msg(k(KeyCode::Esc), ModalKind::None, false),
            Some(Msg::DrillOut)
        ));
    }
    #[test]
    fn backspace_is_drill_out() {
        assert!(matches!(
            key_to_msg(k(KeyCode::Backspace), ModalKind::None, false),
            Some(Msg::DrillOut)
        ));
    }
    #[test]
    fn q_requests_quit() {
        assert!(matches!(
            key_to_msg(k(KeyCode::Char('q')), ModalKind::None, false),
            Some(Msg::RequestQuit)
        ));
    }
    #[test]
    fn d_in_normal_mode_requests_delete() {
        assert!(matches!(
            key_to_msg(k(KeyCode::Char('d')), ModalKind::None, false),
            Some(Msg::DeletePressed)
        ));
    }
    #[test]
    fn y_in_modal_confirms_delete() {
        assert!(matches!(
            key_to_msg(k(KeyCode::Char('y')), ModalKind::DeleteConfirm, false),
            Some(Msg::ConfirmDelete)
        ));
    }
    #[test]
    fn n_in_modal_cancels_delete() {
        assert!(matches!(
            key_to_msg(k(KeyCode::Char('n')), ModalKind::DeleteConfirm, false),
            Some(Msg::CancelDelete)
        ));
    }
    #[test]
    fn y_in_active_modal_confirms_active_mark() {
        assert!(matches!(
            key_to_msg(k(KeyCode::Char('y')), ModalKind::ActiveMark, false),
            Some(Msg::ConfirmActiveMark)
        ));
    }
    #[test]
    fn n_in_active_modal_cancels_active_mark() {
        assert!(matches!(
            key_to_msg(k(KeyCode::Char('n')), ModalKind::ActiveMark, false),
            Some(Msg::CancelActiveMark)
        ));
    }

    #[test]
    fn right_focus_swallows_mark_sort_delete() {
        // While the right pane is focused, those left-pane-only actions are
        // ignored so the user doesn't accidentally mark or delete from a
        // file-list scroll session.
        assert!(key_to_msg(k(KeyCode::Char(' ')), ModalKind::None, true).is_none());
        assert!(key_to_msg(k(KeyCode::Char('m')), ModalKind::None, true).is_none());
        assert!(key_to_msg(k(KeyCode::Char('s')), ModalKind::None, true).is_none());
        assert!(key_to_msg(k(KeyCode::Char('d')), ModalKind::None, true).is_none());
    }

    #[test]
    fn slash_starts_filter() {
        assert!(matches!(
            key_to_msg(k(KeyCode::Char('/')), ModalKind::None, false),
            Some(Msg::FilterStart)
        ));
    }

    #[test]
    fn star_marks_all_visible() {
        assert!(matches!(
            key_to_msg(k(KeyCode::Char('*')), ModalKind::None, false),
            Some(Msg::MarkAllVisible)
        ));
    }

    #[test]
    fn filter_edit_routes_text_input() {
        assert!(matches!(
            key_to_msg(k(KeyCode::Char('a')), ModalKind::FilterEdit, false),
            Some(Msg::FilterChar('a'))
        ));
        assert!(matches!(
            key_to_msg(k(KeyCode::Backspace), ModalKind::FilterEdit, false),
            Some(Msg::FilterBackspace)
        ));
        assert!(matches!(
            key_to_msg(k(KeyCode::Enter), ModalKind::FilterEdit, false),
            Some(Msg::FilterApply)
        ));
        assert!(matches!(
            key_to_msg(k(KeyCode::Esc), ModalKind::FilterEdit, false),
            Some(Msg::FilterCancel)
        ));
    }

    #[test]
    fn right_focus_keeps_movement_focus_quit() {
        assert!(matches!(
            key_to_msg(k(KeyCode::Up), ModalKind::None, true),
            Some(Msg::MoveUp)
        ));
        assert!(matches!(
            key_to_msg(k(KeyCode::Down), ModalKind::None, true),
            Some(Msg::MoveDown)
        ));
        assert!(matches!(
            key_to_msg(k(KeyCode::Tab), ModalKind::None, true),
            Some(Msg::ToggleFocus)
        ));
        assert!(matches!(
            key_to_msg(k(KeyCode::BackTab), ModalKind::None, true),
            Some(Msg::ToggleFocus)
        ));
        assert!(matches!(
            key_to_msg(k(KeyCode::Esc), ModalKind::None, true),
            Some(Msg::ToggleFocus)
        ));
        assert!(matches!(
            key_to_msg(k(KeyCode::Char('q')), ModalKind::None, true),
            Some(Msg::RequestQuit)
        ));
    }
}
