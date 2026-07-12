#![forbid(unsafe_op_in_unsafe_fn)]

//! Input binding/profile DTOs and `engine.input.bindings` service contract.

use serde::{Deserialize, Serialize};

pub use newengine_input_actions_api::{
    InputActionDefinition, InputActionDispatchMode, InputActionEffect, InputActionFrame,
    InputActionListenerRegistration, InputFrameSource,
};
pub use newengine_input_api::{gamepad_axis, gamepad_button, key_code};

mod axis;
mod binding;
mod contracts;
mod labels;
mod profile;
mod registration;
mod resolve;

pub use axis::{GamepadAxisBinding, GamepadAxisTarget};
pub use binding::{InputBinding, InputBindingDevice, InputBindingPhase, InputDevicePreference};
pub use contracts::*;
pub use labels::{binding_display_label, gamepad_button_label, key_code_label, mouse_button_label};
pub use profile::InputBindingsProfile;
pub use registration::{InputBindingRegistration, InputBindingsManifest, InputKeyRegistration};
pub use resolve::input_device_preference_is_display_only;
pub(crate) use resolve::{
    apply_gamepad_axes, binding_matches, bindings_equivalent, canonical_axis_binding,
    dispatch_action_definition, upsert_action_definition, upsert_key_registration,
    upsert_listener_registration,
};

#[cfg(test)]
mod tests;
