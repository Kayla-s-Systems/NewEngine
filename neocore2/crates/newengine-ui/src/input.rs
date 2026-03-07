#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::BTreeSet;

/// UI input snapshot coming from INPUT plugin (engine-level canonical input).
#[derive(Debug, Clone, Default)]
pub struct UiInputFrame {
    pub keys_down: BTreeSet<u32>,
    pub keys_pressed: BTreeSet<u32>,
    pub keys_released: BTreeSet<u32>,

    pub mouse_pos: Option<(f32, f32)>,
    pub mouse_delta: (f32, f32),
    pub mouse_wheel: (f32, f32),

    pub mouse_down: BTreeSet<u32>,
    pub mouse_pressed: BTreeSet<u32>,
    pub mouse_released: BTreeSet<u32>,

    /// Text typed since last `text_take_json` in input plugin.
    pub text: String,

    /// IME preedit is typically frame-local. Provided if you choose to expose it later.
    pub ime_preedit: String,

    /// IME commit text (taken via `ime_commit_take_json`).
    pub ime_commit: String,
}

impl UiInputFrame {
    #[inline]
    pub fn is_key_down(&self, key: u32) -> bool {
        self.keys_down.contains(&key)
    }

    #[inline]
    pub fn is_key_pressed(&self, key: u32) -> bool {
        self.keys_pressed.contains(&key)
    }

    #[inline]
    pub fn is_mouse_down(&self, btn: u32) -> bool {
        self.mouse_down.contains(&btn)
    }

    #[inline]
    pub fn is_mouse_pressed(&self, btn: u32) -> bool {
        self.mouse_pressed.contains(&btn)
    }
}

/// Canonical keyboard ids consumed by editor/UI code.
///
/// These are intentionally plain `u32` values, not `winit::KeyCode`, so higher layers
/// (`editor`, generic UI code) stay fully decoupled from the platform backend crate.
///
/// Current values match the ordinal layout of `winit::keyboard::KeyCode` used by the
/// platform input bridge.
pub mod keys {
    pub const DIGIT1: u32 = 6;
    pub const DIGIT2: u32 = 7;
    pub const DIGIT3: u32 = 8;

    pub const KEY_A: u32 = 19;
    pub const KEY_D: u32 = 22;
    pub const KEY_E: u32 = 23;
    pub const KEY_F: u32 = 24;
    pub const KEY_N: u32 = 32;
    pub const KEY_O: u32 = 33;
    pub const KEY_Q: u32 = 35;
    pub const KEY_R: u32 = 36;
    pub const KEY_S: u32 = 37;
    pub const KEY_W: u32 = 41;
    pub const KEY_Y: u32 = 43;
    pub const KEY_Z: u32 = 44;

    pub const ALT_LEFT: u32 = 50;
    pub const ALT_RIGHT: u32 = 51;
    pub const CONTROL_LEFT: u32 = 55;
    pub const CONTROL_RIGHT: u32 = 56;
    pub const ENTER: u32 = 57;
    pub const SHIFT_LEFT: u32 = 60;
    pub const SHIFT_RIGHT: u32 = 61;

    pub const ESCAPE: u32 = 114;

    pub const F1: u32 = 159;
    pub const F2: u32 = 160;
}