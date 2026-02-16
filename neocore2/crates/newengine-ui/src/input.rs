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
