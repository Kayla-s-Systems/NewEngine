#![forbid(unsafe_op_in_unsafe_fn)]

//! GameReady FPS default input action, listener and binding profile.

use newengine_input_actions_api::{
    move_mask, CameraViewRequest, InputActionDefinition, InputActionDispatchMode,
    InputActionEffect, InputActionListenerRegistration,
};
use newengine_input_api::{
    engine_default_keybind, gamepad_axis, gamepad_button, key_identity, mouse_button,
};
use newengine_input_bindings_api::{
    GamepadAxisBinding, GamepadAxisTarget, InputBinding, InputBindingDevice, InputBindingPhase,
    InputBindingsProfile, InputDevicePreference, InputKeyRegistration,
};

pub mod action;
mod action_catalog;
mod bindings;
mod gamepad_axes;
mod key_registry;
mod listeners;
mod profile;

pub use action_catalog::gameplay_default_actions;
pub use bindings::gameplay_default_bindings;
pub(crate) use bindings::{ensure_required_system_bindings, standalone_fps_bindings};
pub use gamepad_axes::gameplay_default_gamepad_axes;
pub use key_registry::gameplay_default_key_registry;
pub use listeners::gameplay_default_listeners;
pub use profile::{game_ready_game_input_profile, game_ready_input_profile};

pub const GAME_READY_INPUT_PROFILE_RUNTIME_UNIT_SPEC: newengine_service_api::EngineRuntimeUnitSpec =
    newengine_service_api::EngineRuntimeUnitSpec::new(
        "newengine.input-profile.gameready",
        1,
        newengine_service_api::EngineRuntimeUnitKind::Provider,
        &[newengine_service_api::runtime_unit_capability::GAME_INPUT_PROFILE],
        &["input.backend"],
        &[
            "engine.runtime-unit",
            "input-profile",
            "game-ready",
            "first-party",
        ],
    );

#[cfg(test)]
mod tests;
