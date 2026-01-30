use crate::api::{InputApi, InputApiImpl};
use crate::state::{GamepadAxis, GamepadButton, GamepadEvent, GamepadId, InputState};

use std::collections::HashMap;
use std::sync::Arc;

use log::{info, warn};
use newengine_core::host_events::{HostEvent, KeyCode};
use newengine_core::{EngineResult, Module, ModuleCtx};

#[derive(Debug, Clone)]
pub struct InputModuleConfig {
    pub enable_ime: bool,
    pub max_text_chars_per_frame: usize,
}

impl Default for InputModuleConfig {
    fn default() -> Self {
        Self {
            enable_ime: true,
            max_text_chars_per_frame: 128,
        }
    }
}

pub struct InputHandlerModule {
    cfg: InputModuleConfig,
    state: InputState,
    api: Arc<InputApiImpl>,
    sub: Option<newengine_core::events::EventSub<HostEvent>>,
    queue: Vec<std::sync::Arc<HostEvent>>,

    #[cfg(feature = "gamepad")]
    gilrs: Option<gilrs::Gilrs>,
    #[cfg(feature = "gamepad")]
    next_gamepad_id: u32,
    #[cfg(feature = "gamepad")]
    gamepad_ids: HashMap<gilrs::GamepadId, GamepadId>,
}

impl InputHandlerModule {
    #[inline]
    pub fn new(cfg: InputModuleConfig) -> Self {
        let key_count = (KeyCode::Unknown.to_index() + 1).max(256);
        let text_cap = cfg.max_text_chars_per_frame;

        let api = Arc::new(InputApiImpl::new(key_count));

        #[cfg(feature = "gamepad")]
        let gilrs = match gilrs::Gilrs::new() {
            Ok(g) => Some(g),
            Err(e) => {
                warn!("gilrs init failed: {}", e);
                None
            }
        };

        Self {
            cfg,
            state: InputState::new(key_count, text_cap),
            api,
            sub: None,
            queue: Vec::new(),
            #[cfg(feature = "gamepad")]
            gilrs,
            #[cfg(feature = "gamepad")]
            next_gamepad_id: 1,
            #[cfg(feature = "gamepad")]
            gamepad_ids: HashMap::new(),
        }
    }
}

impl<E: Send + 'static> Module<E> for InputHandlerModule {
    fn id(&self) -> &'static str {
        "input-handler"
    }

    fn dependencies(&self) -> &'static [&'static str] {
        &[]
    }

    fn init(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        self.sub = Some(ctx.events().subscribe::<HostEvent>());

        let api: Arc<dyn InputApi> = self.api.clone();
        ctx.resources_mut().insert::<Arc<dyn InputApi>>(api);

        Ok(())
    }

    fn update(&mut self, _ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        self.state.begin_frame();

        if let Some(sub) = &self.sub {
            self.queue.clear();
            sub.drain_into(&mut self.queue);

            for ev in self.queue.iter() {
                self.state.apply(ev.as_ref(), self.cfg.enable_ime);
            }
        }

        #[cfg(feature = "gamepad")]
        {
            if let Some(g) = self.gilrs.as_mut() {
                poll_gilrs(
                    g,
                    &mut self.next_gamepad_id,
                    &mut self.gamepad_ids,
                    &mut self.state.gamepad_events,
                );
            }
        }

        self.api.publish_from_state(&self.state);
        Ok(())
    }
}

#[cfg(feature = "gamepad")]
#[inline]
fn poll_gilrs(
    g: &mut gilrs::Gilrs,
    next_id: &mut u32,
    ids: &mut HashMap<gilrs::GamepadId, GamepadId>,
    out: &mut Vec<GamepadEvent>,
) {
    while let Some(ev) = g.next_event() {
        if let Some(mapped) = map_gilrs_event(&ev, next_id, ids) {
            match &mapped {
                GamepadEvent::Connected { id } => info!("gamepad connected: {:?}", id),
                GamepadEvent::Disconnected { id } => info!("gamepad disconnected: {:?}", id),
                _ => {}
            }
            out.push(mapped);
        }
    }
}

#[cfg(feature = "gamepad")]
#[inline]
fn map_engine_gamepad_id(
    next_id: &mut u32,
    ids: &mut HashMap<gilrs::GamepadId, GamepadId>,
    gid: gilrs::GamepadId,
) -> GamepadId {
    if let Some(v) = ids.get(&gid) {
        return *v;
    }
    let v = GamepadId(*next_id);
    *next_id = next_id.wrapping_add(1).max(1);
    ids.insert(gid, v);
    v
}

#[cfg(feature = "gamepad")]
#[inline]
fn map_gilrs_event(
    ev: &gilrs::Event,
    next_id: &mut u32,
    ids: &mut HashMap<gilrs::GamepadId, GamepadId>,
) -> Option<GamepadEvent> {
    use gilrs::EventType;

    let id = map_engine_gamepad_id(next_id, ids, ev.id);

    match ev.event {
        EventType::Connected => Some(GamepadEvent::Connected { id }),
        EventType::Disconnected => {
            ids.remove(&ev.id);
            Some(GamepadEvent::Disconnected { id })
        }

        EventType::ButtonPressed(b, _) => Some(GamepadEvent::Button {
            id,
            button: map_gilrs_button(b),
            pressed: true,
        }),
        EventType::ButtonReleased(b, _) => Some(GamepadEvent::Button {
            id,
            button: map_gilrs_button(b),
            pressed: false,
        }),

        EventType::AxisChanged(a, v, _) => Some(GamepadEvent::Axis {
            id,
            axis: map_gilrs_axis(a),
            value: v,
        }),

        _ => None,
    }
}

#[cfg(feature = "gamepad")]
#[inline]
fn map_gilrs_button(b: gilrs::Button) -> GamepadButton {
    use gilrs::Button::*;
    match b {
        South => GamepadButton::South,
        East => GamepadButton::East,
        West => GamepadButton::West,
        North => GamepadButton::North,
        Start => GamepadButton::Start,
        Select => GamepadButton::Select,
        Mode => GamepadButton::Mode,
        LeftTrigger => GamepadButton::L1,
        RightTrigger => GamepadButton::R1,
        LeftTrigger2 => GamepadButton::L2,
        RightTrigger2 => GamepadButton::R2,
        LeftThumb => GamepadButton::L3,
        RightThumb => GamepadButton::R3,
        DPadUp => GamepadButton::DPadUp,
        DPadDown => GamepadButton::DPadDown,
        DPadLeft => GamepadButton::DPadLeft,
        DPadRight => GamepadButton::DPadRight,
        Unknown => GamepadButton::Other(0),
        _ => GamepadButton::Other(1),
    }
}

#[cfg(feature = "gamepad")]
#[inline]
#[allow(unreachable_patterns)]
fn map_gilrs_axis(a: gilrs::Axis) -> GamepadAxis {
    use gilrs::Axis::*;
    match a {
        LeftStickX => GamepadAxis::LeftStickX,
        LeftStickY => GamepadAxis::LeftStickY,
        RightStickX => GamepadAxis::RightStickX,
        RightStickY => GamepadAxis::RightStickY,
        LeftZ => GamepadAxis::LeftZ,
        RightZ => GamepadAxis::RightZ,
        DPadX => GamepadAxis::DPadX,
        DPadY => GamepadAxis::DPadY,
        Unknown => GamepadAxis::Other(0),
        _ => GamepadAxis::Other(1),
    }
}