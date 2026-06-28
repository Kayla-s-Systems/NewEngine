#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::atomic::{AtomicBool, Ordering};

use newengine_core::call_service_v1_optional;
use newengine_math::collections_prelude::{NeBTreeMap, NeBTreeSet};
use newengine_ui_api::{UiInputFrame, UiTextEditOp, UiTextEditOpKind};

/// Engine-facing input gateway id. Consumers call the engine facade; the host
/// resolves it to the active input provider by descriptor metadata.
pub const INPUT_SERVICE_ID: &str = newengine_input_api::ENGINE_INPUT_SERVICE_ID;

static INPUT_POLL_ONLINE_LOGGED: AtomicBool = AtomicBool::new(false);
static INPUT_POLL_OFFLINE_LOGGED: AtomicBool = AtomicBool::new(false);

/// Calls a service method returning UTF-8 payload (best-effort).
pub fn call_service_utf8(service_id: &str, method: &str) -> Option<String> {
    let bytes = call_service_v1_optional(service_id, method, &[]).ok()??;
    Some(String::from_utf8_lossy(&bytes).to_string())
}

/// Polls input snapshot from the canonical INPUT plugin and maps it into UiInputFrame.
pub fn poll_input_frame() -> Option<UiInputFrame> {
    let Some(state_json) = call_service_utf8(
        INPUT_SERVICE_ID,
        newengine_input_api::INPUT_METHOD_STATE_JSON,
    ) else {
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
            "input systems: raw input polling online service='{}' method='{}'",
            INPUT_SERVICE_ID,
            newengine_input_api::INPUT_METHOD_STATE_JSON,
        );
    }
    let text_json = call_service_utf8(
        INPUT_SERVICE_ID,
        newengine_input_api::INPUT_METHOD_TEXT_TAKE_JSON,
    )
    .unwrap_or_else(|| "{}".into());
    let ime_json = call_service_utf8(
        INPUT_SERVICE_ID,
        newengine_input_api::INPUT_METHOD_IME_COMMIT_TAKE_JSON,
    )
    .unwrap_or_else(|| "{}".into());

    let mut out = UiInputFrame::default();
    let st: serde_json::Value = match serde_json::from_str(&state_json) {
        Ok(value) => value,
        Err(e) => {
            newengine_ulog_api::ulog::warn!(
                "input systems: raw input state_json decode failed err='{}'",
                e
            );
            return None;
        }
    };

    if let Some(keys) = st.get("keys") {
        merge_u32_set(&mut out.keys_down, keys.get("down"));
        merge_u32_set(&mut out.keys_pressed, keys.get("pressed"));
        merge_u32_set(&mut out.keys_released, keys.get("released"));
    }

    if let Some(mouse) = st.get("mouse") {
        if let Some(pos) = mouse.get("pos") {
            let x = pos.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            let y = pos.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            out.mouse_pos = Some((x, y));
        }
        if let Some(delta) = mouse.get("delta") {
            out.mouse_delta.0 = delta.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            out.mouse_delta.1 = delta.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
        }
        if let Some(wheel) = mouse.get("wheel") {
            out.mouse_wheel.0 = wheel.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            out.mouse_wheel.1 = wheel.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
        }

        merge_u32_set(&mut out.mouse_down, mouse.get("down"));
        merge_u32_set(&mut out.mouse_pressed, mouse.get("pressed"));
        merge_u32_set(&mut out.mouse_released, mouse.get("released"));
    }

    let mut state_text_chars = 0usize;
    let mut state_ime_commit_chars = 0usize;
    let mut state_ime_preedit_chars = 0usize;
    let mut state_edit_op_count = 0usize;
    let mut fallback_text_used = false;
    let mut fallback_ime_used = false;

    if let Some(text) = st.get("text") {
        if let Some(s) = text.get("buffer").and_then(|x| x.as_str()) {
            state_text_chars = s.chars().count();
            out.text.push_str(s);
        }
        if let Some(s) = text.get("ime_preedit").and_then(|x| x.as_str()) {
            state_ime_preedit_chars = s.chars().count();
            out.ime_preedit.push_str(s);
        }
        if let Some(s) = text.get("ime_commit").and_then(|x| x.as_str()) {
            state_ime_commit_chars = s.chars().count();
            out.ime_commit.push_str(s);
        }
        if let Some(ops) = text.get("edit_ops").and_then(|v| v.as_array()) {
            for op in ops.iter().filter_map(|value| value.as_str()) {
                if let Some(kind) = parse_text_edit_op_kind(op) {
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
        }
    }

    if out.text.is_empty() {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text_json) {
            if let Some(s) = v.get("text").and_then(|x| x.as_str()) {
                fallback_text_used = !s.is_empty();
                out.text.push_str(s);
            }
        }
    }

    if out.ime_commit.is_empty() {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&ime_json) {
            if let Some(s) = v.get("ime_commit").and_then(|x| x.as_str()) {
                fallback_ime_used = !s.is_empty();
                out.ime_commit.push_str(s);
            }
        }
    }

    if gate_gameplay_text_leak(&mut out) {
        state_text_chars = 0;
        state_ime_commit_chars = 0;
        fallback_text_used = false;
        fallback_ime_used = false;
    }

    if let Some(gamepads) = st.get("gamepads").and_then(|v| v.as_object()) {
        for pad in gamepads.values() {
            let connected = pad
                .get("connected")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if connected {
                out.gamepad_connected = out.gamepad_connected.saturating_add(1);
            }
            merge_f32_object(&mut out.gamepad_buttons, pad.get("buttons"));
            merge_f32_object(&mut out.gamepad_axes, pad.get("axes"));
            merge_string_set(&mut out.gamepad_buttons_pressed, pad.get("buttons_pressed"));
            merge_string_set(
                &mut out.gamepad_buttons_released,
                pad.get("buttons_released"),
            );
        }
    }

    log_input_frame_summary(
        &out,
        state_text_chars,
        state_ime_commit_chars,
        state_ime_preedit_chars,
        state_edit_op_count,
        fallback_text_used,
        fallback_ime_used,
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

fn merge_u32_set(target: &mut NeBTreeSet<u32>, value: Option<&serde_json::Value>) {
    let Some(arr) = value.and_then(|v| v.as_array()) else {
        return;
    };
    for item in arr {
        if let Some(u) = item.as_u64() {
            target.insert(u as u32);
        }
    }
}

fn merge_f32_object(target: &mut NeBTreeMap<String, f32>, value: Option<&serde_json::Value>) {
    let Some(obj) = value.and_then(|v| v.as_object()) else {
        return;
    };
    for (key, raw) in obj {
        let v = raw.as_f64().unwrap_or(0.0) as f32;
        let entry = target.entry(key.clone()).or_insert(0.0);
        if v.abs() > entry.abs() {
            *entry = v;
        }
    }
}

fn merge_string_set(target: &mut NeBTreeSet<String>, value: Option<&serde_json::Value>) {
    let Some(arr) = value.and_then(|v| v.as_array()) else {
        return;
    };
    for item in arr {
        if let Some(s) = item.as_str() {
            target.insert(s.to_owned());
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
