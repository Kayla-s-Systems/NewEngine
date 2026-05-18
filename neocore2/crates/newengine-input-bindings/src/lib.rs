#![forbid(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};

pub const ENGINE_INPUT_BINDINGS_SERVICE_ID: &str = "engine.input.bindings";
pub const INPUT_BINDINGS_SERVICE_ID: &str = "input.bindings.api";
pub const INPUT_BINDINGS_BACKEND_CAPABILITY_ID: &str = "input.bindings.backend";

pub const ENGINE_INPUT_ACTIONS_SERVICE_ID: &str = "engine.input.actions";
pub const INPUT_ACTIONS_SERVICE_ID: &str = "input.actions.api";
pub const INPUT_ACTIONS_BACKEND_CAPABILITY_ID: &str = "input.actions.backend";

pub const INPUT_BINDINGS_METHOD_INFO: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
pub const INPUT_BINDINGS_METHOD_INVOKE: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
pub const INPUT_BINDINGS_METHOD_SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
pub const INPUT_BINDINGS_METHOD_PROFILE_JSON_V1: &str = "profile_json_v1";
pub const INPUT_BINDINGS_METHOD_SAVE_PROFILE_JSON_V1: &str = "save_profile_json_v1";
pub const INPUT_BINDINGS_METHOD_RESET_PROFILE_JSON_V1: &str = "reset_profile_json_v1";

pub const INPUT_ACTIONS_METHOD_FRAME_JSON_V1: &str = "frame_json_v1";

pub const INPUT_BINDINGS_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "input.bindings",
        ENGINE_INPUT_BINDINGS_SERVICE_ID,
        INPUT_BINDINGS_SERVICE_ID,
        INPUT_BINDINGS_BACKEND_CAPABILITY_ID,
    );

pub const INPUT_BINDINGS_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_INPUT_BINDINGS_SERVICE_ID,
        "newengine.input-bindings >= 0.1.x",
        &[
            newengine_service_api::SERVICE_METHOD_INFO_JSON,
            newengine_service_api::SERVICE_METHOD_INVOKE_JSON,
            newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1,
            INPUT_BINDINGS_METHOD_PROFILE_JSON_V1,
        ],
    );

pub const INPUT_BINDINGS_RUNTIME_REQUIREMENT_SPEC: newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        INPUT_BINDINGS_RUNTIME_CONTRACT_SPEC,
        Some(INPUT_BINDINGS_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_INPUT_BINDINGS"),
    );

pub mod key_code {
    pub const DIGIT1: u32 = 6;
    pub const DIGIT2: u32 = 7;
    pub const DIGIT3: u32 = 8;

    pub const KEY_A: u32 = 19;
    pub const KEY_D: u32 = 22;
    pub const KEY_E: u32 = 23;
    pub const KEY_F: u32 = 24;
    pub const KEY_Q: u32 = 35;
    pub const KEY_S: u32 = 37;
    pub const KEY_W: u32 = 41;

    pub const SHIFT_LEFT: u32 = 60;
    pub const SHIFT_RIGHT: u32 = 61;
}

pub mod action {
    pub const PLAYER_MOVE_FORWARD: &str = "player.move.forward";
    pub const PLAYER_MOVE_BACK: &str = "player.move.back";
    pub const PLAYER_MOVE_LEFT: &str = "player.move.left";
    pub const PLAYER_MOVE_RIGHT: &str = "player.move.right";
    pub const PLAYER_MOVE_UP: &str = "player.move.up";
    pub const PLAYER_MOVE_DOWN: &str = "player.move.down";
    pub const PLAYER_SPRINT: &str = "player.sprint";

    pub const CAMERA_VIEW_NEXT: &str = "camera.view.next";
    pub const CAMERA_VIEW_PREVIOUS: &str = "camera.view.previous";
    pub const CAMERA_VIEW_FIRST_PERSON: &str = "camera.view.first_person";
    pub const CAMERA_VIEW_THIRD_PERSON_FOLLOW: &str = "camera.view.third_person.follow";
    pub const CAMERA_VIEW_THIRD_PERSON_AIM: &str = "camera.view.third_person.aim";
}

pub mod gamepad_button {
    pub const SOUTH: &str = "South";
    pub const EAST: &str = "East";
    pub const WEST: &str = "West";
    pub const NORTH: &str = "North";
    pub const LEFT_TRIGGER: &str = "LeftTrigger";
    pub const LEFT_TRIGGER_2: &str = "LeftTrigger2";
    pub const RIGHT_TRIGGER: &str = "RightTrigger";
    pub const RIGHT_TRIGGER_2: &str = "RightTrigger2";
    pub const SELECT: &str = "Select";
    pub const START: &str = "Start";
    pub const MODE: &str = "Mode";
    pub const LEFT_THUMB: &str = "LeftThumb";
    pub const RIGHT_THUMB: &str = "RightThumb";
    pub const DPAD_UP: &str = "DPadUp";
    pub const DPAD_DOWN: &str = "DPadDown";
    pub const DPAD_LEFT: &str = "DPadLeft";
    pub const DPAD_RIGHT: &str = "DPadRight";
}

pub mod gamepad_axis {
    pub const LEFT_STICK_X: &str = "LeftStickX";
    pub const LEFT_STICK_Y: &str = "LeftStickY";
    pub const RIGHT_STICK_X: &str = "RightStickX";
    pub const RIGHT_STICK_Y: &str = "RightStickY";
    pub const LEFT_Z: &str = "LeftZ";
    pub const RIGHT_Z: &str = "RightZ";
}

pub mod move_mask {
    pub const FORWARD: u64 = 1 << 0;
    pub const LEFT: u64 = 1 << 1;
    pub const BACK: u64 = 1 << 2;
    pub const RIGHT: u64 = 1 << 3;
    pub const UP: u64 = 1 << 4;
    pub const DOWN: u64 = 1 << 5;
    pub const SPRINT: u64 = 1 << 6;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputDevicePreference {
    KeyboardMouse,
    Gamepad,
    Hybrid,
}

impl Default for InputDevicePreference {
    #[inline]
    fn default() -> Self { Self::Hybrid }
}

impl InputDevicePreference {
    #[inline]
    pub fn allows_keyboard_mouse(self) -> bool {
        matches!(self, Self::KeyboardMouse | Self::Hybrid)
    }

    #[inline]
    pub fn allows_gamepad(self) -> bool {
        matches!(self, Self::Gamepad | Self::Hybrid)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputBindingPhase {
    Down,
    Pressed,
    Released,
}

impl Default for InputBindingPhase {
    #[inline]
    fn default() -> Self { Self::Down }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputBindingDevice {
    Keyboard,
    MouseButton,
    GamepadButton,
}

impl Default for InputBindingDevice {
    #[inline]
    fn default() -> Self { Self::Keyboard }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputBinding {
    pub action: String,
    #[serde(default)]
    pub device: InputBindingDevice,
    /// Numeric code for keyboard/mouse bindings.
    #[serde(default)]
    pub code: u32,
    /// Stable symbolic name for gamepad bindings, e.g. `South`, `Start`, `DPadUp`.
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub phase: InputBindingPhase,
}

impl InputBinding {
    #[inline]
    pub fn keyboard_down(action: impl Into<String>, code: u32) -> Self {
        Self { action: action.into(), device: InputBindingDevice::Keyboard, code, name: None, phase: InputBindingPhase::Down }
    }

    #[inline]
    pub fn keyboard_pressed(action: impl Into<String>, code: u32) -> Self {
        Self { action: action.into(), device: InputBindingDevice::Keyboard, code, name: None, phase: InputBindingPhase::Pressed }
    }

    #[inline]
    pub fn gamepad_button_down(action: impl Into<String>, name: impl Into<String>) -> Self {
        Self { action: action.into(), device: InputBindingDevice::GamepadButton, code: 0, name: Some(name.into()), phase: InputBindingPhase::Down }
    }

    #[inline]
    pub fn gamepad_button_pressed(action: impl Into<String>, name: impl Into<String>) -> Self {
        Self { action: action.into(), device: InputBindingDevice::GamepadButton, code: 0, name: Some(name.into()), phase: InputBindingPhase::Pressed }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GamepadAxisTarget {
    MoveX,
    MoveY,
    MoveZ,
    LookX,
    LookY,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GamepadAxisBinding {
    pub axis: String,
    pub target: GamepadAxisTarget,
    #[serde(default = "default_axis_deadzone")]
    pub deadzone: f32,
    #[serde(default = "default_axis_scale")]
    pub scale: f32,
}

#[inline]
fn default_axis_deadzone() -> f32 { 0.18 }
#[inline]
fn default_axis_scale() -> f32 { 1.0 }

impl GamepadAxisBinding {
    #[inline]
    pub fn new(axis: impl Into<String>, target: GamepadAxisTarget, scale: f32) -> Self {
        Self { axis: axis.into(), target, deadzone: default_axis_deadzone(), scale }
    }
}

pub trait InputFrameSource {
    fn is_key_down(&self, key: u32) -> bool;
    fn is_key_pressed(&self, key: u32) -> bool;
    fn is_key_released(&self, key: u32) -> bool;
    fn is_mouse_down(&self, button: u32) -> bool;
    fn is_mouse_pressed(&self, button: u32) -> bool;
    fn is_mouse_released(&self, button: u32) -> bool;

    #[inline]
    fn is_gamepad_button_down(&self, _button: &str) -> bool { false }
    #[inline]
    fn is_gamepad_button_pressed(&self, _button: &str) -> bool { false }
    #[inline]
    fn is_gamepad_button_released(&self, _button: &str) -> bool { false }
    #[inline]
    fn gamepad_axis(&self, _axis: &str) -> f32 { 0.0 }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InputBindingsProfile {
    pub id: String,
    pub version: u32,
    #[serde(default)]
    pub device_preference: InputDevicePreference,
    #[serde(default)]
    pub bindings: Vec<InputBinding>,
    #[serde(default)]
    pub gamepad_axes: Vec<GamepadAxisBinding>,
}

impl InputBindingsProfile {
    #[inline]
    pub fn gameplay_default() -> Self {
        use crate::key_code as keys;
        Self {
            id: "newengine.default.gameplay".to_owned(),
            version: 2,
            device_preference: InputDevicePreference::Hybrid,
            bindings: vec![
                InputBinding::keyboard_down(action::PLAYER_MOVE_FORWARD, keys::KEY_W),
                InputBinding::keyboard_down(action::PLAYER_MOVE_BACK, keys::KEY_S),
                InputBinding::keyboard_down(action::PLAYER_MOVE_LEFT, keys::KEY_A),
                InputBinding::keyboard_down(action::PLAYER_MOVE_RIGHT, keys::KEY_D),
                InputBinding::keyboard_down(action::PLAYER_MOVE_UP, keys::KEY_Q),
                InputBinding::keyboard_down(action::PLAYER_MOVE_DOWN, keys::KEY_E),
                InputBinding::keyboard_down(action::PLAYER_SPRINT, keys::SHIFT_LEFT),
                InputBinding::keyboard_down(action::PLAYER_SPRINT, keys::SHIFT_RIGHT),
                InputBinding::keyboard_pressed(action::CAMERA_VIEW_NEXT, keys::KEY_F),
                InputBinding::keyboard_pressed(action::CAMERA_VIEW_FIRST_PERSON, keys::DIGIT1),
                InputBinding::keyboard_pressed(action::CAMERA_VIEW_THIRD_PERSON_FOLLOW, keys::DIGIT2),
                InputBinding::keyboard_pressed(action::CAMERA_VIEW_THIRD_PERSON_AIM, keys::DIGIT3),
                InputBinding::gamepad_button_down(action::PLAYER_SPRINT, gamepad_button::LEFT_THUMB),
                InputBinding::gamepad_button_pressed(action::CAMERA_VIEW_NEXT, gamepad_button::SELECT),
                InputBinding::gamepad_button_pressed(action::CAMERA_VIEW_NEXT, gamepad_button::MODE),
                InputBinding::gamepad_button_pressed(action::CAMERA_VIEW_FIRST_PERSON, gamepad_button::DPAD_UP),
                InputBinding::gamepad_button_pressed(action::CAMERA_VIEW_THIRD_PERSON_FOLLOW, gamepad_button::DPAD_LEFT),
                InputBinding::gamepad_button_pressed(action::CAMERA_VIEW_THIRD_PERSON_AIM, gamepad_button::DPAD_RIGHT),
            ],
            gamepad_axes: vec![
                GamepadAxisBinding::new(gamepad_axis::LEFT_STICK_X, GamepadAxisTarget::MoveX, 1.0),
                GamepadAxisBinding::new(gamepad_axis::LEFT_STICK_Y, GamepadAxisTarget::MoveZ, -1.0),
                GamepadAxisBinding::new(gamepad_axis::RIGHT_STICK_X, GamepadAxisTarget::LookX, 1.0),
                GamepadAxisBinding::new(gamepad_axis::RIGHT_STICK_Y, GamepadAxisTarget::LookY, -1.0),
            ],
        }
    }

    pub fn resolve<T: InputFrameSource>(&self, input: &T) -> InputActionFrame {
        let mut out = InputActionFrame::default();
        for binding in &self.bindings {
            if !device_allowed(self.device_preference, binding.device) || !binding_matches(binding, input) {
                continue;
            }
            out.actions.push(binding.action.clone());
            apply_action(&mut out, binding.action.as_str());
        }
        if self.device_preference.allows_gamepad() {
            apply_gamepad_axes(&mut out, &self.gamepad_axes, input);
        }
        out
    }
}

impl Default for InputBindingsProfile {
    #[inline]
    fn default() -> Self { Self::gameplay_default() }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CameraViewRequest {
    None,
    Next,
    Previous,
    Set(newengine_camera_api::CameraViewMode),
}

impl Default for CameraViewRequest {
    #[inline]
    fn default() -> Self { Self::None }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InputActionFrame {
    #[serde(default)]
    pub move_mask: u64,
    #[serde(default)]
    pub move_axis: [f32; 3],
    #[serde(default)]
    pub sprint: bool,
    #[serde(default)]
    pub look_axis: [f32; 2],
    #[serde(default)]
    pub camera_view: CameraViewRequest,
    #[serde(default)]
    pub actions: Vec<String>,
}

impl Default for InputActionFrame {
    #[inline]
    fn default() -> Self {
        Self { move_mask: 0, move_axis: [0.0, 0.0, 0.0], sprint: false, look_axis: [0.0, 0.0], camera_view: CameraViewRequest::None, actions: Vec::new() }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InputBindingsServiceInfo {
    pub protocol: String,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub methods: Vec<String>,
}

impl Default for InputBindingsServiceInfo {
    fn default() -> Self {
        Self {
            protocol: "newengine.input-bindings/v1".to_owned(),
            features: vec![
                "semantic-actions".to_owned(),
                "camera-view-switching".to_owned(),
                "gamepad-bindings".to_owned(),
                "device-preference".to_owned(),
                "gameplay-move-mask-compat".to_owned(),
            ],
            methods: vec![
                INPUT_BINDINGS_METHOD_INFO.to_owned(),
                INPUT_BINDINGS_METHOD_INVOKE.to_owned(),
                INPUT_BINDINGS_METHOD_SHUTDOWN_V1.to_owned(),
                INPUT_BINDINGS_METHOD_PROFILE_JSON_V1.to_owned(),
                INPUT_BINDINGS_METHOD_SAVE_PROFILE_JSON_V1.to_owned(),
                INPUT_BINDINGS_METHOD_RESET_PROFILE_JSON_V1.to_owned(),
            ],
        }
    }
}

#[inline]
fn binding_matches<T: InputFrameSource>(binding: &InputBinding, input: &T) -> bool {
    match binding.device {
        InputBindingDevice::Keyboard => match binding.phase {
            InputBindingPhase::Down => input.is_key_down(binding.code),
            InputBindingPhase::Pressed => input.is_key_pressed(binding.code),
            InputBindingPhase::Released => input.is_key_released(binding.code),
        },
        InputBindingDevice::MouseButton => match binding.phase {
            InputBindingPhase::Down => input.is_mouse_down(binding.code),
            InputBindingPhase::Pressed => input.is_mouse_pressed(binding.code),
            InputBindingPhase::Released => input.is_mouse_released(binding.code),
        },
        InputBindingDevice::GamepadButton => {
            let Some(name) = binding.name.as_deref() else { return false; };
            match binding.phase {
                InputBindingPhase::Down => input.is_gamepad_button_down(name),
                InputBindingPhase::Pressed => input.is_gamepad_button_pressed(name),
                InputBindingPhase::Released => input.is_gamepad_button_released(name),
            }
        }
    }
}

#[inline]
fn device_allowed(preference: InputDevicePreference, device: InputBindingDevice) -> bool {
    match device {
        InputBindingDevice::Keyboard | InputBindingDevice::MouseButton => preference.allows_keyboard_mouse(),
        InputBindingDevice::GamepadButton => preference.allows_gamepad(),
    }
}

fn apply_gamepad_axes<T: InputFrameSource>(out: &mut InputActionFrame, axes: &[GamepadAxisBinding], input: &T) {
    for binding in axes {
        let raw = input.gamepad_axis(binding.axis.as_str());
        let v = apply_deadzone(raw, binding.deadzone) * binding.scale;
        if v == 0.0 {
            continue;
        }
        match binding.target {
            GamepadAxisTarget::MoveX => out.move_axis[0] = clamp_axis(out.move_axis[0] + v),
            GamepadAxisTarget::MoveY => out.move_axis[1] = clamp_axis(out.move_axis[1] + v),
            GamepadAxisTarget::MoveZ => out.move_axis[2] = clamp_axis(out.move_axis[2] + v),
            GamepadAxisTarget::LookX => out.look_axis[0] = clamp_axis(out.look_axis[0] + v),
            GamepadAxisTarget::LookY => out.look_axis[1] = clamp_axis(out.look_axis[1] + v),
        }
    }

    if out.move_axis[0] > 0.0 { out.move_mask |= move_mask::RIGHT; }
    if out.move_axis[0] < 0.0 { out.move_mask |= move_mask::LEFT; }
    if out.move_axis[1] > 0.0 { out.move_mask |= move_mask::UP; }
    if out.move_axis[1] < 0.0 { out.move_mask |= move_mask::DOWN; }
    if out.move_axis[2] > 0.0 { out.move_mask |= move_mask::FORWARD; }
    if out.move_axis[2] < 0.0 { out.move_mask |= move_mask::BACK; }
}

#[inline]
fn apply_deadzone(v: f32, deadzone: f32) -> f32 {
    let dz = deadzone.clamp(0.0, 0.95);
    if v.abs() <= dz {
        0.0
    } else {
        let sign = v.signum();
        let scaled = ((v.abs() - dz) / (1.0 - dz)).clamp(0.0, 1.0);
        scaled * sign
    }
}

#[inline]
fn clamp_axis(v: f32) -> f32 { v.clamp(-1.0, 1.0) }

fn apply_action(out: &mut InputActionFrame, action_id: &str) {
    match action_id {
        action::PLAYER_MOVE_FORWARD => out.move_mask |= move_mask::FORWARD,
        action::PLAYER_MOVE_BACK => out.move_mask |= move_mask::BACK,
        action::PLAYER_MOVE_LEFT => out.move_mask |= move_mask::LEFT,
        action::PLAYER_MOVE_RIGHT => out.move_mask |= move_mask::RIGHT,
        action::PLAYER_MOVE_UP => out.move_mask |= move_mask::UP,
        action::PLAYER_MOVE_DOWN => out.move_mask |= move_mask::DOWN,
        action::PLAYER_SPRINT => {
            out.move_mask |= move_mask::SPRINT;
            out.sprint = true;
        }
        action::CAMERA_VIEW_NEXT => out.camera_view = CameraViewRequest::Next,
        action::CAMERA_VIEW_PREVIOUS => out.camera_view = CameraViewRequest::Previous,
        action::CAMERA_VIEW_FIRST_PERSON => out.camera_view = CameraViewRequest::Set(newengine_camera_api::CameraViewMode::FirstPerson),
        action::CAMERA_VIEW_THIRD_PERSON_FOLLOW => out.camera_view = CameraViewRequest::Set(newengine_camera_api::CameraViewMode::ThirdPersonFollow),
        action::CAMERA_VIEW_THIRD_PERSON_AIM => out.camera_view = CameraViewRequest::Set(newengine_camera_api::CameraViewMode::ThirdPersonAim),
        _ => {}
    }

    out.move_axis = move_axis_from_mask(out.move_mask);
}

#[inline]
pub fn move_axis_from_mask(mask: u64) -> [f32; 3] {
    let x = ((mask & move_mask::RIGHT != 0) as i32 - (mask & move_mask::LEFT != 0) as i32) as f32;
    let y = ((mask & move_mask::UP != 0) as i32 - (mask & move_mask::DOWN != 0) as i32) as f32;
    let z = ((mask & move_mask::FORWARD != 0) as i32 - (mask & move_mask::BACK != 0) as i32) as f32;
    [x, y, z]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_profile_has_camera_view_switching() {
        let profile = InputBindingsProfile::gameplay_default();
        assert!(profile.bindings.iter().any(|b| b.action == action::CAMERA_VIEW_NEXT));
        assert!(profile.bindings.iter().any(|b| b.action == action::PLAYER_MOVE_FORWARD));
    }
}
