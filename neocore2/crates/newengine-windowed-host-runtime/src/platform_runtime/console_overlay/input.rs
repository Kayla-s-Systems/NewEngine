use newengine_ui_api::{keys, UiInputFrame, UiTextEditOpKind};

use super::state::RuntimeConsoleOverlayState;

#[derive(Default)]
pub(super) struct ConsoleInputOutcome {
    pub consumed: bool,
    pub state_changed: bool,
    pub buffer_changed: bool,
    pub opened: bool,
    pub closed: bool,
    pub execute_line: Option<String>,
}

pub(super) fn process(
    state: &mut RuntimeConsoleOverlayState,
    input: Option<&UiInputFrame>,
) -> ConsoleInputOutcome {
    let Some(input) = input else {
        return ConsoleInputOutcome::default();
    };
    let toggle_pressed = input.is_key_pressed(keys::BACKQUOTE);
    if toggle_pressed {
        state.open = !state.open;
        state.history_cursor = None;
        state.touch();
        return ConsoleInputOutcome {
            consumed: true,
            state_changed: true,
            opened: state.open,
            closed: !state.open,
            ..ConsoleInputOutcome::default()
        };
    }
    if !state.open {
        return ConsoleInputOutcome::default();
    }

    let mut out = ConsoleInputOutcome {
        consumed: true,
        ..ConsoleInputOutcome::default()
    };

    if input.is_key_pressed(keys::ESCAPE) {
        state.open = false;
        state.history_cursor = None;
        state.touch();
        out.state_changed = true;
        out.closed = true;
        return out;
    }

    if input.mouse_wheel.1.abs() > f32::EPSILON && state.scroll_output_wheel(input.mouse_wheel.1) {
        out.state_changed = true;
    }

    if input.is_key_pressed(keys::ENTER) {
        let line = state.buffer.trim().to_owned();
        if !line.is_empty() {
            state.remember_command(&line);
            state.buffer.clear();
            state.cursor_chars = 0;
            state.history_cursor = None;
            state.touch();
            out.execute_line = Some(line);
            out.state_changed = true;
            out.buffer_changed = true;
        }
        return out;
    }

    if input.is_key_pressed(keys::TAB) {
        if let Some(insert) = state
            .suggestions
            .items
            .first()
            .map(|item| item.insert.clone())
        {
            state.set_buffer(insert);
            out.state_changed = true;
            out.buffer_changed = true;
        }
        return out;
    }

    if input.is_key_pressed(keys::ARROW_UP) {
        let before = state.buffer.clone();
        state.history_step(-1);
        out.state_changed = state.buffer != before;
        out.buffer_changed = out.state_changed;
        return out;
    }
    if input.is_key_pressed(keys::ARROW_DOWN) {
        let before = state.buffer.clone();
        state.history_step(1);
        out.state_changed = state.buffer != before;
        out.buffer_changed = out.state_changed;
        return out;
    }

    let mut had_insert_op = false;
    let mut had_backspace_op = false;
    let mut had_delete_op = false;
    let mut had_move_left_op = false;
    let mut had_move_right_op = false;
    let mut had_move_start_op = false;
    let mut had_move_end_op = false;
    for op in &input.text_edit_ops {
        match op.kind {
            UiTextEditOpKind::InsertText | UiTextEditOpKind::Paste => {
                if !op.text.is_empty() {
                    state.insert_text(&op.text);
                    had_insert_op = true;
                    out.state_changed = true;
                    out.buffer_changed = true;
                }
            }
            UiTextEditOpKind::Backspace => {
                state.backspace();
                had_backspace_op = true;
                out.state_changed = true;
                out.buffer_changed = true;
            }
            UiTextEditOpKind::Delete => {
                state.delete();
                had_delete_op = true;
                out.state_changed = true;
                out.buffer_changed = true;
            }
            UiTextEditOpKind::MoveLeft => {
                state.move_cursor(-1);
                had_move_left_op = true;
                out.state_changed = true;
            }
            UiTextEditOpKind::MoveRight => {
                state.move_cursor(1);
                had_move_right_op = true;
                out.state_changed = true;
            }
            UiTextEditOpKind::MoveStart => {
                state.cursor_chars = 0;
                state.touch();
                had_move_start_op = true;
                out.state_changed = true;
            }
            UiTextEditOpKind::MoveEnd => {
                state.cursor_chars = state.buffer.chars().count();
                state.touch();
                had_move_end_op = true;
                out.state_changed = true;
            }
            UiTextEditOpKind::SelectAll | UiTextEditOpKind::Copy | UiTextEditOpKind::Cut => {}
        }
    }

    if !had_insert_op {
        let committed = if !input.ime_commit.is_empty() {
            input.ime_commit.as_str()
        } else {
            input.text.as_str()
        };
        if !committed.is_empty() {
            state.insert_text(committed);
            out.state_changed = true;
            out.buffer_changed = true;
        }
    }
    if !had_backspace_op && input.is_key_pressed(keys::BACKSPACE) {
        state.backspace();
        out.state_changed = true;
        out.buffer_changed = true;
    }
    if !had_delete_op && input.is_key_pressed(keys::DELETE) {
        state.delete();
        out.state_changed = true;
        out.buffer_changed = true;
    }
    if !had_move_left_op && input.is_key_pressed(keys::ARROW_LEFT) {
        state.move_cursor(-1);
        out.state_changed = true;
    }
    if !had_move_right_op && input.is_key_pressed(keys::ARROW_RIGHT) {
        state.move_cursor(1);
        out.state_changed = true;
    }
    if !had_move_start_op && input.is_key_pressed(keys::HOME) {
        state.cursor_chars = 0;
        state.touch();
        out.state_changed = true;
    }
    if !had_move_end_op && input.is_key_pressed(keys::END) {
        state.cursor_chars = state.buffer.chars().count();
        state.touch();
        out.state_changed = true;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backquote_toggles_and_consumes_without_inserting_character() {
        let mut state = RuntimeConsoleOverlayState::default();
        let mut frame = UiInputFrame::default();
        frame.keys_pressed.insert(keys::BACKQUOTE);
        frame.text = "`".to_owned();
        let out = process(&mut state, Some(&frame));
        assert!(out.consumed);
        assert!(state.open);
        assert!(state.buffer.is_empty());
    }

    #[test]
    fn escape_closes_and_is_consumed() {
        let mut state = RuntimeConsoleOverlayState {
            open: true,
            ..RuntimeConsoleOverlayState::default()
        };
        let mut frame = UiInputFrame::default();
        frame.keys_pressed.insert(keys::ESCAPE);
        let out = process(&mut state, Some(&frame));
        assert!(out.closed);
        assert!(out.consumed);
        assert!(!state.open);
    }
}
