#![forbid(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};

pub const ENGINE_INPUT_ACTIONS_SERVICE_ID: &str = "engine.input.actions";
pub const INPUT_ACTIONS_SERVICE_ID: &str = "input.actions.api";
pub const INPUT_ACTIONS_BACKEND_CAPABILITY_ID: &str = "input.actions.backend";

pub const INPUT_ACTIONS_METHOD_INFO: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
pub const INPUT_ACTIONS_METHOD_INVOKE: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
pub const INPUT_ACTIONS_METHOD_SHUTDOWN_V1: &str =
    newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
pub const INPUT_ACTIONS_METHOD_FRAME_JSON_V1: &str = "frame_json_v1";
pub const INPUT_ACTIONS_METHOD_DISPATCH_JSON_V1: &str = "dispatch_json_v1";

pub const INPUT_ACTIONS_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "input.actions",
        ENGINE_INPUT_ACTIONS_SERVICE_ID,
        INPUT_ACTIONS_SERVICE_ID,
        INPUT_ACTIONS_BACKEND_CAPABILITY_ID,
    );

pub const INPUT_ACTIONS_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_INPUT_ACTIONS_SERVICE_ID,
        "newengine.input-actions-api >= 0.1.x",
        &[
            newengine_service_api::SERVICE_METHOD_INFO_JSON,
            newengine_service_api::SERVICE_METHOD_INVOKE_JSON,
            newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1,
            INPUT_ACTIONS_METHOD_FRAME_JSON_V1,
        ],
    );

pub const INPUT_ACTIONS_RUNTIME_REQUIREMENT_SPEC:
    newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        INPUT_ACTIONS_RUNTIME_CONTRACT_SPEC,
        Some(INPUT_ACTIONS_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_INPUT_ACTIONS"),
    );

/// Gameplay movement bit-mask values carried by semantic action frames.
/// These values are generic semantic effects, not a concrete keybinding profile.
pub mod move_mask {
    pub const FORWARD: u64 = 1 << 0;
    pub const LEFT: u64 = 1 << 1;
    pub const BACK: u64 = 1 << 2;
    pub const RIGHT: u64 = 1 << 3;
    pub const UP: u64 = 1 << 4;
    pub const DOWN: u64 = 1 << 5;
    pub const SPRINT: u64 = 1 << 6;
}

/// Engine-reserved semantic action ids. These are action contracts, not platform shortcuts.
pub mod engine_action {
    pub const UI_NAVIGATION_TOGGLE: &str = "engine.ui.primary.toggle";
    pub const UI_NAVIGATION_ACCEPT: &str = "ui.accept";
    pub const UI_NAVIGATION_BACK: &str = "ui.back";
    pub const UI_NAVIGATION_UP: &str = "ui.nav.up";
    pub const UI_NAVIGATION_DOWN: &str = "ui.nav.down";
    pub const UI_NAVIGATION_LEFT: &str = "ui.nav.left";
    pub const UI_NAVIGATION_RIGHT: &str = "ui.nav.right";
    pub const ASSET_CATALOG_UI_TOGGLE: &str = "ui.assets.catalog.toggle";
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
    /// action consumer for the current frame.
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
fn default_true() -> bool {
    true
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputActionDispatchMode {
    Broadcast,
    ConsumeFirst,
}

impl Default for InputActionDispatchMode {
    #[inline]
    fn default() -> Self {
        Self::Broadcast
    }
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
    fn default() -> Self {
        Self::None
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InputActionEffect {
    MoveMask { mask: u64 },
    Sprint { enabled: bool },
    CameraView { request: CameraViewRequest },
    UiToggle,
    UiAccept,
    UiBack,
    UiNav { x: i8, y: i8 },
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
    pub ui_toggle: bool,
    #[serde(default)]
    pub ui_accept: bool,
    #[serde(default)]
    pub ui_back: bool,
    #[serde(default)]
    pub ui_nav: [i8; 2],
    #[serde(default)]
    pub actions: Vec<String>,
    #[serde(default)]
    pub events: Vec<InputActionDispatchEvent>,
}

impl Default for InputActionFrame {
    #[inline]
    fn default() -> Self {
        Self {
            move_mask: 0,
            move_axis: [0.0, 0.0, 0.0],
            sprint: false,
            look_axis: [0.0, 0.0],
            camera_view: CameraViewRequest::None,
            ui_toggle: false,
            ui_accept: false,
            ui_back: false,
            ui_nav: [0, 0],
            actions: Vec::new(),
            events: Vec::new(),
        }
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

pub trait InputFrameSource {
    fn is_key_down(&self, key: u32) -> bool;
    fn is_key_pressed(&self, key: u32) -> bool;
    fn is_key_released(&self, key: u32) -> bool;
    fn is_mouse_down(&self, button: u32) -> bool;
    fn is_mouse_pressed(&self, button: u32) -> bool;
    fn is_mouse_released(&self, button: u32) -> bool;

    /// Returns true when at least one gamepad is currently visible to the raw input backend.
    ///
    /// This is intentionally advisory. Binding profiles use it for diagnostics and fallback
    /// policy, not for hard device gating; modal capture belongs to `engine.input.contexts`.
    #[inline]
    fn has_gamepad_connected(&self) -> bool {
        false
    }

    #[inline]
    fn is_gamepad_button_down(&self, _button: &str) -> bool {
        false
    }
    #[inline]
    fn is_gamepad_button_pressed(&self, _button: &str) -> bool {
        false
    }
    #[inline]
    fn is_gamepad_button_released(&self, _button: &str) -> bool {
        false
    }
    #[inline]
    fn gamepad_axis(&self, _axis: &str) -> f32 {
        0.0
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InputActionsServiceInfo {
    pub protocol: String,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub methods: Vec<String>,
}

impl Default for InputActionsServiceInfo {
    fn default() -> Self {
        Self {
            protocol: "newengine.input-actions/v1".to_owned(),
            features: vec![
                "semantic-action-frame".to_owned(),
                "action-dispatch-events".to_owned(),
                "listener-priority-consumption".to_owned(),
            ],
            methods: vec![
                INPUT_ACTIONS_METHOD_INFO.to_owned(),
                INPUT_ACTIONS_METHOD_INVOKE.to_owned(),
                INPUT_ACTIONS_METHOD_SHUTDOWN_V1.to_owned(),
                INPUT_ACTIONS_METHOD_FRAME_JSON_V1.to_owned(),
            ],
        }
    }
}

#[inline]
pub fn normalize_action_id(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.contains(char::is_whitespace)
        || trimmed.contains('/')
        || trimmed.contains('\\')
    {
        None
    } else {
        Some(trimmed.to_ascii_lowercase())
    }
}

#[inline]
pub fn normalize_id_like(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.contains(char::is_whitespace)
        || trimmed.contains('/')
        || trimmed.contains('\\')
    {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

pub fn normalize_string_list(values: Vec<String>) -> Vec<String> {
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

pub fn normalize_action_filter(values: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    for value in values {
        let Some(action) = normalize_action_id(&value) else {
            continue;
        };
        if out.iter().any(|v: &String| v == &action) {
            continue;
        }
        out.push(action);
    }
    out
}

#[inline]
pub fn move_axis_from_mask(mask: u64) -> [f32; 3] {
    let x = ((mask & move_mask::RIGHT != 0) as i32 - (mask & move_mask::LEFT != 0) as i32) as f32;
    let y = ((mask & move_mask::UP != 0) as i32 - (mask & move_mask::DOWN != 0) as i32) as f32;
    let z = ((mask & move_mask::FORWARD != 0) as i32 - (mask & move_mask::BACK != 0) as i32) as f32;
    [x, y, z]
}
