#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::sabi_trait::TD_Opaque;
use abi_stable::std_types::{RResult, RString};
use abi_stable::StableAbi;

use newengine_plugin_api::{
    HostApiV1, HostEventAbi, HostEventSink, HostEventSinkDyn, HostEventSink_TO, InputApiV1,
    InputApiV1Dyn, InputApiV1_TO, InputHostEventAbi, KeyCodeAbi, KeyStateAbi, MouseButtonAbi,
    PluginInfo, PluginModule, TextHostEventAbi, Vec2fAbi,
};

use std::sync::{Mutex, OnceLock};

struct State {
    keys_down: [bool; 256],
    keys_pressed: [bool; 256],
    keys_released: [bool; 256],

    mouse_pos: Vec2fAbi,
    mouse_delta: Vec2fAbi,
    wheel_delta: Vec2fAbi,

    mouse_down_bits: u32,
    mouse_pressed_bits: u32,
    mouse_released_bits: u32,

    text: String,
    ime_preedit: String,
    ime_commit: String,
}

impl Default for State {
    #[inline]
    fn default() -> Self {
        Self {
            keys_down: [false; 256],
            keys_pressed: [false; 256],
            keys_released: [false; 256],

            mouse_pos: Vec2fAbi::new(0.0, 0.0),
            mouse_delta: Vec2fAbi::new(0.0, 0.0),
            wheel_delta: Vec2fAbi::new(0.0, 0.0),

            mouse_down_bits: 0,
            mouse_pressed_bits: 0,
            mouse_released_bits: 0,

            text: String::new(),
            ime_preedit: String::new(),
            ime_commit: String::new(),
        }
    }
}

static STATE: OnceLock<Mutex<State>> = OnceLock::new();

#[inline(always)]
fn state_opt() -> Option<&'static Mutex<State>> {
    STATE.get()
}

#[inline(always)]
fn key_idx(k: KeyCodeAbi) -> usize {
    (k as usize).min(255)
}

#[inline(always)]
fn mouse_bit(b: MouseButtonAbi) -> u32 {
    match b {
        MouseButtonAbi::Left => 1 << 0,
        MouseButtonAbi::Right => 1 << 1,
        MouseButtonAbi::Middle => 1 << 2,
        MouseButtonAbi::Other(n) => {
            let v = (n as u32).min(28);
            1 << (3 + v)
        }
    }
}

#[inline(always)]
fn ok_unit() -> RResult<(), RString> {
    RResult::ROk(())
}

#[derive(StableAbi)]
#[repr(C)]
pub struct InputPlugin;

impl Default for InputPlugin {
    #[inline]
    fn default() -> Self {
        Self
    }
}

impl PluginModule for InputPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            id: RString::from(env!("CARGO_PKG_NAME")),
            version: RString::from(env!("CARGO_PKG_VERSION")),
        }
    }

    fn init(&mut self, host: HostApiV1) -> RResult<(), RString> {
        // Initialize global state once. If it already exists (hot reload / re-init), reset it.
        if STATE.set(Mutex::new(State::default())).is_err() {
            if let Some(m) = state_opt() {
                if let Ok(mut g) = m.lock() {
                    *g = State::default();
                }
            }
        }

        let sink: HostEventSinkDyn<'static> =
            HostEventSink_TO::from_value(InputHostSink, TD_Opaque);

        match (host.subscribe_host_events)(sink).into_result() {
            Ok(()) => {}
            Err(e) => return RResult::RErr(e),
        }

        let api: InputApiV1Dyn<'static> = InputApiV1_TO::from_value(InputApi, TD_Opaque);

        match (host.provide_input_api_v1)(api).into_result() {
            Ok(()) => {}
            Err(e) => return RResult::RErr(e),
        }

        ok_unit()
    }

    fn start(&mut self) -> RResult<(), RString> {
        ok_unit()
    }

    fn fixed_update(&mut self, _dt: f32) -> RResult<(), RString> {
        ok_unit()
    }

    fn update(&mut self, _dt: f32) -> RResult<(), RString> {
        // Per-frame cleanup: deltas + edge flags.
        let Some(m) = state_opt() else { return ok_unit(); };
        let Ok(mut g) = m.lock() else { return ok_unit(); };

        g.mouse_delta = Vec2fAbi::new(0.0, 0.0);
        g.wheel_delta = Vec2fAbi::new(0.0, 0.0);

        g.mouse_pressed_bits = 0;
        g.mouse_released_bits = 0;

        for v in g.keys_pressed.iter_mut() {
            *v = false;
        }
        for v in g.keys_released.iter_mut() {
            *v = false;
        }

        ok_unit()
    }

    fn render(&mut self, _dt: f32) -> RResult<(), RString> {
        ok_unit()
    }

    fn shutdown(&mut self) {
        // Keep STATE allocated; the DLL will be unloaded anyway.
    }
}

#[derive(StableAbi)]
#[repr(C)]
struct InputHostSink;

impl HostEventSink for InputHostSink {
    fn on_host_event(&mut self, ev: HostEventAbi) {
        let Some(m) = state_opt() else { return; };
        let Ok(mut g) = m.lock() else { return; };

        match ev {
            HostEventAbi::Input(ie) => match ie {
                InputHostEventAbi::Key { code, state, repeat } => {
                    let i = key_idx(code);
                    let was_down = g.keys_down[i];
                    let is_down = state == KeyStateAbi::Pressed;

                    g.keys_down[i] = is_down;

                    if repeat {
                        // Repeat does not emit edges.
                        return;
                    }

                    g.keys_pressed[i] = is_down && !was_down;
                    g.keys_released[i] = !is_down && was_down;
                }

                InputHostEventAbi::MouseMove { pos } => {
                    g.mouse_pos = pos;
                }

                InputHostEventAbi::MouseDelta { delta } => {
                    g.mouse_delta =
                        Vec2fAbi::new(g.mouse_delta.x + delta.x, g.mouse_delta.y + delta.y);
                }

                InputHostEventAbi::MouseWheel { delta } => {
                    g.wheel_delta =
                        Vec2fAbi::new(g.wheel_delta.x + delta.x, g.wheel_delta.y + delta.y);
                }

                InputHostEventAbi::MouseButton { button, state } => {
                    let bit = mouse_bit(button);
                    let was = (g.mouse_down_bits & bit) != 0;
                    let is = state == KeyStateAbi::Pressed;

                    if is {
                        g.mouse_down_bits |= bit;
                    } else {
                        g.mouse_down_bits &= !bit;
                    }

                    if is && !was {
                        g.mouse_pressed_bits |= bit;
                    }
                    if !is && was {
                        g.mouse_released_bits |= bit;
                    }
                }
            },

            HostEventAbi::Text(te) => match te {
                TextHostEventAbi::CharU32(cp) => {
                    if let Some(ch) = char::from_u32(cp) {
                        g.text.push(ch);
                    }
                }
                TextHostEventAbi::ImePreedit(s) => {
                    g.ime_preedit.clear();
                    g.ime_preedit.push_str(&s);
                }
                TextHostEventAbi::ImeCommit(s) => {
                    g.ime_commit.clear();
                    g.ime_commit.push_str(&s);
                }
            },

            HostEventAbi::Window(_) => {}
        }
    }
}

#[derive(StableAbi)]
#[repr(C)]
struct InputApi;

impl InputApiV1 for InputApi {
    fn key_down(&self, key: KeyCodeAbi) -> bool {
        let Some(m) = state_opt() else { return false; };
        let Ok(g) = m.lock() else { return false; };
        g.keys_down[key_idx(key)]
    }

    fn key_pressed(&self, key: KeyCodeAbi) -> bool {
        let Some(m) = state_opt() else { return false; };
        let Ok(g) = m.lock() else { return false; };
        g.keys_pressed[key_idx(key)]
    }

    fn key_released(&self, key: KeyCodeAbi) -> bool {
        let Some(m) = state_opt() else { return false; };
        let Ok(g) = m.lock() else { return false; };
        g.keys_released[key_idx(key)]
    }

    fn mouse_pos(&self) -> Vec2fAbi {
        let Some(m) = state_opt() else { return Vec2fAbi::new(0.0, 0.0); };
        let Ok(g) = m.lock() else { return Vec2fAbi::new(0.0, 0.0); };
        g.mouse_pos
    }

    fn mouse_delta(&self) -> Vec2fAbi {
        let Some(m) = state_opt() else { return Vec2fAbi::new(0.0, 0.0); };
        let Ok(g) = m.lock() else { return Vec2fAbi::new(0.0, 0.0); };
        g.mouse_delta
    }

    fn wheel_delta(&self) -> Vec2fAbi {
        let Some(m) = state_opt() else { return Vec2fAbi::new(0.0, 0.0); };
        let Ok(g) = m.lock() else { return Vec2fAbi::new(0.0, 0.0); };
        g.wheel_delta
    }

    fn mouse_down(&self, btn: MouseButtonAbi) -> bool {
        let Some(m) = state_opt() else { return false; };
        let Ok(g) = m.lock() else { return false; };
        (g.mouse_down_bits & mouse_bit(btn)) != 0
    }

    fn mouse_pressed(&self, btn: MouseButtonAbi) -> bool {
        let Some(m) = state_opt() else { return false; };
        let Ok(g) = m.lock() else { return false; };
        (g.mouse_pressed_bits & mouse_bit(btn)) != 0
    }

    fn mouse_released(&self, btn: MouseButtonAbi) -> bool {
        let Some(m) = state_opt() else { return false; };
        let Ok(g) = m.lock() else { return false; };
        (g.mouse_released_bits & mouse_bit(btn)) != 0
    }

    fn text_take(&self) -> RString {
        let Some(m) = state_opt() else { return RString::new(); };
        let Ok(mut g) = m.lock() else { return RString::new(); };
        RString::from(std::mem::take(&mut g.text))
    }

    fn ime_preedit(&self) -> RString {
        let Some(m) = state_opt() else { return RString::new(); };
        let Ok(g) = m.lock() else { return RString::new(); };
        RString::from(g.ime_preedit.as_str())
    }

    fn ime_commit_take(&self) -> RString {
        let Some(m) = state_opt() else { return RString::new(); };
        let Ok(mut g) = m.lock() else { return RString::new(); };
        RString::from(std::mem::take(&mut g.ime_commit))
    }
}