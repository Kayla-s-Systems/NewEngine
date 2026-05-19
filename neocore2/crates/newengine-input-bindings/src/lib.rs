#![forbid(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};

pub const ENGINE_INPUT_BINDINGS_SERVICE_ID: &str = "engine.input.bindings";
pub const INPUT_BINDINGS_SERVICE_ID: &str = "input.bindings.api";
pub const INPUT_BINDINGS_BACKEND_CAPABILITY_ID: &str = "input.bindings.backend";

pub const ENGINE_INPUT_ACTIONS_SERVICE_ID: &str = "engine.input.actions";
pub const INPUT_ACTIONS_SERVICE_ID: &str = "input.actions.api";
pub const INPUT_ACTIONS_BACKEND_CAPABILITY_ID: &str = "input.actions.backend";

pub const ENGINE_INPUT_CONTEXTS_SERVICE_ID: &str = "engine.input.contexts";
pub const INPUT_CONTEXTS_SERVICE_ID: &str = "input.contexts.api";
pub const INPUT_CONTEXTS_BACKEND_CAPABILITY_ID: &str = "input.contexts.backend";

pub const INPUT_BINDINGS_METHOD_INFO: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
pub const INPUT_BINDINGS_METHOD_INVOKE: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
pub const INPUT_BINDINGS_METHOD_SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
pub const INPUT_BINDINGS_METHOD_PROFILE_JSON_V1: &str = "profile_json_v1";
pub const INPUT_BINDINGS_METHOD_SAVE_PROFILE_JSON_V1: &str = "save_profile_json_v1";
pub const INPUT_BINDINGS_METHOD_RESET_PROFILE_JSON_V1: &str = "reset_profile_json_v1";
pub const INPUT_BINDINGS_METHOD_ACTION_CATALOG_JSON_V1: &str = "action_catalog_json_v1";
pub const INPUT_BINDINGS_METHOD_KEY_CATALOG_JSON_V1: &str = "key_catalog_json_v1";
pub const INPUT_BINDINGS_METHOD_REGISTER_KEY_JSON_V1: &str = "register_key_json_v1";
pub const INPUT_BINDINGS_METHOD_REGISTER_ACTION_JSON_V1: &str = "register_action_json_v1";
pub const INPUT_BINDINGS_METHOD_REGISTER_BINDING_JSON_V1: &str = "register_binding_json_v1";
pub const INPUT_BINDINGS_METHOD_REGISTER_LISTENER_JSON_V1: &str = "register_listener_json_v1";
pub const INPUT_BINDINGS_METHOD_REGISTER_MANIFEST_JSON_V1: &str = "register_manifest_json_v1";

pub const INPUT_ACTIONS_METHOD_FRAME_JSON_V1: &str = "frame_json_v1";
pub const INPUT_CONTEXTS_METHOD_STACK_JSON_V1: &str = "stack_json_v1";

pub const INPUT_BINDINGS_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "input.bindings",
        ENGINE_INPUT_BINDINGS_SERVICE_ID,
        INPUT_BINDINGS_SERVICE_ID,
        INPUT_BINDINGS_BACKEND_CAPABILITY_ID,
    );

pub const INPUT_ACTIONS_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "input.actions",
        ENGINE_INPUT_ACTIONS_SERVICE_ID,
        INPUT_ACTIONS_SERVICE_ID,
        INPUT_ACTIONS_BACKEND_CAPABILITY_ID,
    );

pub const INPUT_CONTEXTS_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "input.contexts",
        ENGINE_INPUT_CONTEXTS_SERVICE_ID,
        INPUT_CONTEXTS_SERVICE_ID,
        INPUT_CONTEXTS_BACKEND_CAPABILITY_ID,
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
            INPUT_BINDINGS_METHOD_KEY_CATALOG_JSON_V1,
            INPUT_BINDINGS_METHOD_REGISTER_KEY_JSON_V1,
            INPUT_BINDINGS_METHOD_REGISTER_ACTION_JSON_V1,
            INPUT_BINDINGS_METHOD_REGISTER_BINDING_JSON_V1,
            INPUT_BINDINGS_METHOD_REGISTER_LISTENER_JSON_V1,
            INPUT_BINDINGS_METHOD_REGISTER_MANIFEST_JSON_V1,
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

    pub const ENTER: u32 = 57;
    pub const SPACE: u32 = 58;
    pub const SHIFT_LEFT: u32 = 60;
    pub const SHIFT_RIGHT: u32 = 61;
    pub const TAB: u32 = 63;
    pub const BACKSPACE: u32 = 70;

    pub const ARROW_LEFT: u32 = 86;
    pub const ARROW_UP: u32 = 87;
    pub const ARROW_RIGHT: u32 = 88;
    pub const ARROW_DOWN: u32 = 89;

    pub const ESCAPE: u32 = 114;
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

    pub const UI_MENU_TOGGLE: &str = "engine.menu.toggle_pause";
    pub const UI_MENU_ACCEPT: &str = "ui.menu.accept";
    pub const UI_MENU_BACK: &str = "ui.menu.back";
    pub const UI_MENU_UP: &str = "ui.menu.up";
    pub const UI_MENU_DOWN: &str = "ui.menu.down";
    pub const UI_MENU_LEFT: &str = "ui.menu.left";
    pub const UI_MENU_RIGHT: &str = "ui.menu.right";
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputKeyRegistration {
    /// Stable canonical engine key code. Platform backends must explicitly map native keys to this value.
    pub code: u32,
    /// Stable semantic key id, e.g. `keyboard.escape` or `keyboard.key_w`.
    pub id: String,
    #[serde(default)]
    pub label: String,
}

impl InputKeyRegistration {
    pub fn new(code: u32, id: impl Into<String>, label: impl Into<String>) -> Self {
        Self { code, id: id.into(), label: label.into() }
    }

    pub fn normalized(mut self) -> Option<Self> {
        if self.code == 0 {
            return None;
        }
        self.id = normalize_id_like(&self.id)?;
        self.label = self.label.trim().to_owned();
        if self.label.is_empty() {
            self.label = self.id.clone();
        }
        Some(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputActionListenerRegistration {
    pub owner: String,
    pub id: String,
    #[serde(default)]
    pub action_filter: Vec<String>,
    #[serde(default)]
    pub context_filter: Vec<String>,
    /// Higher priority listeners observe an action before lower priority ones.
    #[serde(default)]
    pub priority: i32,
    /// When true and the action uses `consume_first`, this listener becomes the
    /// action consumer for the current frame. This records ownership without
    /// hard-coded action-id branches in gameplay/menu code.
    #[serde(default)]
    pub consume: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl InputActionListenerRegistration {
    pub fn new(owner: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            owner: owner.into(),
            id: id.into(),
            action_filter: Vec::new(),
            context_filter: Vec::new(),
            priority: 0,
            consume: false,
            enabled: true,
        }
    }

    pub fn with_actions(mut self, actions: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.action_filter = actions.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    pub fn consuming(mut self) -> Self {
        self.consume = true;
        self
    }

    #[inline]
    pub fn normalized(mut self) -> Option<Self> {
        self.owner = normalize_id_like(&self.owner)?;
        self.id = normalize_id_like(&self.id)?;
        self.action_filter = normalize_action_filter(self.action_filter);
        self.context_filter = normalize_string_list(self.context_filter);
        Some(self)
    }
}

#[inline]
fn default_true() -> bool { true }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputActionDispatchMode {
    Broadcast,
    ConsumeFirst,
}

impl Default for InputActionDispatchMode {
    #[inline]
    fn default() -> Self { Self::Broadcast }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InputActionEffect {
    MoveMask { mask: u64 },
    Sprint { enabled: bool },
    CameraView { request: CameraViewRequest },
    MenuToggle,
    MenuAccept,
    MenuBack,
    MenuNav { x: i8, y: i8 },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InputActionDefinition {
    pub id: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub contexts: Vec<String>,
    #[serde(default)]
    pub dispatch: InputActionDispatchMode,
    #[serde(default)]
    pub effects: Vec<InputActionEffect>,
}

impl InputActionDefinition {
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();
        Self {
            label: id.clone(),
            id,
            contexts: Vec::new(),
            dispatch: InputActionDispatchMode::Broadcast,
            effects: Vec::new(),
        }
    }

    pub fn with_effect(mut self, effect: InputActionEffect) -> Self {
        self.effects.push(effect);
        self
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    pub fn with_dispatch(mut self, dispatch: InputActionDispatchMode) -> Self {
        self.dispatch = dispatch;
        self
    }

    #[inline]
    pub fn normalized(mut self) -> Option<Self> {
        self.id = normalize_action_id(&self.id)?;
        if self.label.trim().is_empty() {
            self.label = self.id.clone();
        } else {
            self.label = self.label.trim().to_owned();
        }
        self.contexts = normalize_string_list(self.contexts);
        Some(self)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InputBindingRegistration {
    pub binding: InputBinding,
    #[serde(default)]
    pub replace_existing_for_action_device: bool,
}

impl InputBindingRegistration {
    #[inline]
    pub fn new(binding: InputBinding) -> Self {
        Self { binding, replace_existing_for_action_device: false }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct InputBindingsManifest {
    #[serde(default)]
    pub keys: Vec<InputKeyRegistration>,
    #[serde(default)]
    pub actions: Vec<InputActionDefinition>,
    #[serde(default)]
    pub bindings: Vec<InputBindingRegistration>,
    #[serde(default)]
    pub listeners: Vec<InputActionListenerRegistration>,
    #[serde(default)]
    pub gamepad_axes: Vec<GamepadAxisBinding>,
}

impl InputBindingsManifest {
    pub fn apply_to(self, profile: &mut InputBindingsProfile) -> Result<(), String> {
        for key in self.keys {
            profile.register_key(key)?;
        }
        for action in self.actions {
            profile.register_action(action)?;
        }
        for binding in self.bindings {
            profile.register_binding(binding)?;
        }
        for listener in self.listeners {
            profile.register_listener(listener)?;
        }
        for axis in self.gamepad_axes {
            profile.register_gamepad_axis(axis)?;
        }
        Ok(())
    }
}

#[inline]
fn normalize_action_id(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.contains(char::is_whitespace) || trimmed.contains('/') || trimmed.contains('\\') {
        None
    } else {
        Some(trimmed.to_ascii_lowercase())
    }
}

#[inline]
fn normalize_id_like(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.contains(char::is_whitespace) || trimmed.contains('/') || trimmed.contains('\\') {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

fn normalize_string_list(values: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() || out.iter().any(|v: &String| v == trimmed) {
            continue;
        }
        out.push(trimmed.to_owned());
    }
    out
}

fn normalize_action_filter(values: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    for value in values {
        let Some(action) = normalize_action_id(&value) else { continue; };
        if out.iter().any(|v: &String| v == &action) {
            continue;
        }
        out.push(action);
    }
    out
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
    pub fn normalized(mut self) -> Option<Self> {
        self.action = normalize_action_id(&self.action)?;
        if let Some(name) = self.name.take() {
            let trimmed = name.trim();
            if !trimmed.is_empty() {
                self.name = Some(trimmed.to_owned());
            }
        }
        Some(self)
    }

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
    pub keys: Vec<InputKeyRegistration>,
    #[serde(default)]
    pub actions: Vec<InputActionDefinition>,
    #[serde(default)]
    pub listeners: Vec<InputActionListenerRegistration>,
    #[serde(default)]
    pub bindings: Vec<InputBinding>,
    #[serde(default)]
    pub gamepad_axes: Vec<GamepadAxisBinding>,
}

impl InputBindingsProfile {
    #[inline]
    pub fn gameplay_default() -> Self {
        Self {
            id: "newengine.default.gameplay".to_owned(),
            version: 4,
            device_preference: InputDevicePreference::Hybrid,
            keys: gameplay_default_key_registry(),
            actions: gameplay_default_actions(),
            listeners: gameplay_default_listeners(),
            bindings: gameplay_default_bindings(),
            gamepad_axes: gameplay_default_gamepad_axes(),
        }
    }

    pub fn canonicalized(mut self) -> Self {
        self.id = normalize_id_like(&self.id).unwrap_or_else(|| "newengine.input.profile".to_owned());
        self.version = self.version.max(4);

        let mut keys = gameplay_default_key_registry();
        for key in self.keys.into_iter().filter_map(InputKeyRegistration::normalized) {
            upsert_key_registration(&mut keys, key);
        }
        keys.sort_by(|a, b| a.code.cmp(&b.code).then_with(|| a.id.cmp(&b.id)));
        self.keys = keys;

        let mut actions = gameplay_default_actions();
        for action in self.actions.into_iter().filter_map(InputActionDefinition::normalized) {
            upsert_action_definition(&mut actions, action);
        }
        self.actions = actions;

        self.bindings = self
            .bindings
            .into_iter()
            .filter_map(InputBinding::normalized)
            .collect();
        if self.bindings.is_empty() {
            self.bindings = gameplay_default_bindings();
        }
        ensure_required_system_bindings(&mut self.bindings);

        let mut listeners = gameplay_default_listeners();
        for listener in self.listeners.into_iter().filter_map(InputActionListenerRegistration::normalized) {
            upsert_listener_registration(&mut listeners, listener);
        }
        listeners.sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.id.cmp(&b.id)));
        self.listeners = listeners;

        self.gamepad_axes = self
            .gamepad_axes
            .into_iter()
            .map(|mut axis| {
                axis.axis = axis.axis.trim().to_owned();
                axis.deadzone = axis.deadzone.clamp(0.0, 0.95);
                axis.scale = axis.scale.clamp(-8.0, 8.0);
                axis
            })
            .filter(|axis| !axis.axis.is_empty())
            .collect();
        if self.gamepad_axes.is_empty() {
            self.gamepad_axes = gameplay_default_gamepad_axes();
        }

        self
    }

    pub fn register_key(&mut self, key: InputKeyRegistration) -> Result<(), String> {
        let key = key.normalized().ok_or_else(|| "invalid input key registration".to_owned())?;
        upsert_key_registration(&mut self.keys, key);
        self.keys.sort_by(|a, b| a.code.cmp(&b.code).then_with(|| a.id.cmp(&b.id)));
        Ok(())
    }

    pub fn register_action(&mut self, action: InputActionDefinition) -> Result<(), String> {
        let action = action
            .normalized()
            .ok_or_else(|| "invalid input action registration".to_owned())?;
        upsert_action_definition(&mut self.actions, action);
        Ok(())
    }

    pub fn register_binding(&mut self, registration: InputBindingRegistration) -> Result<(), String> {
        let binding = registration
            .binding
            .normalized()
            .ok_or_else(|| "invalid input binding registration".to_owned())?;
        if !self.actions.iter().any(|action| action.id == binding.action) {
            return Err(format!("input binding references undeclared action '{}'", binding.action));
        }
        if registration.replace_existing_for_action_device {
            self.bindings
                .retain(|existing| !(existing.action == binding.action && existing.device == binding.device));
        }
        self.bindings.push(binding);
        Ok(())
    }

    pub fn register_listener(&mut self, listener: InputActionListenerRegistration) -> Result<(), String> {
        let listener = listener
            .normalized()
            .ok_or_else(|| "invalid input listener registration".to_owned())?;
        upsert_listener_registration(&mut self.listeners, listener);
        Ok(())
    }

    pub fn register_gamepad_axis(&mut self, mut axis: GamepadAxisBinding) -> Result<(), String> {
        axis.axis = axis.axis.trim().to_owned();
        if axis.axis.is_empty() {
            return Err("invalid empty gamepad axis binding".to_owned());
        }
        axis.deadzone = axis.deadzone.clamp(0.0, 0.95);
        axis.scale = axis.scale.clamp(-8.0, 8.0);
        self.gamepad_axes.retain(|existing| !(existing.axis == axis.axis && existing.target == axis.target));
        self.gamepad_axes.push(axis);
        Ok(())
    }

    pub fn resolve<T: InputFrameSource>(&self, input: &T) -> InputActionFrame {
        let mut out = InputActionFrame::default();
        let actions = self.action_catalog();
        let mut seen = std::collections::BTreeSet::<String>::new();
        for binding in &self.bindings {
            if !device_allowed(self.device_preference, binding.device) || !binding_matches(binding, input) {
                continue;
            }
            if !seen.insert(binding.action.clone()) {
                continue;
            }
            if let Some(definition) = actions.get(binding.action.as_str()) {
                dispatch_action_definition(&mut out, definition, &self.listeners);
            } else {
                out.actions.push(binding.action.clone());
            }
        }
        if self.device_preference.allows_gamepad() {
            apply_gamepad_axes(&mut out, &self.gamepad_axes, input);
        }
        out
    }

    pub fn action_catalog(&self) -> std::collections::BTreeMap<&str, &InputActionDefinition> {
        self.actions
            .iter()
            .map(|definition| (definition.id.as_str(), definition))
            .collect()
    }
}


fn gameplay_default_key_registry() -> Vec<InputKeyRegistration> {
    use crate::key_code as keys;
    vec![
        InputKeyRegistration::new(keys::DIGIT1, "keyboard.digit1", "1"),
        InputKeyRegistration::new(keys::DIGIT2, "keyboard.digit2", "2"),
        InputKeyRegistration::new(keys::DIGIT3, "keyboard.digit3", "3"),
        InputKeyRegistration::new(keys::KEY_A, "keyboard.key_a", "A"),
        InputKeyRegistration::new(keys::KEY_D, "keyboard.key_d", "D"),
        InputKeyRegistration::new(keys::KEY_E, "keyboard.key_e", "E"),
        InputKeyRegistration::new(keys::KEY_F, "keyboard.key_f", "F"),
        InputKeyRegistration::new(keys::KEY_Q, "keyboard.key_q", "Q"),
        InputKeyRegistration::new(keys::KEY_S, "keyboard.key_s", "S"),
        InputKeyRegistration::new(keys::KEY_W, "keyboard.key_w", "W"),
        InputKeyRegistration::new(keys::ENTER, "keyboard.enter", "ENTER"),
        InputKeyRegistration::new(keys::SPACE, "keyboard.space", "SPACE"),
        InputKeyRegistration::new(keys::SHIFT_LEFT, "keyboard.shift_left", "LEFT SHIFT"),
        InputKeyRegistration::new(keys::SHIFT_RIGHT, "keyboard.shift_right", "RIGHT SHIFT"),
        InputKeyRegistration::new(keys::TAB, "keyboard.tab", "TAB"),
        InputKeyRegistration::new(keys::BACKSPACE, "keyboard.backspace", "BACKSPACE"),
        InputKeyRegistration::new(keys::ARROW_LEFT, "keyboard.arrow_left", "LEFT"),
        InputKeyRegistration::new(keys::ARROW_UP, "keyboard.arrow_up", "UP"),
        InputKeyRegistration::new(keys::ARROW_RIGHT, "keyboard.arrow_right", "RIGHT"),
        InputKeyRegistration::new(keys::ARROW_DOWN, "keyboard.arrow_down", "DOWN"),
        InputKeyRegistration::new(keys::ESCAPE, "keyboard.escape", "ESC"),
    ]
}

fn gameplay_default_actions() -> Vec<InputActionDefinition> {
    vec![
        InputActionDefinition::new(action::PLAYER_MOVE_FORWARD).with_label("Move forward").with_effect(InputActionEffect::MoveMask { mask: move_mask::FORWARD }),
        InputActionDefinition::new(action::PLAYER_MOVE_BACK).with_label("Move back").with_effect(InputActionEffect::MoveMask { mask: move_mask::BACK }),
        InputActionDefinition::new(action::PLAYER_MOVE_LEFT).with_label("Move left").with_effect(InputActionEffect::MoveMask { mask: move_mask::LEFT }),
        InputActionDefinition::new(action::PLAYER_MOVE_RIGHT).with_label("Move right").with_effect(InputActionEffect::MoveMask { mask: move_mask::RIGHT }),
        InputActionDefinition::new(action::PLAYER_MOVE_UP).with_label("Move up").with_effect(InputActionEffect::MoveMask { mask: move_mask::UP }),
        InputActionDefinition::new(action::PLAYER_MOVE_DOWN).with_label("Move down").with_effect(InputActionEffect::MoveMask { mask: move_mask::DOWN }),
        InputActionDefinition::new(action::PLAYER_SPRINT)
            .with_label("Sprint")
            .with_effect(InputActionEffect::MoveMask { mask: move_mask::SPRINT })
            .with_effect(InputActionEffect::Sprint { enabled: true }),
        InputActionDefinition::new(action::CAMERA_VIEW_NEXT).with_label("Next camera view").with_effect(InputActionEffect::CameraView { request: CameraViewRequest::Next }),
        InputActionDefinition::new(action::CAMERA_VIEW_PREVIOUS).with_label("Previous camera view").with_effect(InputActionEffect::CameraView { request: CameraViewRequest::Previous }),
        InputActionDefinition::new(action::CAMERA_VIEW_FIRST_PERSON).with_label("First-person camera").with_effect(InputActionEffect::CameraView { request: CameraViewRequest::Set(newengine_camera_api::CameraViewMode::FirstPerson) }),
        InputActionDefinition::new(action::CAMERA_VIEW_THIRD_PERSON_FOLLOW).with_label("Third-person follow camera").with_effect(InputActionEffect::CameraView { request: CameraViewRequest::Set(newengine_camera_api::CameraViewMode::ThirdPersonFollow) }),
        InputActionDefinition::new(action::CAMERA_VIEW_THIRD_PERSON_AIM).with_label("Third-person aim camera").with_effect(InputActionEffect::CameraView { request: CameraViewRequest::Set(newengine_camera_api::CameraViewMode::ThirdPersonAim) }),
        InputActionDefinition::new(action::UI_MENU_TOGGLE).with_dispatch(InputActionDispatchMode::ConsumeFirst).with_label("Toggle menu").with_effect(InputActionEffect::MenuToggle),
        InputActionDefinition::new(action::UI_MENU_ACCEPT).with_dispatch(InputActionDispatchMode::ConsumeFirst).with_label("Accept").with_effect(InputActionEffect::MenuAccept),
        InputActionDefinition::new(action::UI_MENU_BACK).with_dispatch(InputActionDispatchMode::ConsumeFirst).with_label("Back").with_effect(InputActionEffect::MenuBack),
        InputActionDefinition::new(action::UI_MENU_UP).with_dispatch(InputActionDispatchMode::ConsumeFirst).with_label("Menu up").with_effect(InputActionEffect::MenuNav { x: 0, y: -1 }),
        InputActionDefinition::new(action::UI_MENU_DOWN).with_dispatch(InputActionDispatchMode::ConsumeFirst).with_label("Menu down").with_effect(InputActionEffect::MenuNav { x: 0, y: 1 }),
        InputActionDefinition::new(action::UI_MENU_LEFT).with_dispatch(InputActionDispatchMode::ConsumeFirst).with_label("Menu left").with_effect(InputActionEffect::MenuNav { x: -1, y: 0 }),
        InputActionDefinition::new(action::UI_MENU_RIGHT).with_dispatch(InputActionDispatchMode::ConsumeFirst).with_label("Menu right").with_effect(InputActionEffect::MenuNav { x: 1, y: 0 }),
    ]
}

fn upsert_key_registration(keys: &mut Vec<InputKeyRegistration>, key: InputKeyRegistration) {
    keys.retain(|existing| existing.code != key.code && existing.id != key.id);
    keys.push(key);
}

fn upsert_action_definition(actions: &mut Vec<InputActionDefinition>, action: InputActionDefinition) {
    if let Some(slot) = actions.iter_mut().find(|existing| existing.id == action.id) {
        *slot = action;
    } else {
        actions.push(action);
    }
}

fn upsert_listener_registration(listeners: &mut Vec<InputActionListenerRegistration>, listener: InputActionListenerRegistration) {
    listeners.retain(|existing| !(existing.owner == listener.owner && existing.id == listener.id));
    listeners.push(listener);
}

fn gameplay_default_listeners() -> Vec<InputActionListenerRegistration> {
    vec![
        InputActionListenerRegistration::new("newengine-ui", "pause-menu")
            .with_actions([
                action::UI_MENU_TOGGLE,
                action::UI_MENU_ACCEPT,
                action::UI_MENU_BACK,
                action::UI_MENU_UP,
                action::UI_MENU_DOWN,
                action::UI_MENU_LEFT,
                action::UI_MENU_RIGHT,
            ])
            .with_priority(100)
            .consuming(),
        InputActionListenerRegistration::new("newengine-camera-runtime", "camera-view")
            .with_actions([
                action::CAMERA_VIEW_NEXT,
                action::CAMERA_VIEW_PREVIOUS,
                action::CAMERA_VIEW_FIRST_PERSON,
                action::CAMERA_VIEW_THIRD_PERSON_FOLLOW,
                action::CAMERA_VIEW_THIRD_PERSON_AIM,
            ])
            .with_priority(50),
        InputActionListenerRegistration::new("newengine-gameplay", "player-controller")
            .with_actions([
                action::PLAYER_MOVE_FORWARD,
                action::PLAYER_MOVE_BACK,
                action::PLAYER_MOVE_LEFT,
                action::PLAYER_MOVE_RIGHT,
                action::PLAYER_MOVE_UP,
                action::PLAYER_MOVE_DOWN,
                action::PLAYER_SPRINT,
            ])
            .with_priority(10),
    ]
}

fn ensure_required_system_bindings(bindings: &mut Vec<InputBinding>) {
    let has_keyboard_toggle = bindings.iter().any(|binding| {
        binding.action == action::UI_MENU_TOGGLE
            && binding.device == InputBindingDevice::Keyboard
            && binding.phase == InputBindingPhase::Pressed
    });
    if !has_keyboard_toggle {
        bindings.push(InputBinding::keyboard_pressed(action::UI_MENU_TOGGLE, key_code::ESCAPE));
    }

    let has_gamepad_toggle = bindings.iter().any(|binding| {
        binding.action == action::UI_MENU_TOGGLE
            && binding.device == InputBindingDevice::GamepadButton
            && binding.phase == InputBindingPhase::Pressed
    });
    if !has_gamepad_toggle {
        bindings.push(InputBinding::gamepad_button_pressed(action::UI_MENU_TOGGLE, gamepad_button::START));
    }

    bindings.retain(|binding| {
        !(binding.action == action::UI_MENU_BACK
            && binding.device == InputBindingDevice::Keyboard
            && binding.code == key_code::ESCAPE)
    });
}

fn gameplay_default_bindings() -> Vec<InputBinding> {
    let mut bindings = Vec::with_capacity(32);
    bindings.extend(gameplay_keyboard_bindings());
    bindings.extend(gameplay_gamepad_button_bindings());
    bindings
}

fn gameplay_keyboard_bindings() -> [InputBinding; 20] {
    use crate::key_code as keys;
    [
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
        InputBinding::keyboard_pressed(action::UI_MENU_TOGGLE, keys::ESCAPE),
        InputBinding::keyboard_pressed(action::UI_MENU_ACCEPT, keys::ENTER),
        InputBinding::keyboard_pressed(action::UI_MENU_ACCEPT, keys::SPACE),
        InputBinding::keyboard_pressed(action::UI_MENU_BACK, keys::BACKSPACE),
        InputBinding::keyboard_pressed(action::UI_MENU_UP, keys::ARROW_UP),
        InputBinding::keyboard_pressed(action::UI_MENU_DOWN, keys::ARROW_DOWN),
        InputBinding::keyboard_pressed(action::UI_MENU_LEFT, keys::ARROW_LEFT),
        InputBinding::keyboard_pressed(action::UI_MENU_RIGHT, keys::ARROW_RIGHT),
    ]
}

fn gameplay_gamepad_button_bindings() -> [InputBinding; 13] {
    [
        InputBinding::gamepad_button_down(action::PLAYER_SPRINT, gamepad_button::LEFT_THUMB),
        InputBinding::gamepad_button_pressed(action::CAMERA_VIEW_NEXT, gamepad_button::SELECT),
        InputBinding::gamepad_button_pressed(action::CAMERA_VIEW_NEXT, gamepad_button::MODE),
        InputBinding::gamepad_button_pressed(action::CAMERA_VIEW_FIRST_PERSON, gamepad_button::DPAD_UP),
        InputBinding::gamepad_button_pressed(action::CAMERA_VIEW_THIRD_PERSON_FOLLOW, gamepad_button::DPAD_LEFT),
        InputBinding::gamepad_button_pressed(action::CAMERA_VIEW_THIRD_PERSON_AIM, gamepad_button::DPAD_RIGHT),
        InputBinding::gamepad_button_pressed(action::UI_MENU_TOGGLE, gamepad_button::START),
        InputBinding::gamepad_button_pressed(action::UI_MENU_ACCEPT, gamepad_button::SOUTH),
        InputBinding::gamepad_button_pressed(action::UI_MENU_BACK, gamepad_button::EAST),
        InputBinding::gamepad_button_pressed(action::UI_MENU_UP, gamepad_button::DPAD_UP),
        InputBinding::gamepad_button_pressed(action::UI_MENU_DOWN, gamepad_button::DPAD_DOWN),
        InputBinding::gamepad_button_pressed(action::UI_MENU_LEFT, gamepad_button::DPAD_LEFT),
        InputBinding::gamepad_button_pressed(action::UI_MENU_RIGHT, gamepad_button::DPAD_RIGHT),
    ]
}

fn gameplay_default_gamepad_axes() -> Vec<GamepadAxisBinding> {
    vec![
        GamepadAxisBinding::new(gamepad_axis::LEFT_STICK_X, GamepadAxisTarget::MoveX, 1.0),
        GamepadAxisBinding::new(gamepad_axis::LEFT_STICK_Y, GamepadAxisTarget::MoveZ, -1.0),
        GamepadAxisBinding::new(gamepad_axis::RIGHT_STICK_X, GamepadAxisTarget::LookX, 1.0),
        GamepadAxisBinding::new(gamepad_axis::RIGHT_STICK_Y, GamepadAxisTarget::LookY, -1.0),
    ]
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
    pub menu_toggle: bool,
    #[serde(default)]
    pub menu_accept: bool,
    #[serde(default)]
    pub menu_back: bool,
    #[serde(default)]
    pub menu_nav: [i8; 2],
    #[serde(default)]
    pub actions: Vec<String>,
    #[serde(default)]
    pub events: Vec<InputActionDispatchEvent>,
}

impl Default for InputActionFrame {
    #[inline]
    fn default() -> Self {
        Self { move_mask: 0, move_axis: [0.0, 0.0, 0.0], sprint: false, look_axis: [0.0, 0.0], camera_view: CameraViewRequest::None, menu_toggle: false, menu_accept: false, menu_back: false, menu_nav: [0, 0], actions: Vec::new(), events: Vec::new() }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputActionDispatchEvent {
    pub action: String,
    #[serde(default)]
    pub listeners: Vec<String>,
    #[serde(default)]
    pub consumed_by: Option<String>,
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
                "central-key-registry".to_owned(),
                "central-action-registry".to_owned(),
                "semantic-actions".to_owned(),
                "action-listeners".to_owned(),
                "listener-priority-consumption".to_owned(),
                "manifest-registration".to_owned(),
                "camera-view-switching".to_owned(),
                "gamepad-bindings".to_owned(),
                "device-preference".to_owned(),
                "gameplay-move-mask".to_owned(),
            ],
            methods: vec![
                INPUT_BINDINGS_METHOD_INFO.to_owned(),
                INPUT_BINDINGS_METHOD_INVOKE.to_owned(),
                INPUT_BINDINGS_METHOD_SHUTDOWN_V1.to_owned(),
                INPUT_BINDINGS_METHOD_PROFILE_JSON_V1.to_owned(),
                INPUT_BINDINGS_METHOD_SAVE_PROFILE_JSON_V1.to_owned(),
                INPUT_BINDINGS_METHOD_RESET_PROFILE_JSON_V1.to_owned(),
                INPUT_BINDINGS_METHOD_ACTION_CATALOG_JSON_V1.to_owned(),
                INPUT_BINDINGS_METHOD_KEY_CATALOG_JSON_V1.to_owned(),
                INPUT_BINDINGS_METHOD_REGISTER_KEY_JSON_V1.to_owned(),
                INPUT_BINDINGS_METHOD_REGISTER_ACTION_JSON_V1.to_owned(),
                INPUT_BINDINGS_METHOD_REGISTER_BINDING_JSON_V1.to_owned(),
                INPUT_BINDINGS_METHOD_REGISTER_LISTENER_JSON_V1.to_owned(),
                INPUT_BINDINGS_METHOD_REGISTER_MANIFEST_JSON_V1.to_owned(),
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

fn dispatch_action_definition(
    out: &mut InputActionFrame,
    definition: &InputActionDefinition,
    listeners: &[InputActionListenerRegistration],
) {
    let event = dispatch_event_for_action(definition, listeners);
    out.actions.push(definition.id.clone());
    out.events.push(event);
    for effect in &definition.effects {
        apply_action_effect(out, effect);
    }
    out.move_axis = move_axis_from_mask(out.move_mask);
}

fn dispatch_event_for_action(
    definition: &InputActionDefinition,
    listeners: &[InputActionListenerRegistration],
) -> InputActionDispatchEvent {
    let mut event = InputActionDispatchEvent {
        action: definition.id.clone(),
        listeners: Vec::new(),
        consumed_by: None,
    };
    for listener in listeners.iter().filter(|listener| listener.enabled && listener_matches_action(listener, definition)) {
        let listener_id = format!("{}:{}", listener.owner, listener.id);
        event.listeners.push(listener_id.clone());
        if definition.dispatch == InputActionDispatchMode::ConsumeFirst && listener.consume {
            event.consumed_by = Some(listener_id);
            break;
        }
    }
    event
}

fn listener_matches_action(listener: &InputActionListenerRegistration, definition: &InputActionDefinition) -> bool {
    let action_match = listener.action_filter.is_empty() || listener.action_filter.iter().any(|action| action == &definition.id);
    if !action_match {
        return false;
    }
    listener.context_filter.is_empty()
        || definition.contexts.iter().any(|ctx| listener.context_filter.iter().any(|wanted| wanted == ctx))
}

fn apply_action_effect(out: &mut InputActionFrame, effect: &InputActionEffect) {
    match effect {
        InputActionEffect::MoveMask { mask } => out.move_mask |= *mask,
        InputActionEffect::Sprint { enabled } => out.sprint |= *enabled,
        InputActionEffect::CameraView { request } => out.camera_view = *request,
        InputActionEffect::MenuToggle => out.menu_toggle = true,
        InputActionEffect::MenuAccept => out.menu_accept = true,
        InputActionEffect::MenuBack => out.menu_back = true,
        InputActionEffect::MenuNav { x, y } => {
            out.menu_nav[0] = out.menu_nav[0].saturating_add(*x).clamp(-1, 1);
            out.menu_nav[1] = out.menu_nav[1].saturating_add(*y).clamp(-1, 1);
        }
    }
}

#[inline]
pub fn move_axis_from_mask(mask: u64) -> [f32; 3] {
    let x = ((mask & move_mask::RIGHT != 0) as i32 - (mask & move_mask::LEFT != 0) as i32) as f32;
    let y = ((mask & move_mask::UP != 0) as i32 - (mask & move_mask::DOWN != 0) as i32) as f32;
    let z = ((mask & move_mask::FORWARD != 0) as i32 - (mask & move_mask::BACK != 0) as i32) as f32;
    [x, y, z]
}

impl InputBindingsProfile {
    #[inline]
    pub fn primary_binding_label(&self, action: &str) -> String {
        let action = normalize_action_id(action).unwrap_or_else(|| action.trim().to_owned());
        let preferred = match self.device_preference {
            InputDevicePreference::Gamepad => [
                InputBindingDevice::GamepadButton,
                InputBindingDevice::Keyboard,
                InputBindingDevice::MouseButton,
            ],
            InputDevicePreference::KeyboardMouse | InputDevicePreference::Hybrid => [
                InputBindingDevice::Keyboard,
                InputBindingDevice::MouseButton,
                InputBindingDevice::GamepadButton,
            ],
        };
        for device in preferred {
            if let Some(binding) = self
                .bindings
                .iter()
                .find(|binding| binding.action == action && binding.device == device)
            {
                return self.binding_display_label(binding);
            }
        }
        self.bindings
            .iter()
            .find(|binding| binding.action == action)
            .map(|binding| self.binding_display_label(binding))
            .unwrap_or_else(|| "UNBOUND".to_owned())
    }

    #[inline]
    pub fn key_label(&self, code: u32) -> String {
        self.keys
            .iter()
            .find(|key| key.code == code)
            .map(|key| key.label.clone())
            .unwrap_or_else(|| key_code_label(code).to_owned())
    }

    #[inline]
    pub fn binding_display_label(&self, binding: &InputBinding) -> String {
        match binding.device {
            InputBindingDevice::Keyboard => self.key_label(binding.code),
            InputBindingDevice::MouseButton => mouse_button_label(binding.code).to_owned(),
            InputBindingDevice::GamepadButton => binding
                .name
                .as_deref()
                .map(gamepad_button_label)
                .unwrap_or("GAMEPAD")
                .to_owned(),
        }
    }
}


#[inline]
pub fn binding_display_label(binding: &InputBinding) -> String {
    match binding.device {
        InputBindingDevice::Keyboard => key_code_label(binding.code),
        InputBindingDevice::MouseButton => mouse_button_label(binding.code),
        InputBindingDevice::GamepadButton => binding
            .name
            .as_deref()
            .map(gamepad_button_label)
            .unwrap_or("GAMEPAD"),
    }
    .to_owned()
}

#[inline]
pub fn key_code_label(code: u32) -> &'static str {
    match code {
        key_code::DIGIT1 => "1",
        key_code::DIGIT2 => "2",
        key_code::DIGIT3 => "3",
        key_code::KEY_A => "A",
        key_code::KEY_D => "D",
        key_code::KEY_E => "E",
        key_code::KEY_F => "F",
        key_code::KEY_Q => "Q",
        key_code::KEY_S => "S",
        key_code::KEY_W => "W",
        key_code::ENTER => "ENTER",
        key_code::SPACE => "SPACE",
        key_code::SHIFT_LEFT => "LEFT SHIFT",
        key_code::SHIFT_RIGHT => "RIGHT SHIFT",
        key_code::TAB => "TAB",
        key_code::BACKSPACE => "BACKSPACE",
        key_code::ARROW_LEFT => "LEFT",
        key_code::ARROW_UP => "UP",
        key_code::ARROW_RIGHT => "RIGHT",
        key_code::ARROW_DOWN => "DOWN",
        key_code::ESCAPE => "ESC",
        _ => "KEY",
    }
}

#[inline]
pub fn mouse_button_label(code: u32) -> &'static str {
    match code {
        1 => "MOUSE LEFT",
        2 => "MOUSE RIGHT",
        3 => "MOUSE MIDDLE",
        4 => "MOUSE BACK",
        5 => "MOUSE FORWARD",
        _ => "MOUSE",
    }
}

#[inline]
pub fn gamepad_button_label(name: &str) -> &'static str {
    match name {
        gamepad_button::SOUTH => "PAD SOUTH",
        gamepad_button::EAST => "PAD EAST",
        gamepad_button::WEST => "PAD WEST",
        gamepad_button::NORTH => "PAD NORTH",
        gamepad_button::LEFT_THUMB => "PAD L3",
        gamepad_button::RIGHT_THUMB => "PAD R3",
        gamepad_button::START => "PAD START",
        gamepad_button::SELECT => "PAD SELECT",
        gamepad_button::MODE => "PAD MODE",
        gamepad_button::DPAD_UP => "DPAD UP",
        gamepad_button::DPAD_DOWN => "DPAD DOWN",
        gamepad_button::DPAD_LEFT => "DPAD LEFT",
        gamepad_button::DPAD_RIGHT => "DPAD RIGHT",
        _ => "GAMEPAD",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_profile_has_camera_view_switching() {
        let profile = InputBindingsProfile::gameplay_default();
        assert!(profile.keys.iter().any(|k| k.id == "keyboard.escape" && k.code == key_code::ESCAPE));
        assert!(profile.bindings.iter().any(|b| b.action == action::CAMERA_VIEW_NEXT));
        assert!(profile.bindings.iter().any(|b| b.action == action::PLAYER_MOVE_FORWARD));
        assert!(profile.actions.iter().any(|a| a.id == action::CAMERA_VIEW_NEXT));
        assert!(profile.listeners.iter().any(|l| l.id == "pause-menu"));
        assert!(profile.bindings.iter().any(|b| b.action == action::UI_MENU_TOGGLE && b.code == key_code::ESCAPE));
        assert!(!profile.bindings.iter().any(|b| b.action == action::UI_MENU_BACK && b.code == key_code::ESCAPE));
    }

    #[test]
    fn custom_action_can_register_keyboard_and_gamepad_bindings() {
        let mut profile = InputBindingsProfile::gameplay_default();
        profile
            .register_action(
                InputActionDefinition::new("player.custom_dash")
                    .with_effect(InputActionEffect::MoveMask { mask: move_mask::SPRINT }),
            )
            .unwrap();
        profile
            .register_binding(InputBindingRegistration {
                binding: InputBinding::keyboard_pressed("player.custom_dash", key_code::TAB),
                replace_existing_for_action_device: false,
            })
            .unwrap();
        profile
            .register_binding(InputBindingRegistration {
                binding: InputBinding::gamepad_button_pressed("player.custom_dash", gamepad_button::WEST),
                replace_existing_for_action_device: false,
            })
            .unwrap();

        assert!(profile.actions.iter().any(|a| a.id == "player.custom_dash"));
        assert!(profile.bindings.iter().any(|b| b.action == "player.custom_dash"));
    }
}
