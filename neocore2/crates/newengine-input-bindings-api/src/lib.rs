#![forbid(unsafe_op_in_unsafe_fn)]
use serde::{Deserialize, Serialize};

pub use newengine_input_actions_api::{
    InputActionDefinition, InputActionDispatchMode, InputActionEffect, InputActionFrame,
    InputActionListenerRegistration, InputFrameSource,
};
pub use newengine_input_api::{gamepad_axis, gamepad_button, key_code};

pub const ENGINE_INPUT_BINDINGS_SERVICE_ID: &str = "engine.input.bindings";
pub const INPUT_BINDINGS_SERVICE_ID: &str = "input.bindings.api";
pub const INPUT_BINDINGS_BACKEND_CAPABILITY_ID: &str = "input.bindings.backend";

pub const INPUT_BINDINGS_METHOD_INFO: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
pub const INPUT_BINDINGS_METHOD_INVOKE: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
pub const INPUT_BINDINGS_METHOD_SHUTDOWN_V1: &str =
    newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
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
        "newengine.input-bindings-api >= 0.1.x",
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

pub const INPUT_BINDINGS_RUNTIME_REQUIREMENT_SPEC:
    newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        INPUT_BINDINGS_RUNTIME_CONTRACT_SPEC,
        Some(INPUT_BINDINGS_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_INPUT_BINDINGS"),
    );

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
        Self {
            code,
            id: id.into(),
            label: label.into(),
        }
    }

    pub fn normalized(mut self) -> Option<Self> {
        if self.code == 0 {
            return None;
        }
        self.id = newengine_input_actions_api::normalize_id_like(&self.id)?;
        self.label = self.label.trim().to_owned();
        if self.label.is_empty() {
            self.label = self.id.clone();
        }
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
        Self {
            binding,
            replace_existing_for_action_device: false,
        }
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputDevicePreference {
    KeyboardMouse,
    Gamepad,
    Hybrid,
}

impl Default for InputDevicePreference {
    #[inline]
    fn default() -> Self {
        Self::Hybrid
    }
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
    fn default() -> Self {
        Self::Down
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputBindingDevice {
    Keyboard,
    MouseButton,
    GamepadButton,
}

impl Default for InputBindingDevice {
    #[inline]
    fn default() -> Self {
        Self::Keyboard
    }
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
        self.action = newengine_input_actions_api::normalize_action_id(&self.action)?;
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
        Self {
            action: action.into(),
            device: InputBindingDevice::Keyboard,
            code,
            name: None,
            phase: InputBindingPhase::Down,
        }
    }

    #[inline]
    pub fn keyboard_pressed(action: impl Into<String>, code: u32) -> Self {
        Self {
            action: action.into(),
            device: InputBindingDevice::Keyboard,
            code,
            name: None,
            phase: InputBindingPhase::Pressed,
        }
    }

    #[inline]
    pub fn gamepad_button_down(action: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            action: action.into(),
            device: InputBindingDevice::GamepadButton,
            code: 0,
            name: Some(name.into()),
            phase: InputBindingPhase::Down,
        }
    }

    #[inline]
    pub fn gamepad_button_pressed(action: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            action: action.into(),
            device: InputBindingDevice::GamepadButton,
            code: 0,
            name: Some(name.into()),
            phase: InputBindingPhase::Pressed,
        }
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
fn default_axis_deadzone() -> f32 {
    0.18
}
#[inline]
fn default_axis_scale() -> f32 {
    1.0
}

impl GamepadAxisBinding {
    #[inline]
    pub fn new(axis: impl Into<String>, target: GamepadAxisTarget, scale: f32) -> Self {
        Self {
            axis: axis.into(),
            target,
            deadzone: default_axis_deadzone(),
            scale,
        }
    }
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
    pub fn empty(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            version: 4,
            device_preference: InputDevicePreference::Hybrid,
            keys: Vec::new(),
            actions: Vec::new(),
            listeners: Vec::new(),
            bindings: Vec::new(),
            gamepad_axes: Vec::new(),
        }
    }

    /// Canonicalizes only this profile. It does not inject product/gameplay defaults.
    #[inline]
    pub fn canonicalized(self) -> Self {
        self.canonicalized_with_defaults(&InputBindingsProfile::empty(
            "newengine.input.defaults.empty",
        ))
    }

    /// Canonicalizes this profile over a profile-owned default layer.
    /// Generic bindings API owns merging rules; a product/profile crate owns the default data.
    pub fn canonicalized_with_defaults(mut self, defaults: &InputBindingsProfile) -> Self {
        self.id = newengine_input_actions_api::normalize_id_like(&self.id)
            .unwrap_or_else(|| defaults.id.clone());
        self.version = self.version.max(defaults.version).max(4);

        let mut keys: Vec<_> = defaults
            .keys
            .clone()
            .into_iter()
            .filter_map(InputKeyRegistration::normalized)
            .collect();
        for key in self
            .keys
            .into_iter()
            .filter_map(InputKeyRegistration::normalized)
        {
            upsert_key_registration(&mut keys, key);
        }
        keys.sort_by(|a, b| a.code.cmp(&b.code).then_with(|| a.id.cmp(&b.id)));
        self.keys = keys;

        let mut actions: Vec<_> = defaults
            .actions
            .clone()
            .into_iter()
            .filter_map(InputActionDefinition::normalized)
            .collect();
        for action in self
            .actions
            .into_iter()
            .filter_map(InputActionDefinition::normalized)
        {
            upsert_action_definition(&mut actions, action);
        }
        self.actions = actions;

        let mut bindings = Vec::new();
        for binding in self
            .bindings
            .into_iter()
            .filter_map(InputBinding::normalized)
        {
            if !bindings
                .iter()
                .any(|existing| bindings_equivalent(existing, &binding))
            {
                bindings.push(binding);
            }
        }
        for binding in defaults
            .bindings
            .clone()
            .into_iter()
            .filter_map(InputBinding::normalized)
        {
            if !bindings
                .iter()
                .any(|existing| bindings_equivalent(existing, &binding))
            {
                bindings.push(binding);
            }
        }
        self.bindings = bindings;

        let mut listeners: Vec<_> = defaults
            .listeners
            .clone()
            .into_iter()
            .filter_map(InputActionListenerRegistration::normalized)
            .collect();
        for listener in self
            .listeners
            .into_iter()
            .filter_map(InputActionListenerRegistration::normalized)
        {
            upsert_listener_registration(&mut listeners, listener);
        }
        listeners.sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.id.cmp(&b.id)));
        self.listeners = listeners;

        let mut gamepad_axes: Vec<_> = self
            .gamepad_axes
            .into_iter()
            .filter_map(canonical_axis_binding)
            .collect();
        for axis in defaults
            .gamepad_axes
            .clone()
            .into_iter()
            .filter_map(canonical_axis_binding)
        {
            if !gamepad_axes
                .iter()
                .any(|existing| existing.axis == axis.axis && existing.target == axis.target)
            {
                gamepad_axes.push(axis);
            }
        }
        self.gamepad_axes = gamepad_axes;
        self
    }

    pub fn register_key(&mut self, key: InputKeyRegistration) -> Result<(), String> {
        let key = key
            .normalized()
            .ok_or_else(|| "invalid input key registration".to_owned())?;
        upsert_key_registration(&mut self.keys, key);
        self.keys
            .sort_by(|a, b| a.code.cmp(&b.code).then_with(|| a.id.cmp(&b.id)));
        Ok(())
    }

    pub fn register_action(&mut self, action: InputActionDefinition) -> Result<(), String> {
        let action = action
            .normalized()
            .ok_or_else(|| "invalid input action registration".to_owned())?;
        upsert_action_definition(&mut self.actions, action);
        Ok(())
    }

    pub fn register_binding(
        &mut self,
        registration: InputBindingRegistration,
    ) -> Result<(), String> {
        let binding = registration
            .binding
            .normalized()
            .ok_or_else(|| "invalid input binding registration".to_owned())?;
        if !self
            .actions
            .iter()
            .any(|action| action.id == binding.action)
        {
            return Err(format!(
                "input binding references undeclared action '{}'",
                binding.action
            ));
        }
        if registration.replace_existing_for_action_device {
            self.bindings.retain(|existing| {
                !(existing.action == binding.action && existing.device == binding.device)
            });
        } else if self
            .bindings
            .iter()
            .any(|existing| bindings_equivalent(existing, &binding))
        {
            return Ok(());
        }
        self.bindings.push(binding);
        Ok(())
    }

    pub fn register_listener(
        &mut self,
        listener: InputActionListenerRegistration,
    ) -> Result<(), String> {
        let listener = listener
            .normalized()
            .ok_or_else(|| "invalid input listener registration".to_owned())?;
        upsert_listener_registration(&mut self.listeners, listener);
        Ok(())
    }

    pub fn register_gamepad_axis(&mut self, axis: GamepadAxisBinding) -> Result<(), String> {
        let axis = canonical_axis_binding(axis)
            .ok_or_else(|| "invalid empty gamepad axis binding".to_owned())?;
        self.gamepad_axes
            .retain(|existing| !(existing.axis == axis.axis && existing.target == axis.target));
        self.gamepad_axes.push(axis);
        Ok(())
    }

    pub fn resolve<T: InputFrameSource>(&self, input: &T) -> InputActionFrame {
        let mut out = InputActionFrame::default();
        let actions = self.action_catalog();
        let mut seen = std::collections::BTreeSet::<String>::new();
        for binding in &self.bindings {
            if !binding_matches(binding, input) {
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

    #[inline]
    pub fn primary_binding_label(&self, action: &str) -> String {
        let action = newengine_input_actions_api::normalize_action_id(action)
            .unwrap_or_else(|| action.trim().to_owned());
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

impl Default for InputBindingsProfile {
    #[inline]
    fn default() -> Self {
        Self::empty("newengine.input.profile")
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
                "central-key-registry".to_owned(),
                "central-action-registry".to_owned(),
                "semantic-actions".to_owned(),
                "action-listeners".to_owned(),
                "listener-priority-consumption".to_owned(),
                "manifest-registration".to_owned(),
                "gamepad-bindings".to_owned(),
                "device-preference".to_owned(),
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
fn bindings_equivalent(a: &InputBinding, b: &InputBinding) -> bool {
    a.action == b.action
        && a.device == b.device
        && a.code == b.code
        && a.name == b.name
        && a.phase == b.phase
}

fn canonical_axis_binding(mut axis: GamepadAxisBinding) -> Option<GamepadAxisBinding> {
    axis.axis = axis.axis.trim().to_owned();
    axis.deadzone = axis.deadzone.clamp(0.0, 0.95);
    axis.scale = axis.scale.clamp(-8.0, 8.0);
    if axis.axis.is_empty() {
        None
    } else {
        Some(axis)
    }
}

fn upsert_key_registration(keys: &mut Vec<InputKeyRegistration>, key: InputKeyRegistration) {
    keys.retain(|existing| existing.code != key.code && existing.id != key.id);
    keys.push(key);
}

fn upsert_action_definition(
    actions: &mut Vec<InputActionDefinition>,
    action: InputActionDefinition,
) {
    if let Some(slot) = actions.iter_mut().find(|existing| existing.id == action.id) {
        *slot = action;
    } else {
        actions.push(action);
    }
}

fn upsert_listener_registration(
    listeners: &mut Vec<InputActionListenerRegistration>,
    listener: InputActionListenerRegistration,
) {
    listeners.retain(|existing| !(existing.owner == listener.owner && existing.id == listener.id));
    listeners.push(listener);
}

#[inline]
pub fn input_device_preference_is_display_only(_preference: InputDevicePreference) -> bool {
    // Device preference is intentionally not a hard gameplay input gate. It orders binding
    // labels and menu presentation only. Exclusive capture belongs to `engine.input.contexts`,
    // where modal policy can be expressed explicitly instead of silently disabling fallback
    // devices at the action resolver level.
    true
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
            let Some(name) = binding.name.as_deref() else {
                return false;
            };
            match binding.phase {
                InputBindingPhase::Down => input.is_gamepad_button_down(name),
                InputBindingPhase::Pressed => input.is_gamepad_button_pressed(name),
                InputBindingPhase::Released => input.is_gamepad_button_released(name),
            }
        }
    }
}

fn apply_gamepad_axes<T: InputFrameSource>(
    out: &mut InputActionFrame,
    axes: &[GamepadAxisBinding],
    input: &T,
) {
    for axis in axes {
        let mut value = input.gamepad_axis(&axis.axis);
        if value.abs() < axis.deadzone {
            value = 0.0;
        }
        value = (value * axis.scale).clamp(-1.0, 1.0);
        match axis.target {
            GamepadAxisTarget::MoveX => out.move_axis[0] += value,
            GamepadAxisTarget::MoveY => out.move_axis[1] += value,
            GamepadAxisTarget::MoveZ => out.move_axis[2] += value,
            GamepadAxisTarget::LookX => out.look_axis[0] += value,
            GamepadAxisTarget::LookY => out.look_axis[1] += value,
        }
    }
    out.move_axis = out.move_axis.map(|v| v.clamp(-1.0, 1.0));
    out.look_axis = out.look_axis.map(|v| v.clamp(-1.0, 1.0));
    if out.move_axis[0] > 0.0 {
        out.move_mask |= newengine_input_actions_api::move_mask::RIGHT;
    }
    if out.move_axis[0] < 0.0 {
        out.move_mask |= newengine_input_actions_api::move_mask::LEFT;
    }
    if out.move_axis[1] > 0.0 {
        out.move_mask |= newengine_input_actions_api::move_mask::UP;
    }
    if out.move_axis[1] < 0.0 {
        out.move_mask |= newengine_input_actions_api::move_mask::DOWN;
    }
    if out.move_axis[2] > 0.0 {
        out.move_mask |= newengine_input_actions_api::move_mask::FORWARD;
    }
    if out.move_axis[2] < 0.0 {
        out.move_mask |= newengine_input_actions_api::move_mask::BACK;
    }
}

fn dispatch_action_definition(
    out: &mut InputActionFrame,
    definition: &InputActionDefinition,
    listeners: &[InputActionListenerRegistration],
) {
    out.actions.push(definition.id.clone());
    out.events.push(dispatch_event_for(definition, listeners));
    for effect in &definition.effects {
        apply_action_effect(out, effect);
    }
    out.move_axis = newengine_input_actions_api::move_axis_from_mask(out.move_mask);
}

fn dispatch_event_for(
    definition: &InputActionDefinition,
    listeners: &[InputActionListenerRegistration],
) -> newengine_input_actions_api::InputActionDispatchEvent {
    let mut event = newengine_input_actions_api::InputActionDispatchEvent {
        action: definition.id.clone(),
        listeners: Vec::new(),
        consumed_by: None,
    };
    for listener in listeners
        .iter()
        .filter(|listener| listener.enabled && listener_matches_action(listener, definition))
    {
        let listener_id = format!("{}:{}", listener.owner, listener.id);
        event.listeners.push(listener_id.clone());
        if definition.dispatch == InputActionDispatchMode::ConsumeFirst && listener.consume {
            event.consumed_by = Some(listener_id);
            break;
        }
    }
    event
}

fn listener_matches_action(
    listener: &InputActionListenerRegistration,
    definition: &InputActionDefinition,
) -> bool {
    let action_match = listener.action_filter.is_empty()
        || listener
            .action_filter
            .iter()
            .any(|action| action == &definition.id);
    if !action_match {
        return false;
    }
    listener.context_filter.is_empty()
        || definition
            .contexts
            .iter()
            .any(|ctx| listener.context_filter.iter().any(|wanted| wanted == ctx))
}

fn apply_action_effect(out: &mut InputActionFrame, effect: &InputActionEffect) {
    match effect {
        InputActionEffect::MoveMask { mask } => out.move_mask |= *mask,
        InputActionEffect::Sprint { enabled } => out.sprint |= *enabled,
        InputActionEffect::CameraView { request } => out.camera_view = *request,
        InputActionEffect::UiToggle => out.ui_toggle = true,
        InputActionEffect::UiAccept => out.ui_accept = true,
        InputActionEffect::UiBack => out.ui_back = true,
        InputActionEffect::UiNav { x, y } => {
            out.ui_nav[0] = out.ui_nav[0].saturating_add(*x).clamp(-1, 1);
            out.ui_nav[1] = out.ui_nav[1].saturating_add(*y).clamp(-1, 1);
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
        key_code::F1 => "F1",
        key_code::F2 => "F2",
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
