#![forbid(unsafe_op_in_unsafe_fn)]

//! Runtime input-system state table.
//!
//! This is intentionally engine-side runtime state, not a backend plugin API.
//! Raw input can remain available while semantic systems are disabled or captured
//! by gameplay state (pause menu, cutscene, loading gate, dialogue, scripted camera).

use newengine_input_actions_api::{CameraViewRequest, InputActionFrame, move_mask};
use newengine_ui::UiInputFrame;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InputRuntimeSystem {
    RawInput,
    Bindings,
    Actions,
    Contexts,
    Gamepad,
    CameraLook,
    GameplayMovement,
    PauseMenu,
}

impl InputRuntimeSystem {
    pub const ALL: [InputRuntimeSystem; 8] = [
        InputRuntimeSystem::RawInput,
        InputRuntimeSystem::Bindings,
        InputRuntimeSystem::Actions,
        InputRuntimeSystem::Contexts,
        InputRuntimeSystem::Gamepad,
        InputRuntimeSystem::CameraLook,
        InputRuntimeSystem::GameplayMovement,
        InputRuntimeSystem::PauseMenu,
    ];

    #[inline]
    pub const fn id(self) -> &'static str {
        match self {
            Self::RawInput => "engine.input.raw",
            Self::Bindings => "engine.input.bindings",
            Self::Actions => "engine.input.actions",
            Self::Contexts => "engine.input.contexts",
            Self::Gamepad => "engine.input.gamepad",
            Self::CameraLook => "engine.input.camera_look",
            Self::GameplayMovement => "engine.input.gameplay_movement",
            Self::PauseMenu => "engine.input.pause_menu",
        }
    }

    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::RawInput => "Raw device input",
            Self::Bindings => "Bindings profile",
            Self::Actions => "Semantic action frame",
            Self::Contexts => "Input context/capture stack",
            Self::Gamepad => "Gamepad backend",
            Self::CameraLook => "Camera look controls",
            Self::GameplayMovement => "Gameplay movement controls",
            Self::PauseMenu => "Pause/menu controls",
        }
    }

    #[inline]
    pub const fn owner(self) -> &'static str {
        match self {
            Self::RawInput => "newengine.input",
            Self::Bindings => "newengine-input-bindings-runtime",
            Self::Actions => "newengine-input-actions-api",
            Self::Contexts => "newengine-input-contexts-api",
            Self::Gamepad => "newengine.input/gilrs",
            Self::CameraLook => "newengine-camera-runtime",
            Self::GameplayMovement => "newengine-gameplay.player-controller",
            Self::PauseMenu => "newengine-ui-menu-runtime",
        }
    }
}

#[derive(Clone, Debug)]
pub struct InputRuntimeSystemState {
    pub system: InputRuntimeSystem,
    pub enabled: bool,
    pub active: bool,
    pub captured: bool,
    pub reason: String,
    pub frame_index: u64,
}

impl InputRuntimeSystemState {
    #[inline]
    pub fn id(&self) -> &'static str { self.system.id() }
    #[inline]
    pub fn label(&self) -> &'static str { self.system.label() }
    #[inline]
    pub fn owner(&self) -> &'static str { self.system.owner() }
}

#[derive(Clone, Debug, Default)]
pub struct InputRuntimeSystemsSnapshot {
    pub frame_index: u64,
    pub systems: Vec<InputRuntimeSystemState>,
}

impl InputRuntimeSystemsSnapshot {
    #[inline]
    pub fn is_enabled(&self, system: InputRuntimeSystem) -> bool {
        self.systems.iter().find(|s| s.system == system).map(|s| s.enabled).unwrap_or(false)
    }

    #[inline]
    pub fn is_active(&self, system: InputRuntimeSystem) -> bool {
        self.systems.iter().find(|s| s.system == system).map(|s| s.active).unwrap_or(false)
    }
}

#[derive(Clone, Debug)]
pub struct InputRuntimeSystems {
    states: Vec<InputRuntimeSystemState>,
    last_frame_summary: String,
    last_summary_frame: u64,
}

impl Default for InputRuntimeSystems {
    #[inline]
    fn default() -> Self { Self::new() }
}

impl InputRuntimeSystems {
    pub fn new() -> Self {
        let mut states = Vec::with_capacity(InputRuntimeSystem::ALL.len());
        for system in InputRuntimeSystem::ALL {
            let enabled = !matches!(system, InputRuntimeSystem::Contexts);
            states.push(InputRuntimeSystemState {
                system,
                enabled,
                active: false,
                captured: false,
                reason: if enabled {
                    "initialized".to_owned()
                } else {
                    "declared but no context stack runtime installed yet".to_owned()
                },
                frame_index: 0,
            });
        }
        let out = Self { states, last_frame_summary: String::new(), last_summary_frame: 0 };
        out.log_initial_table();
        out
    }

    pub fn snapshot(&self, frame_index: u64) -> InputRuntimeSystemsSnapshot {
        InputRuntimeSystemsSnapshot { frame_index, systems: self.states.clone() }
    }

    pub fn set_enabled(
        &mut self,
        system: InputRuntimeSystem,
        enabled: bool,
        reason: impl Into<String>,
        frame_index: u64,
    ) {
        let reason = reason.into();
        let state = self.state_mut(system);
        if state.enabled == enabled && state.reason == reason {
            return;
        }
        let old = state.enabled;
        state.enabled = enabled;
        state.reason = reason.clone();
        state.frame_index = frame_index;
        log::info!(
            "input systems: {} system='{}' owner='{}' old_enabled={} new_enabled={} frame={} reason='{}'",
            if enabled { "enabled" } else { "disabled" },
            system.id(),
            system.owner(),
            old,
            enabled,
            frame_index,
            reason,
        );
    }

    #[inline]
    pub fn is_enabled(&self, system: InputRuntimeSystem) -> bool {
        self.state(system).map(|s| s.enabled).unwrap_or(false)
    }

    /// Observe raw input and semantic action resolution before higher-level capture policy.
    pub fn observe_frame(
        &mut self,
        frame_index: u64,
        surface_input: Option<&UiInputFrame>,
        input: &mut InputActionFrameCarrier,
    ) {
        let raw_present = surface_input.is_some();
        let (keys_down, keys_pressed, mouse_motion, gamepad_connected, gamepad_activity) = surface_input
            .map(|frame| {
                (
                    frame.keys_down.len(),
                    frame.keys_pressed.len(),
                    frame.mouse_delta.0.abs() > f32::EPSILON || frame.mouse_delta.1.abs() > f32::EPSILON,
                    frame.gamepad_connected,
                    frame.has_gamepad_activity(),
                )
            })
            .unwrap_or((0, 0, false, 0, false));

        let raw_enabled = self.is_enabled(InputRuntimeSystem::RawInput);
        let bindings_enabled = self.is_enabled(InputRuntimeSystem::Bindings);
        let actions_enabled = self.is_enabled(InputRuntimeSystem::Actions);
        let gamepad_enabled = self.is_enabled(InputRuntimeSystem::Gamepad);
        let camera_look_enabled = self.is_enabled(InputRuntimeSystem::CameraLook);
        let gameplay_movement_enabled = self.is_enabled(InputRuntimeSystem::GameplayMovement);
        let pause_menu_enabled = self.is_enabled(InputRuntimeSystem::PauseMenu);

        self.transition(
            InputRuntimeSystem::RawInput,
            raw_present && raw_enabled,
            false,
            frame_index,
            if raw_present { "raw input frame received" } else { "raw input frame missing" },
        );
        self.transition(
            InputRuntimeSystem::Bindings,
            raw_present && bindings_enabled,
            false,
            frame_index,
            "bindings resolver available",
        );

        let has_actions = action_frame_has_activity(input.actions);
        self.transition(
            InputRuntimeSystem::Actions,
            actions_enabled && has_actions,
            false,
            frame_index,
            if has_actions { "semantic actions resolved" } else { "no semantic actions this frame" },
        );
        self.transition(
            InputRuntimeSystem::Gamepad,
            gamepad_enabled && (gamepad_connected > 0 || gamepad_activity),
            false,
            frame_index,
            if gamepad_connected > 0 { "gamepad connected" } else { "no gamepad activity" },
        );
        self.transition(
            InputRuntimeSystem::CameraLook,
            camera_look_enabled && ((*input.dx_px).abs() > f32::EPSILON || (*input.dy_px).abs() > f32::EPSILON || input.actions.look_axis != [0.0, 0.0]),
            false,
            frame_index,
            "camera look input evaluated",
        );
        self.transition(
            InputRuntimeSystem::GameplayMovement,
            gameplay_movement_enabled && movement_has_activity(input.actions),
            false,
            frame_index,
            "gameplay movement input evaluated",
        );
        self.transition(
            InputRuntimeSystem::PauseMenu,
            pause_menu_enabled && (input.actions.menu_toggle || input.actions.menu_accept || input.actions.menu_back || input.actions.menu_nav != [0, 0]),
            false,
            frame_index,
            "pause/menu input evaluated",
        );

        if !raw_enabled {
            input.suppress_all("raw input system disabled");
        } else {
            if !actions_enabled || !bindings_enabled {
                input.suppress_actions("semantic input disabled");
            }
            if !camera_look_enabled {
                input.suppress_camera_look("camera look system disabled");
            }
            if !gameplay_movement_enabled {
                input.suppress_gameplay_movement("gameplay movement system disabled");
            }
            if !pause_menu_enabled {
                input.suppress_pause_menu("pause menu input system disabled");
            }
            if !gamepad_enabled {
                input.suppress_gamepad_effects("gamepad input system disabled");
            }
        }

        self.log_compact_summary(
            frame_index,
            &format!(
                "raw={} keys_down={} keys_pressed={} mouse_motion={} gamepads={} gamepad_activity={} actions={} move_mask=0x{:X} look=({:.2},{:.2}) menu={}",
                raw_present,
                keys_down,
                keys_pressed,
                mouse_motion,
                gamepad_connected,
                gamepad_activity,
                input.actions.actions.len(),
                input.actions.move_mask,
                input.actions.look_axis[0],
                input.actions.look_axis[1],
                input.actions.menu_toggle || input.actions.menu_accept || input.actions.menu_back || input.actions.menu_nav != [0, 0],
            ),
        );
    }

    /// Apply modal capture after the pause menu has had first chance to read its own actions.
    pub fn apply_pause_capture(
        &mut self,
        frame_index: u64,
        blocks_gameplay: bool,
        input: &mut InputActionFrameCarrier,
    ) {
        if blocks_gameplay {
            self.transition(
                InputRuntimeSystem::GameplayMovement,
                false,
                true,
                frame_index,
                "captured by engine.pause_menu surface",
            );
            self.transition(
                InputRuntimeSystem::CameraLook,
                false,
                true,
                frame_index,
                "captured by engine.pause_menu surface",
            );
            input.suppress_runtime_controls("pause menu captures gameplay/camera input");
        }
    }

    pub fn log_explicit_snapshot(&self, frame_index: u64, reason: &str) {
        log::info!("input systems: snapshot frame={} reason='{}'", frame_index, reason);
        for state in &self.states {
            log::info!(
                "input systems: | {:31} | enabled={:<5} active={:<5} captured={:<5} owner='{}' reason='{}' |",
                state.id(),
                state.enabled,
                state.active,
                state.captured,
                state.owner(),
                state.reason,
            );
        }
    }

    fn log_initial_table(&self) {
        log::info!("input systems: initialized state table");
        for state in &self.states {
            log::info!(
                "input systems: declared system='{}' label='{}' owner='{}' enabled={} reason='{}'",
                state.id(),
                state.label(),
                state.owner(),
                state.enabled,
                state.reason,
            );
        }
    }

    fn transition(
        &mut self,
        system: InputRuntimeSystem,
        active: bool,
        captured: bool,
        frame_index: u64,
        reason: impl Into<String>,
    ) {
        let reason = reason.into();
        let state = self.state_mut(system);
        let changed = state.active != active || state.captured != captured || state.reason != reason;
        if !changed {
            return;
        }
        let old_active = state.active;
        let old_captured = state.captured;
        state.active = active;
        state.captured = captured;
        state.reason = reason.clone();
        state.frame_index = frame_index;
        log::debug!(
            "input systems: transition system='{}' enabled={} active {}->{} captured {}->{} frame={} reason='{}'",
            system.id(),
            state.enabled,
            old_active,
            active,
            old_captured,
            captured,
            frame_index,
            reason,
        );
    }

    fn log_compact_summary(&mut self, frame_index: u64, summary: &str) {
        let should_log = summary != self.last_frame_summary || frame_index.saturating_sub(self.last_summary_frame) >= 120;
        if !should_log {
            return;
        }
        self.last_frame_summary.clear();
        self.last_frame_summary.push_str(summary);
        self.last_summary_frame = frame_index;
        log::info!("input systems: frame={} {}", frame_index, summary);
    }

    fn state(&self, system: InputRuntimeSystem) -> Option<&InputRuntimeSystemState> {
        self.states.iter().find(|s| s.system == system)
    }

    fn state_mut(&mut self, system: InputRuntimeSystem) -> &mut InputRuntimeSystemState {
        self.states
            .iter_mut()
            .find(|s| s.system == system)
            .expect("all input runtime systems must be predeclared")
    }
}

/// Mutable carrier used by the render controller so the policy layer does not
/// need to own `ViewportInputSnap` directly.
pub struct InputActionFrameCarrier<'a> {
    pub dx_px: &'a mut f32,
    pub dy_px: &'a mut f32,
    pub wheel_y: &'a mut f32,
    pub active: &'a mut bool,
    pub look_drag: &'a mut bool,
    pub pan_drag: &'a mut bool,
    pub ui_busy: &'a mut bool,
    pub fly_rmb: &'a mut bool,
    pub move_mask: &'a mut u64,
    pub speed_scalar: &'a mut f32,
    pub camera_view: &'a mut CameraViewRequest,
    pub actions: &'a mut InputActionFrame,
}

impl InputActionFrameCarrier<'_> {
    fn suppress_all(&mut self, reason: &str) {
        log::debug!("input systems: suppress all reason='{}'", reason);
        self.suppress_runtime_controls(reason);
        self.suppress_actions(reason);
    }

    fn suppress_actions(&mut self, reason: &str) {
        log::debug!("input systems: suppress semantic actions reason='{}'", reason);
        *self.move_mask = 0;
        *self.camera_view = CameraViewRequest::None;
        *self.actions = InputActionFrame::default();
    }

    fn suppress_camera_look(&mut self, reason: &str) {
        log::debug!("input systems: suppress camera look reason='{}'", reason);
        *self.dx_px = 0.0;
        *self.dy_px = 0.0;
        self.actions.look_axis = [0.0, 0.0];
    }

    fn suppress_gameplay_movement(&mut self, reason: &str) {
        log::debug!("input systems: suppress gameplay movement reason='{}'", reason);
        self.actions.move_mask &= !(move_mask::FORWARD
            | move_mask::BACK
            | move_mask::LEFT
            | move_mask::RIGHT
            | move_mask::UP
            | move_mask::DOWN
            | move_mask::SPRINT);
        self.actions.move_axis = [0.0, 0.0, 0.0];
        self.actions.sprint = false;
        *self.move_mask = 0;
        *self.speed_scalar = 1.0;
    }

    fn suppress_pause_menu(&mut self, reason: &str) {
        log::debug!("input systems: suppress pause menu actions reason='{}'", reason);
        self.actions.menu_toggle = false;
        self.actions.menu_accept = false;
        self.actions.menu_back = false;
        self.actions.menu_nav = [0, 0];
    }

    fn suppress_gamepad_effects(&mut self, reason: &str) {
        log::debug!("input systems: suppress gamepad-derived axes reason='{}'", reason);
        self.actions.look_axis = [0.0, 0.0];
        self.suppress_gameplay_movement(reason);
    }

    fn suppress_runtime_controls(&mut self, reason: &str) {
        log::debug!("input systems: suppress runtime controls reason='{}'", reason);
        *self.dx_px = 0.0;
        *self.dy_px = 0.0;
        *self.wheel_y = 0.0;
        *self.active = false;
        *self.look_drag = false;
        *self.pan_drag = false;
        *self.ui_busy = true;
        *self.fly_rmb = false;
        *self.move_mask = 0;
        *self.speed_scalar = 1.0;
        *self.camera_view = CameraViewRequest::None;
        self.actions.move_mask = 0;
        self.actions.move_axis = [0.0, 0.0, 0.0];
        self.actions.look_axis = [0.0, 0.0];
        self.actions.sprint = false;
        self.actions.camera_view = CameraViewRequest::None;
    }
}

#[inline]
fn action_frame_has_activity(frame: &InputActionFrame) -> bool {
    frame.move_mask != 0
        || frame.move_axis != [0.0, 0.0, 0.0]
        || frame.look_axis != [0.0, 0.0]
        || frame.sprint
        || !matches!(frame.camera_view, CameraViewRequest::None)
        || frame.menu_toggle
        || frame.menu_accept
        || frame.menu_back
        || frame.menu_nav != [0, 0]
        || !frame.actions.is_empty()
}

#[inline]
fn movement_has_activity(frame: &InputActionFrame) -> bool {
    frame.move_mask & (move_mask::FORWARD
        | move_mask::BACK
        | move_mask::LEFT
        | move_mask::RIGHT
        | move_mask::UP
        | move_mask::DOWN
        | move_mask::SPRINT) != 0
        || frame.move_axis != [0.0, 0.0, 0.0]
        || frame.sprint
}
