#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::call_service_v1_optional;
use newengine_ui::UiInputFrame;

/// Engine-facing input gateway id. Consumers call the engine facade; the host
/// resolves it to the active input provider by descriptor metadata.
pub const INPUT_SERVICE_ID: &str = "engine.input";

/// Calls a service method returning UTF-8 payload (best-effort).
pub fn call_service_utf8(service_id: &str, method: &str) -> Option<String> {
    let bytes = call_service_v1_optional(service_id, method, &[]).ok()??;
    Some(String::from_utf8_lossy(&bytes).to_string())
}

/// Polls input snapshot from the canonical INPUT plugin and maps it into UiInputFrame.
pub fn poll_input_frame() -> Option<UiInputFrame> {
    let state_json = call_service_utf8(INPUT_SERVICE_ID, "state_json")?;
    let text_json = call_service_utf8(INPUT_SERVICE_ID, "text_take_json")
        .unwrap_or_else(|| "{}".into());
    let ime_json = call_service_utf8(INPUT_SERVICE_ID, "ime_commit_take_json")
        .unwrap_or_else(|| "{}".into());

    let mut out = UiInputFrame::default();
    let st: serde_json::Value = serde_json::from_str(&state_json).ok()?;

    if let Some(keys) = st.get("keys") {
        for (field, target) in [
            ("down", &mut out.keys_down),
            ("pressed", &mut out.keys_pressed),
            ("released", &mut out.keys_released),
        ] {
            if let Some(arr) = keys.get(field).and_then(|v| v.as_array()) {
                for x in arr {
                    if let Some(u) = x.as_u64() {
                        target.insert(u as u32);
                    }
                }
            }
        }
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

        for (field, target) in [
            ("down", &mut out.mouse_down),
            ("pressed", &mut out.mouse_pressed),
            ("released", &mut out.mouse_released),
        ] {
            if let Some(arr) = mouse.get(field).and_then(|v| v.as_array()) {
                for x in arr {
                    if let Some(u) = x.as_u64() {
                        target.insert(u as u32);
                    }
                }
            }
        }
    }

    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text_json) {
        if let Some(s) = v.get("text").and_then(|x| x.as_str()) {
            out.text.push_str(s);
        }
    }

    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&ime_json) {
        if let Some(s) = v.get("ime_commit").and_then(|x| x.as_str()) {
            out.ime_commit.push_str(s);
        }
    }

    if let Some(text) = st.get("text") {
        if let Some(s) = text.get("ime_preedit").and_then(|x| x.as_str()) {
            out.ime_preedit.push_str(s);
        }
    }

    if let Some(gamepads) = st.get("gamepads").and_then(|v| v.as_object()) {
        for pad in gamepads.values() {
            let connected = pad.get("connected").and_then(|v| v.as_bool()).unwrap_or(false);
            if connected {
                out.gamepad_connected = out.gamepad_connected.saturating_add(1);
            }
            merge_f32_object(&mut out.gamepad_buttons, pad.get("buttons"));
            merge_f32_object(&mut out.gamepad_axes, pad.get("axes"));
            merge_string_set(&mut out.gamepad_buttons_pressed, pad.get("buttons_pressed"));
            merge_string_set(&mut out.gamepad_buttons_released, pad.get("buttons_released"));
        }
    }

    Some(out)
}

fn merge_f32_object(target: &mut newengine_math::collections_prelude::NeBTreeMap<String, f32>, value: Option<&serde_json::Value>) {
    let Some(obj) = value.and_then(|v| v.as_object()) else { return; };
    for (key, raw) in obj {
        let v = raw.as_f64().unwrap_or(0.0) as f32;
        let entry = target.entry(key.clone()).or_insert(0.0);
        if v.abs() > entry.abs() {
            *entry = v;
        }
    }
}

fn merge_string_set(target: &mut newengine_math::collections_prelude::NeBTreeSet<String>, value: Option<&serde_json::Value>) {
    let Some(arr) = value.and_then(|v| v.as_array()) else { return; };
    for item in arr {
        if let Some(s) = item.as_str() {
            target.insert(s.to_owned());
        }
    }
}
