#![forbid(unsafe_op_in_unsafe_fn)]

//! Runtime input-system state table.
//!
//! This is intentionally engine-side runtime state, not a backend plugin API.
//! Raw input can remain available while semantic systems are disabled or captured
//! by gameplay state such as UI navigation, cutscene, loading gate, dialogue or a
//! scripted camera.

mod carrier;
mod policy;
mod sample;
mod system;

use newengine_ui_api::UiInputFrame;

pub use carrier::InputActionFrameCarrier;
use carrier::{action_frame_has_activity, movement_has_activity};
use policy::{RuntimeInputCapturePolicy, SystemObservation};
use sample::RawInputSample;
pub use system::{InputRuntimeSystem, InputRuntimeSystemState, InputRuntimeSystemsSnapshot};

const SUMMARY_INTERVAL_FRAMES: u64 = 240;
const REASON_CAPTURED_BY_MODAL_UI: &str = "captured by engine.ui.modal";
const REASON_RAW_RECEIVED: &str = "raw input frame received";
const REASON_RAW_MISSING: &str = "raw input frame missing";
const REASON_BINDINGS_READY: &str = "bindings resolver available";
const REASON_ACTIONS_ACTIVE: &str = "semantic actions resolved";
const REASON_ACTIONS_IDLE: &str = "semantic actions idle";
const REASON_GAMEPAD_CONNECTED: &str = "gamepad connected";
const REASON_GAMEPAD_IDLE: &str = "gamepad idle";
const REASON_LOOK_ACTIVE: &str = "camera look input";
const REASON_MOVE_ACTIVE: &str = "gameplay movement input";
const REASON_UI_NAV_ACTIVE: &str = "ui navigation input";
const REASON_IDLE: &str = "idle";

#[derive(Clone, Debug, Default)]
struct InputSystemsLogState {
    last_frame_summary: String,
    last_summary_frame: u64,
}

#[derive(Clone, Debug)]
pub struct InputRuntimeSystems {
    states: Vec<InputRuntimeSystemState>,
    capture_policy: RuntimeInputCapturePolicy,
    log_state: InputSystemsLogState,
}

impl Default for InputRuntimeSystems {
    #[inline]
    fn default() -> Self { Self::new() }
}

impl InputRuntimeSystems {
    pub fn new() -> Self {
        let states = InputRuntimeSystem::ALL
            .into_iter()
            .map(|system| {
                let enabled = !matches!(system, InputRuntimeSystem::Contexts);
                InputRuntimeSystemState {
                    system,
                    enabled,
                    active: false,
                    captured: false,
                    reason: if enabled {
                        "initialized".to_owned()
                    } else {
                        "declared; context stack runtime not installed".to_owned()
                    },
                    frame_index: 0,
                }
            })
            .collect::<Vec<_>>();

        let out = Self {
            states,
            capture_policy: RuntimeInputCapturePolicy::default(),
            log_state: InputSystemsLogState::default(),
        };
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
            "input systems: {} system='{}' enabled {}->{} frame={} reason='{}'",
            if enabled { "enabled" } else { "disabled" },
            system.id(),
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

    /// Observe raw input and semantic action resolution before higher-level
    /// modal capture policy has first chance to consume UI/menu actions.
    pub fn observe_frame(
        &mut self,
        frame_index: u64,
        surface_input: Option<&UiInputFrame>,
        input: &mut InputActionFrameCarrier,
    ) {
        let raw_sample = RawInputSample::from_surface(surface_input);

        let raw_enabled = self.is_enabled(InputRuntimeSystem::RawInput);
        let bindings_enabled = self.is_enabled(InputRuntimeSystem::Bindings);
        let actions_enabled = self.is_enabled(InputRuntimeSystem::Actions);
        let gamepad_enabled = self.is_enabled(InputRuntimeSystem::Gamepad);
        let camera_look_enabled = self.is_enabled(InputRuntimeSystem::CameraLook);
        let gameplay_movement_enabled = self.is_enabled(InputRuntimeSystem::GameplayMovement);
        let ui_navigation_enabled = self.is_enabled(InputRuntimeSystem::UiNavigation);

        self.transition(
            InputRuntimeSystem::RawInput,
            frame_index,
            SystemObservation::new(
                raw_sample.present && raw_enabled,
                false,
                if raw_sample.present { REASON_RAW_RECEIVED } else { REASON_RAW_MISSING },
            ),
        );
        self.transition(
            InputRuntimeSystem::Bindings,
            frame_index,
            SystemObservation::new(raw_sample.present && bindings_enabled, false, REASON_BINDINGS_READY),
        );

        let has_actions = action_frame_has_activity(input.actions);
        self.transition(
            InputRuntimeSystem::Actions,
            frame_index,
            SystemObservation::new(
                actions_enabled && has_actions,
                false,
                if has_actions { REASON_ACTIONS_ACTIVE } else { REASON_ACTIONS_IDLE },
            ),
        );
        self.transition(
            InputRuntimeSystem::Gamepad,
            frame_index,
            SystemObservation::new(
                gamepad_enabled && (raw_sample.gamepad_connected > 0 || raw_sample.gamepad_activity),
                false,
                if raw_sample.gamepad_connected > 0 { REASON_GAMEPAD_CONNECTED } else { REASON_GAMEPAD_IDLE },
            ),
        );

        let camera_captured = self.capture_policy.captures(InputRuntimeSystem::CameraLook);
        let movement_captured = self.capture_policy.captures(InputRuntimeSystem::GameplayMovement);
        self.transition(
            InputRuntimeSystem::CameraLook,
            frame_index,
            SystemObservation::new(
                camera_look_enabled && !camera_captured && input_has_look(input),
                camera_captured,
                if camera_captured { REASON_CAPTURED_BY_MODAL_UI } else if input_has_look(input) { REASON_LOOK_ACTIVE } else { REASON_IDLE },
            ),
        );
        self.transition(
            InputRuntimeSystem::GameplayMovement,
            frame_index,
            SystemObservation::new(
                gameplay_movement_enabled && !movement_captured && movement_has_activity(input.actions),
                movement_captured,
                if movement_captured { REASON_CAPTURED_BY_MODAL_UI } else if movement_has_activity(input.actions) { REASON_MOVE_ACTIVE } else { REASON_IDLE },
            ),
        );
        self.transition(
            InputRuntimeSystem::UiNavigation,
            frame_index,
            SystemObservation::new(
                ui_navigation_enabled && input_has_ui_navigation_action(input),
                false,
                if input_has_ui_navigation_action(input) { REASON_UI_NAV_ACTIVE } else { REASON_IDLE },
            ),
        );

        self.apply_disabled_system_suppression(
            raw_enabled,
            bindings_enabled,
            actions_enabled,
            gamepad_enabled,
            camera_look_enabled,
            gameplay_movement_enabled,
            ui_navigation_enabled,
            input,
        );

        self.log_compact_summary(frame_index, raw_sample.summary(input));
    }

    /// Apply modal capture after the UI navigation has had first chance to read its
    /// own actions. Capture state is persistent, so an open UI navigation does not
    /// generate a false->true transition every frame.
    pub fn apply_modal_ui_capture(
        &mut self,
        frame_index: u64,
        blocks_gameplay: bool,
        input: &mut InputActionFrameCarrier,
    ) {
        let changed = self.capture_policy.set_modal_ui_capture(blocks_gameplay);
        for system in [InputRuntimeSystem::GameplayMovement, InputRuntimeSystem::CameraLook] {
            let was_active = self.state(system).map(|state| state.active).unwrap_or(false);
            self.transition(
                system,
                frame_index,
                SystemObservation::new(
                    was_active && !blocks_gameplay,
                    blocks_gameplay,
                    if blocks_gameplay { REASON_CAPTURED_BY_MODAL_UI } else { REASON_IDLE },
                ),
            );
        }

        if blocks_gameplay {
            input.suppress_runtime_controls();
            if changed {
                log::debug!(
                    "input systems: runtime controls captured surface='engine.ui.modal' frame={}",
                    frame_index
                );
            }
        } else if changed {
            log::debug!(
                "input systems: runtime controls released surface='engine.ui.modal' frame={}",
                frame_index
            );
        }
    }

    pub fn log_explicit_snapshot(&self, frame_index: u64, reason: &str) {
        log::info!("input systems: snapshot frame={} reason='{}'", frame_index, reason);
        for state in &self.states {
            log::info!(
                "input systems: | {:31} | enabled={:<5} active={:<5} captured={:<5} reason='{}' |",
                state.id(),
                state.enabled,
                state.active,
                state.captured,
                state.reason,
            );
        }
    }

    fn apply_disabled_system_suppression(
        &self,
        raw_enabled: bool,
        bindings_enabled: bool,
        actions_enabled: bool,
        gamepad_enabled: bool,
        camera_look_enabled: bool,
        gameplay_movement_enabled: bool,
        ui_navigation_enabled: bool,
        input: &mut InputActionFrameCarrier,
    ) {
        if !raw_enabled {
            input.suppress_all();
            return;
        }
        if !actions_enabled || !bindings_enabled {
            input.suppress_actions();
        }
        if !camera_look_enabled {
            input.suppress_camera_look();
        }
        if !gameplay_movement_enabled {
            input.suppress_gameplay_movement();
        }
        if !ui_navigation_enabled {
            input.suppress_ui_navigation();
        }
        if !gamepad_enabled {
            input.suppress_gamepad_effects();
        }
    }

    fn log_initial_table(&self) {
        let enabled_count = self.states.iter().filter(|state| state.enabled).count();
        let disabled = self
            .states
            .iter()
            .filter(|state| !state.enabled)
            .map(|state| state.id())
            .collect::<Vec<_>>()
            .join(",");
        log::info!(
            "input systems: initialized systems={} enabled={} disabled=[{}]",
            self.states.len(),
            enabled_count,
            disabled,
        );
        if log::log_enabled!(log::Level::Debug) {
            for state in &self.states {
                log::debug!(
                    "input systems: declared system='{}' enabled={} owner='{}' reason='{}'",
                    state.id(),
                    state.enabled,
                    state.owner(),
                    state.reason,
                );
            }
        }
    }

    fn transition(&mut self, system: InputRuntimeSystem, frame_index: u64, observed: SystemObservation) {
        let state = self.state_mut(system);
        let state_changed = state.active != observed.active || state.captured != observed.captured;
        let reason_changed = state.reason != observed.reason;
        if !state_changed && !reason_changed {
            return;
        }

        let old_active = state.active;
        let old_captured = state.captured;
        state.active = observed.active;
        state.captured = observed.captured;
        state.reason = observed.reason.to_owned();
        state.frame_index = frame_index;

        if state_changed && frame_index % 120 == 0 {
            log::trace!(
                "input systems: transition system='{}' active {}->{} captured {}->{} frame={} reason='{}'",
                system.id(),
                old_active,
                observed.active,
                old_captured,
                observed.captured,
                frame_index,
                observed.reason,
            );
        }
    }

    fn log_compact_summary(&mut self, frame_index: u64, summary: String) {
        let should_log = summary != self.log_state.last_frame_summary
            || frame_index.saturating_sub(self.log_state.last_summary_frame) >= SUMMARY_INTERVAL_FRAMES;
        if !should_log {
            return;
        }
        self.log_state.last_frame_summary = summary.clone();
        self.log_state.last_summary_frame = frame_index;
        if frame_index % 120 == 0 {
            log::trace!("input systems: frame={} {}", frame_index, summary);
        }
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

#[inline]
fn input_has_look(input: &InputActionFrameCarrier<'_>) -> bool {
    (*input.dx_px).abs() > f32::EPSILON
        || (*input.dy_px).abs() > f32::EPSILON
        || input.actions.look_axis != [0.0, 0.0]
}

#[inline]
fn input_has_ui_navigation_action(input: &InputActionFrameCarrier<'_>) -> bool {
    input.actions.ui_toggle
        || input.actions.ui_accept
        || input.actions.ui_back
        || input.actions.ui_nav != [0, 0]
}
