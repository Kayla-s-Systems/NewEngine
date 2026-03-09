#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::call_service_v1_optional;
use newengine_ui::UiInputFrame;

/// Canonical input service capability id.
pub const INPUT_SERVICE_ID: &str = "newengine.input.v1";

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

    Some(out)
}
