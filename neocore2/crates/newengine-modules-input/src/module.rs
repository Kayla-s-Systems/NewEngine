#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::sabi_trait::TD_Opaque;
use abi_stable::std_types::{RResult, RString, RVec};
use abi_stable::StableAbi;

use newengine_plugin_api::{
    Blob, EventSinkV1, EventSinkV1Dyn, EventSinkV1_TO, HostApiV1, MethodName, PluginInfo,
    PluginModule, ServiceV1, ServiceV1Dyn, ServiceV1_TO,
};

use gilrs::{EventType, Gilrs};
use parking_lot::Mutex;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

/* =============================================================================================
   Internal state (plugin-owned schema)
   ============================================================================================= */

#[derive(Default)]
struct KeyState {
    down: BTreeSet<u32>,
    pressed: BTreeSet<u32>,
    released: BTreeSet<u32>,
}

#[derive(Default)]
struct MouseState {
    x: f32,
    y: f32,
    dx: f32,
    dy: f32,
    wheel_x: f32,
    wheel_y: f32,
    down: BTreeSet<u32>,
    pressed: BTreeSet<u32>,
    released: BTreeSet<u32>,
}

#[derive(Default)]
struct TextState {
    text: String,
    ime_preedit: String,
    ime_commit: String,
}

#[derive(Default)]
struct GamepadState {
    connected: bool,
    buttons: BTreeMap<String, f32>,
    axes: BTreeMap<String, f32>,
}

#[derive(Default)]
struct State {
    keys: KeyState,
    mouse: MouseState,
    text: TextState,
    gamepads: BTreeMap<String, GamepadState>,
}

static STATE: OnceLock<Mutex<State>> = OnceLock::new();

#[inline]
fn state() -> &'static Mutex<State> {
    STATE.get_or_init(|| Mutex::new(State::default()))
}

/* =============================================================================================
   Incoming event JSON (sent by host/platform plugin)
   ============================================================================================= */

#[derive(Debug, Deserialize)]
struct KeyEventJson {
    key: u32,
    #[serde(default)]
    scancode: u32,
    state: String,
    #[serde(default)]
    repeat: bool,
}

#[derive(Debug, Deserialize)]
struct MouseMoveJson {
    x: f32,
    y: f32,
}

#[derive(Debug, Deserialize)]
struct MouseDeltaJson {
    dx: f32,
    dy: f32,
}

#[derive(Debug, Deserialize)]
struct MouseWheelJson {
    dx: f32,
    dy: f32,
}

#[derive(Debug, Deserialize)]
struct MouseButtonJson {
    button: u32,
    state: String,
}

/* =============================================================================================
   Event sink
   ============================================================================================= */

#[derive(StableAbi)]
#[repr(C)]
struct InputEventSink;

impl EventSinkV1 for InputEventSink {
    fn on_event(&mut self, topic: RString, payload: Blob) {
        let topic = topic.as_str();
        let bytes: Vec<u8> = payload.into_vec();

        let Ok(text) = std::str::from_utf8(&bytes) else {
            return;
        };

        let Ok(v) = serde_json::from_str::<Value>(text) else {
            return;
        };

        match topic {
            "winit.key" => {
                let Ok(ev) = serde_json::from_value::<KeyEventJson>(v) else {
                    return;
                };

                let mut g = state().lock();
                let was_down = g.keys.down.contains(&ev.key);
                let is_down = ev.state.eq_ignore_ascii_case("pressed");

                if is_down {
                    g.keys.down.insert(ev.key);
                } else {
                    g.keys.down.remove(&ev.key);
                }

                if !ev.repeat {
                    if is_down && !was_down {
                        g.keys.pressed.insert(ev.key);
                    }
                    if !is_down && was_down {
                        g.keys.released.insert(ev.key);
                    }
                }
            }

            "winit.mouse_move" => {
                let Ok(ev) = serde_json::from_value::<MouseMoveJson>(v) else {
                    return;
                };
                let mut g = state().lock();
                g.mouse.x = ev.x;
                g.mouse.y = ev.y;
            }

            "winit.mouse_delta" => {
                let Ok(ev) = serde_json::from_value::<MouseDeltaJson>(v) else {
                    return;
                };
                let mut g = state().lock();
                g.mouse.dx += ev.dx;
                g.mouse.dy += ev.dy;
            }

            "winit.mouse_wheel" => {
                let Ok(ev) = serde_json::from_value::<MouseWheelJson>(v) else {
                    return;
                };
                let mut g = state().lock();
                g.mouse.wheel_x += ev.dx;
                g.mouse.wheel_y += ev.dy;
            }

            "winit.mouse_button" => {
                let Ok(ev) = serde_json::from_value::<MouseButtonJson>(v) else {
                    return;
                };

                let mut g = state().lock();
                let was_down = g.mouse.down.contains(&ev.button);
                let is_down = ev.state.eq_ignore_ascii_case("pressed");

                if is_down {
                    g.mouse.down.insert(ev.button);
                } else {
                    g.mouse.down.remove(&ev.button);
                }

                if is_down && !was_down {
                    g.mouse.pressed.insert(ev.button);
                }
                if !is_down && was_down {
                    g.mouse.released.insert(ev.button);
                }
            }

            "winit.text_char" => {
                if let Some(cp) = v.get("cp").and_then(|x| x.as_u64()) {
                    if let Some(ch) = char::from_u32(cp as u32) {
                        let mut g = state().lock();
                        g.text.text.push(ch);
                    }
                }
            }

            "winit.ime_preedit" => {
                if let Some(s) = v.get("text").and_then(|x| x.as_str()) {
                    let mut g = state().lock();
                    g.text.ime_preedit.clear();
                    g.text.ime_preedit.push_str(s);
                }
            }

            "winit.ime_commit" => {
                if let Some(s) = v.get("text").and_then(|x| x.as_str()) {
                    let mut g = state().lock();
                    g.text.ime_commit.clear();
                    g.text.ime_commit.push_str(s);
                }
            }

            _ => {}
        }
    }
}

/* =============================================================================================
   Service (capability)
   ============================================================================================= */

#[derive(StableAbi)]
#[repr(C)]
struct InputService;

impl InputService {
    fn snapshot_json() -> String {
        let g = state().lock();

        let keys_down: Vec<u32> = g.keys.down.iter().copied().collect();
        let keys_pressed: Vec<u32> = g.keys.pressed.iter().copied().collect();
        let keys_released: Vec<u32> = g.keys.released.iter().copied().collect();

        let mouse_down: Vec<u32> = g.mouse.down.iter().copied().collect();
        let mouse_pressed: Vec<u32> = g.mouse.pressed.iter().copied().collect();
        let mouse_released: Vec<u32> = g.mouse.released.iter().copied().collect();

        let pads = g
            .gamepads
            .iter()
            .map(|(id, st)| {
                (
                    id.clone(),
                    json!({
                        "connected": st.connected,
                        "buttons": st.buttons,
                        "axes": st.axes,
                    }),
                )
            })
            .collect::<BTreeMap<_, _>>();

        json!({
            "keys": {
                "down": keys_down,
                "pressed": keys_pressed,
                "released": keys_released,
            },
            "mouse": {
                "pos": { "x": g.mouse.x, "y": g.mouse.y },
                "delta": { "x": g.mouse.dx, "y": g.mouse.dy },
                "wheel": { "x": g.mouse.wheel_x, "y": g.mouse.wheel_y },
                "down": mouse_down,
                "pressed": mouse_pressed,
                "released": mouse_released,
            },
            "text": {
                "buffer": g.text.text,
                "ime_preedit": g.text.ime_preedit,
                "ime_commit": g.text.ime_commit,
            },
            "gamepads": pads
        })
            .to_string()
    }

    fn take_text_json() -> String {
        let mut g = state().lock();
        let text = std::mem::take(&mut g.text.text);
        json!({ "text": text }).to_string()
    }

    fn take_ime_commit_json() -> String {
        let mut g = state().lock();
        let text = std::mem::take(&mut g.text.ime_commit);
        json!({ "ime_commit": text }).to_string()
    }
}

impl ServiceV1 for InputService {
    fn id(&self) -> RString {
        RString::from("kalitech.input.v1")
    }

    fn describe(&self) -> RString {
        RString::from(
            r#"{
  "id":"kalitech.input.v1",
  "methods":{
    "state_json":{"in":"{}","out":"input state snapshot as JSON"},
    "text_take_json":{"in":"{}","out":"{text:string} and clears internal text buffer"},
    "ime_commit_take_json":{"in":"{}","out":"{ime_commit:string} and clears internal commit buffer"}
  },
  "events_expected":{
    "winit.key":"{key:u32, scancode?:u32, state:'pressed'|'released', repeat?:bool}",
    "winit.mouse_move":"{x:f32,y:f32}",
    "winit.mouse_delta":"{dx:f32,dy:f32}",
    "winit.mouse_button":"{button:u32,state:'pressed'|'released'}",
    "winit.mouse_wheel":"{dx:f32,dy:f32}",
    "winit.text_char":"{cp:u32}",
    "winit.ime_preedit":"{text:string}",
    "winit.ime_commit":"{text:string}"
  }
}"#,
        )
    }

    fn call(&self, method: MethodName, _payload: Blob) -> RResult<Blob, RString> {
        match method.as_str() {
            "state_json" => RResult::ROk(RVec::from(InputService::snapshot_json().into_bytes())),
            "text_take_json" => RResult::ROk(RVec::from(InputService::take_text_json().into_bytes())),
            "ime_commit_take_json" => {
                RResult::ROk(RVec::from(InputService::take_ime_commit_json().into_bytes()))
            }
            _ => RResult::RErr(RString::from(format!(
                "input: unknown method '{}'",
                method
            ))),
        }
    }
}

/* =============================================================================================
   Plugin module
   ============================================================================================= */

pub struct InputPlugin {
    // FIX: Gilrs is not Sync; keep it behind a Mutex so InputPlugin becomes Sync.
    gilrs: Mutex<Option<Gilrs>>,
}

impl Default for InputPlugin {
    fn default() -> Self {
        let g = Gilrs::new().ok();
        Self {
            gilrs: Mutex::new(g),
        }
    }
}

impl InputPlugin {
    fn poll_gilrs(&self) {
        let mut lock = self.gilrs.lock();
        let Some(gilrs) = lock.as_mut() else { return; };

        while let Some(ev) = gilrs.next_event() {
            let id = format!("{:?}", ev.id);

            let mut g = state().lock();
            let st = g.gamepads.entry(id).or_default();

            match ev.event {
                EventType::Connected => {
                    st.connected = true;
                }
                EventType::Disconnected => {
                    st.connected = false;
                }

                EventType::ButtonPressed(b, _) => {
                    st.buttons.insert(format!("{:?}", b), 1.0);
                }
                EventType::ButtonReleased(b, _) => {
                    st.buttons.insert(format!("{:?}", b), 0.0);
                }
                EventType::ButtonChanged(b, v, _) => {
                    st.buttons.insert(format!("{:?}", b), v);
                }

                EventType::AxisChanged(a, v, _) => {
                    st.axes.insert(format!("{:?}", a), v);
                }

                _ => {}
            }
        }
    }

    fn end_frame(&self) {
        let mut g = state().lock();

        g.keys.pressed.clear();
        g.keys.released.clear();

        g.mouse.pressed.clear();
        g.mouse.released.clear();

        g.mouse.dx = 0.0;
        g.mouse.dy = 0.0;
        g.mouse.wheel_x = 0.0;
        g.mouse.wheel_y = 0.0;

        // Keep commit until taken; preedit is frame-local.
        g.text.ime_preedit.clear();
    }
}

impl PluginModule for InputPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            id: RString::from(env!("CARGO_PKG_NAME")),
            name: RString::from("NewEngine Input"),
            version: RString::from(env!("CARGO_PKG_VERSION")),
        }
    }

    fn init(&mut self, host: HostApiV1) -> RResult<(), RString> {
        let sink: EventSinkV1Dyn<'static> = EventSinkV1_TO::from_value(InputEventSink, TD_Opaque);
        if let Err(e) = (host.subscribe_events_v1)(sink).into_result() {
            return RResult::RErr(RString::from(format!(
                "input: subscribe_events_v1 failed: {}",
                e
            )));
        }

        let svc: ServiceV1Dyn<'static> = ServiceV1_TO::from_value(InputService, TD_Opaque);
        if let Err(e) = (host.register_service_v1)(svc).into_result() {
            return RResult::RErr(RString::from(format!(
                "input: register_service_v1 failed: {}",
                e
            )));
        }

        (host.log_info)(RString::from("input: initialized (events + gilrs)"));
        RResult::ROk(())
    }

    fn start(&mut self) -> RResult<(), RString> {
        RResult::ROk(())
    }

    fn fixed_update(&mut self, _dt: f32) -> RResult<(), RString> {
        RResult::ROk(())
    }

    fn update(&mut self, _dt: f32) -> RResult<(), RString> {
        self.poll_gilrs();
        self.end_frame();
        RResult::ROk(())
    }

    fn render(&mut self, _dt: f32) -> RResult<(), RString> {
        RResult::ROk(())
    }

    fn shutdown(&mut self) {}
}