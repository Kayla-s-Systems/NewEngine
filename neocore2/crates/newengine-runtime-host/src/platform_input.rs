#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::atomic::{AtomicBool, Ordering};

use newengine_core::call_service_v1_optional;
use newengine_math::collections_prelude::NeBTreeMap;
use newengine_ui_api::{UiInputFrame, UiTextEditOp, UiTextEditOpKind};

/// Engine-facing input gateway id. Consumers call the engine facade; the host
/// resolves it to the active input provider by descriptor metadata.
pub const INPUT_SERVICE_ID: &str = newengine_input_api::ENGINE_INPUT_SERVICE_ID;

static INPUT_POLL_ONLINE_LOGGED: AtomicBool = AtomicBool::new(false);
static INPUT_POLL_OFFLINE_LOGGED: AtomicBool = AtomicBool::new(false);

/// Polls one atomic input snapshot from the canonical INPUT provider and maps it
/// into the UI/runtime DTO. `state_json` owns all one-shot edges, deltas, text and
/// IME commit data, so the host performs exactly one gateway round-trip per frame.
pub fn poll_input_frame() -> Option<UiInputFrame> {
    let Some(bytes) = call_service_v1_optional(
        INPUT_SERVICE_ID,
        newengine_input_api::INPUT_METHOD_STATE_JSON,
        &[],
    )
    .ok()?
    else {
        if !INPUT_POLL_OFFLINE_LOGGED.swap(true, Ordering::Relaxed) {
            newengine_ulog_api::ulog::warn!(
                "input systems: raw input polling unavailable service='{}' method='{}'",
                INPUT_SERVICE_ID,
                newengine_input_api::INPUT_METHOD_STATE_JSON,
            );
        }
        return None;
    };
    if !INPUT_POLL_ONLINE_LOGGED.swap(true, Ordering::Relaxed) {
        newengine_ulog_api::ulog::info!(
            "input systems: raw input polling online service='{}' method='{}' policy='single atomic typed snapshot'",
            INPUT_SERVICE_ID,
            newengine_input_api::INPUT_METHOD_STATE_JSON,
        );
    }

    let st: newengine_input_api::InputStateSnapshot = match serde_json::from_slice(&bytes) {
        Ok(value) => value,
        Err(e) => {
            newengine_ulog_api::ulog::warn!(
                "input systems: raw input state_json typed decode failed err='{}'",
                e
            );
            return None;
        }
    };

    let mut out = UiInputFrame::default();
    out.keys_down.extend(st.keys.down);
    out.keys_pressed.extend(st.keys.pressed);
    out.keys_released.extend(st.keys.released);

    out.mouse_pos = Some((st.mouse.pos.x, st.mouse.pos.y));
    out.mouse_delta = (st.mouse.delta.x, st.mouse.delta.y);
    out.mouse_wheel = (st.mouse.wheel.x, st.mouse.wheel.y);
    out.mouse_down.extend(st.mouse.down);
    out.mouse_pressed.extend(st.mouse.pressed);
    out.mouse_released.extend(st.mouse.released);

    let mut state_edit_op_count = 0usize;
    out.text = st.text.buffer;
    out.ime_preedit = st.text.ime_preedit;
    out.ime_commit = st.text.ime_commit;
    let state_text_chars = out.text.chars().count();
    let state_ime_commit_chars = out.ime_commit.chars().count();
    let state_ime_preedit_chars = out.ime_preedit.chars().count();
    for op in st.text.edit_ops {
        if let Some(kind) = parse_text_edit_op_kind(&op) {
            state_edit_op_count = state_edit_op_count.saturating_add(1);
            out.text_edit_ops
                .push(UiTextEditOp::new(kind, "engine.input"));
        } else {
            newengine_ulog_api::ulog::warn!(
                "input systems: ignored unknown text edit op op='{}'",
                op
            );
        }
    }

    for pad in st.gamepads.into_values() {
        if pad.connected {
            out.gamepad_connected = out.gamepad_connected.saturating_add(1);
        }
        merge_f32_map(&mut out.gamepad_buttons, pad.buttons);
        merge_f32_map(&mut out.gamepad_axes, pad.axes);
        out.gamepad_buttons_pressed.extend(pad.buttons_pressed);
        out.gamepad_buttons_released.extend(pad.buttons_released);
    }

    let leaked_gameplay_text = gate_gameplay_text_leak(&mut out);
    log_input_frame_summary(
        &out,
        if leaked_gameplay_text {
            0
        } else {
            state_text_chars
        },
        if leaked_gameplay_text {
            0
        } else {
            state_ime_commit_chars
        },
        state_ime_preedit_chars,
        state_edit_op_count,
        false,
        false,
    );

    Some(out)
}

fn gate_gameplay_text_leak(frame: &mut UiInputFrame) -> bool {
    if frame.text.is_empty() && frame.ime_commit.is_empty() {
        return false;
    }
    if !frame.ime_preedit.is_empty() || !frame.text_edit_ops.is_empty() {
        return false;
    }
    let key_activity = !frame.keys_down.is_empty()
        || !frame.keys_pressed.is_empty()
        || !frame.keys_released.is_empty();
    let text_is_gameplay_controls = frame.text.chars().all(is_gameplay_text_leak_char)
        && frame.ime_commit.chars().all(is_gameplay_text_leak_char);
    if !text_is_gameplay_controls {
        return false;
    }
    if !key_activity && frame.text.chars().count() + frame.ime_commit.chars().count() > 2 {
        return false;
    }
    frame.text.clear();
    frame.ime_commit.clear();
    true
}

fn is_gameplay_text_leak_char(ch: char) -> bool {
    matches!(
        ch,
        'w' | 'a'
            | 's'
            | 'd'
            | 'W'
            | 'A'
            | 'S'
            | 'D'
            | 'ц'
            | 'ф'
            | 'ы'
            | 'в'
            | 'Ц'
            | 'Ф'
            | 'Ы'
            | 'В'
            | ' '
            | '`'
            | '~'
            | 'ё'
            | 'Ё'
            | '\u{1b}'
            | '\n'
            | '\r'
            | '\t'
    )
}

fn log_input_frame_summary(
    frame: &UiInputFrame,
    state_text_chars: usize,
    state_ime_commit_chars: usize,
    state_ime_preedit_chars: usize,
    state_edit_op_count: usize,
    fallback_text_used: bool,
    fallback_ime_used: bool,
) {
    let interesting = !frame.text.is_empty()
        || !frame.ime_commit.is_empty()
        || !frame.ime_preedit.is_empty()
        || !frame.text_edit_ops.is_empty()
        || !frame.keys_pressed.is_empty()
        || !frame.keys_released.is_empty()
        || !frame.mouse_pressed.is_empty()
        || !frame.mouse_released.is_empty()
        || frame.mouse_wheel.0.abs() > f32::EPSILON
        || frame.mouse_wheel.1.abs() > f32::EPSILON;
    if !interesting {
        return;
    }

    newengine_ulog_api::ulog::info!(
        "input systems: ui input frame text_chars={} ime_commit_chars={} ime_preedit_chars={} edit_ops={} state_text_chars={} state_ime_commit_chars={} state_ime_preedit_chars={} state_edit_ops={} fallback_text_used={} fallback_ime_used={} keys_pressed={} keys_released={} mouse_pressed={} mouse_released={} mouse_pos={:?} wheel=({:.2},{:.2}) text_preview='{}'",
        frame.text.chars().count(),
        frame.ime_commit.chars().count(),
        frame.ime_preedit.chars().count(),
        frame.text_edit_ops.len(),
        state_text_chars,
        state_ime_commit_chars,
        state_ime_preedit_chars,
        state_edit_op_count,
        fallback_text_used,
        fallback_ime_used,
        frame.keys_pressed.len(),
        frame.keys_released.len(),
        frame.mouse_pressed.len(),
        frame.mouse_released.len(),
        frame.mouse_pos,
        frame.mouse_wheel.0,
        frame.mouse_wheel.1,
        preview_text(&frame.text),
    );
}

fn preview_text(value: &str) -> String {
    let mut out = value.chars().take(24).collect::<String>();
    if value.chars().count() > 24 {
        out.push('…');
    }
    out.replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn merge_f32_map(
    target: &mut NeBTreeMap<String, f32>,
    values: std::collections::BTreeMap<String, f32>,
) {
    for (key, value) in values {
        let entry = target.entry(key).or_insert(0.0);
        if value.abs() > entry.abs() {
            *entry = value;
        }
    }
}

fn parse_text_edit_op_kind(op: &str) -> Option<UiTextEditOpKind> {
    match op.trim() {
        "backspace" => Some(UiTextEditOpKind::Backspace),
        "delete" => Some(UiTextEditOpKind::Delete),
        "move_left" => Some(UiTextEditOpKind::MoveLeft),
        "move_right" => Some(UiTextEditOpKind::MoveRight),
        "move_start" => Some(UiTextEditOpKind::MoveStart),
        "move_end" => Some(UiTextEditOpKind::MoveEnd),
        "select_all" => Some(UiTextEditOpKind::SelectAll),
        "copy" => Some(UiTextEditOpKind::Copy),
        "cut" => Some(UiTextEditOpKind::Cut),
        "paste" => Some(UiTextEditOpKind::Paste),
        _ => None,
    }
}
