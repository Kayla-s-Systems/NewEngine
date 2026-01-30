use std::sync::{Arc, RwLock};

use newengine_core::host_events::{KeyCode, MouseButton};

use crate::state::{GamepadEvent, InputState};

/// Public input API exposed via Resources.
///
/// Keep it stable and minimal; build higher-level action mapping in separate layers.
pub trait InputApi: Send + Sync {
    fn key_down(&self, key: KeyCode) -> bool;
    fn key_pressed(&self, key: KeyCode) -> bool;
    fn key_released(&self, key: KeyCode) -> bool;

    fn mouse_pos(&self) -> (f32, f32);
    fn mouse_delta(&self) -> (f32, f32);
    fn wheel_delta(&self) -> (f32, f32);

    fn mouse_down(&self, btn: MouseButton) -> bool;
    fn mouse_pressed(&self, btn: MouseButton) -> bool;
    fn mouse_released(&self, btn: MouseButton) -> bool;

    fn text_chars(&self) -> Vec<char>;
    fn ime_preedit(&self) -> String;
    fn ime_commit(&self) -> String;

    /// Drains gamepad events produced since the last publish.
    /// Caller should reuse `out` to avoid allocations.
    fn drain_gamepad_events(&self, out: &mut Vec<GamepadEvent>);
}

#[derive(Clone)]
pub struct InputApiImpl {
    snap: Arc<RwLock<Snapshot>>,
}

#[derive(Default, Clone)]
struct Snapshot {
    keys_down: Vec<bool>,
    keys_pressed: Vec<bool>,
    keys_released: Vec<bool>,

    mouse_x: f32,
    mouse_y: f32,
    mouse_dx: f32,
    mouse_dy: f32,
    wheel_dx: f32,
    wheel_dy: f32,

    mouse_down_bits: u32,
    mouse_pressed_bits: u32,
    mouse_released_bits: u32,

    text: Vec<char>,
    ime_preedit: String,
    ime_commit: String,

    gamepad_events: Vec<GamepadEvent>,
}

impl InputApiImpl {
    #[inline]
    pub fn new(key_count: usize) -> Self {
        let mut s = Snapshot::default();
        s.keys_down.resize(key_count, false);
        s.keys_pressed.resize(key_count, false);
        s.keys_released.resize(key_count, false);

        Self {
            snap: Arc::new(RwLock::new(s)),
        }
    }

    #[inline]
    pub fn publish_from_state(&self, st: &InputState) {
        let mut g = self.snap.write().expect("InputApi snapshot poisoned");

        g.keys_down.clone_from(&st.keys_down);
        g.keys_pressed.clone_from(&st.keys_pressed);
        g.keys_released.clone_from(&st.keys_released);

        g.mouse_x = st.mouse_x;
        g.mouse_y = st.mouse_y;
        g.mouse_dx = st.mouse_dx;
        g.mouse_dy = st.mouse_dy;
        g.wheel_dx = st.wheel_dx;
        g.wheel_dy = st.wheel_dy;

        g.mouse_down_bits = st.mouse_down_bits;
        g.mouse_pressed_bits = st.mouse_pressed_bits;
        g.mouse_released_bits = st.mouse_released_bits;

        g.text.clone_from(&st.text);
        g.ime_preedit.clone_from(&st.ime_preedit);
        g.ime_commit.clone_from(&st.ime_commit);

        g.gamepad_events.clone_from(&st.gamepad_events);
    }

    #[inline]
    pub fn as_dyn(self) -> Arc<dyn InputApi> {
        Arc::new(self)
    }
}

impl InputApi for InputApiImpl {
    #[inline(always)]
    fn key_down(&self, key: KeyCode) -> bool {
        let g = self.snap.read().expect("InputApi snapshot poisoned");
        g.keys_down.get(key.to_index()).copied().unwrap_or(false)
    }

    #[inline(always)]
    fn key_pressed(&self, key: KeyCode) -> bool {
        let g = self.snap.read().expect("InputApi snapshot poisoned");
        g.keys_pressed.get(key.to_index()).copied().unwrap_or(false)
    }

    #[inline(always)]
    fn key_released(&self, key: KeyCode) -> bool {
        let g = self.snap.read().expect("InputApi snapshot poisoned");
        g.keys_released.get(key.to_index()).copied().unwrap_or(false)
    }

    #[inline(always)]
    fn mouse_pos(&self) -> (f32, f32) {
        let g = self.snap.read().expect("InputApi snapshot poisoned");
        (g.mouse_x, g.mouse_y)
    }

    #[inline(always)]
    fn mouse_delta(&self) -> (f32, f32) {
        let g = self.snap.read().expect("InputApi snapshot poisoned");
        (g.mouse_dx, g.mouse_dy)
    }

    #[inline(always)]
    fn wheel_delta(&self) -> (f32, f32) {
        let g = self.snap.read().expect("InputApi snapshot poisoned");
        (g.wheel_dx, g.wheel_dy)
    }

    #[inline(always)]
    fn mouse_down(&self, btn: MouseButton) -> bool {
        let g = self.snap.read().expect("InputApi snapshot poisoned");
        (g.mouse_down_bits & crate::state::mouse_bit(btn)) != 0
    }

    #[inline(always)]
    fn mouse_pressed(&self, btn: MouseButton) -> bool {
        let g = self.snap.read().expect("InputApi snapshot poisoned");
        (g.mouse_pressed_bits & crate::state::mouse_bit(btn)) != 0
    }

    #[inline(always)]
    fn mouse_released(&self, btn: MouseButton) -> bool {
        let g = self.snap.read().expect("InputApi snapshot poisoned");
        (g.mouse_released_bits & crate::state::mouse_bit(btn)) != 0
    }

    #[inline]
    fn text_chars(&self) -> Vec<char> {
        self.snap.read().expect("InputApi snapshot poisoned").text.clone()
    }

    #[inline]
    fn ime_preedit(&self) -> String {
        self.snap.read().expect("InputApi snapshot poisoned").ime_preedit.clone()
    }

    #[inline]
    fn ime_commit(&self) -> String {
        self.snap.read().expect("InputApi snapshot poisoned").ime_commit.clone()
    }

    #[inline]
    fn drain_gamepad_events(&self, out: &mut Vec<GamepadEvent>) {
        let mut g = self.snap.write().expect("InputApi snapshot poisoned");
        out.clear();
        out.extend(g.gamepad_events.drain(..));
    }
}