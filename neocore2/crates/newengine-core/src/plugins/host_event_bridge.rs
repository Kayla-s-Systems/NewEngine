#![forbid(unsafe_op_in_unsafe_fn)]

//! HostEvent -> plugin event bridge.
//!
//! Rationale:
//! - `Engine` should not know about the plugin wire format (topics/payload encoding).
//! - Bridging policy is expected to evolve (binary encoding, multiple domains, filtering).
//! - Keeping this in `plugins::*` makes it an internal adapter and reduces churn in `engine.rs`.

use crate::host_events::{HostEvent, InputHostEvent, KeyState, MouseButton, TextHostEvent};
use abi_stable::std_types::RVec;
use newengine_plugin_api::Blob;
use serde_json::json;

/// Broadcast a host event into the plugin event system.
///
/// Currently uses JSON payloads for convenience; this is intentionally isolated here so we can
/// migrate to a binary/event-schema format later without touching the engine loop.
pub(crate) fn broadcast_host_event_to_plugins(ev: &HostEvent) {
    fn key_state_str(s: KeyState) -> &'static str {
        match s {
            KeyState::Pressed => "pressed",
            KeyState::Released => "released",
        }
    }

    fn mouse_button_code(b: MouseButton) -> u32 {
        match b {
            MouseButton::Left => 1,
            MouseButton::Right => 2,
            MouseButton::Middle => 3,
            MouseButton::Other(v) => 10_000 + v as u32,
        }
    }

    let (topic, value) = match ev {
        HostEvent::Input(i) => match i {
            InputHostEvent::Key {
                code,
                state,
                repeat,
            } => (
                "winit.key",
                json!({
                    "key": (*code as u32),
                    "scancode": 0u32,
                    "state": key_state_str(*state),
                    "repeat": *repeat,
                }),
            ),
            InputHostEvent::MouseMove { x, y } => ("winit.mouse_move", json!({ "x": *x, "y": *y })),
            InputHostEvent::MouseDelta { dx, dy } => {
                ("winit.mouse_delta", json!({ "dx": *dx, "dy": *dy }))
            }
            InputHostEvent::MouseWheel { dx, dy } => {
                ("winit.mouse_wheel", json!({ "dx": *dx, "dy": *dy }))
            }
            InputHostEvent::MouseButton { button, state } => (
                "winit.mouse_button",
                json!({
                    "button": mouse_button_code(*button),
                    "state": key_state_str(*state),
                }),
            ),
        },
        HostEvent::Text(t) => match t {
            TextHostEvent::Char(ch) => ("winit.text_char", json!({ "cp": (*ch as u32) })),
            TextHostEvent::ImeCommit(text) => ("winit.ime_commit", json!({ "text": text })),
            TextHostEvent::ImePreedit(text) => ("winit.ime_preedit", json!({ "text": text })),
        },
        HostEvent::Window(_) => return,
    };

    let bytes = match serde_json::to_vec(&value) {
        Ok(v) => v,
        Err(_) => return,
    };

    let blob: Blob = RVec::from(bytes);
    crate::plugins::host_context::emit_event_v1(topic, blob);
}
