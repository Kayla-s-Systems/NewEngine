#![forbid(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};

pub const ENGINE_INPUT_CONTEXTS_SERVICE_ID: &str = "engine.input.contexts";
pub const INPUT_CONTEXTS_SERVICE_ID: &str = "input.contexts.api";
pub const INPUT_CONTEXTS_BACKEND_CAPABILITY_ID: &str = "input.contexts.backend";

pub const INPUT_CONTEXTS_METHOD_INFO: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
pub const INPUT_CONTEXTS_METHOD_INVOKE: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
pub const INPUT_CONTEXTS_METHOD_SHUTDOWN_V1: &str =
    newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
pub const INPUT_CONTEXTS_METHOD_STACK_JSON_V1: &str = "stack_json_v1";
pub const INPUT_CONTEXTS_METHOD_PUSH_JSON_V1: &str = "push_json_v1";
pub const INPUT_CONTEXTS_METHOD_POP_JSON_V1: &str = "pop_json_v1";

pub const INPUT_CONTEXTS_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "input.contexts",
        ENGINE_INPUT_CONTEXTS_SERVICE_ID,
        INPUT_CONTEXTS_SERVICE_ID,
        INPUT_CONTEXTS_BACKEND_CAPABILITY_ID,
    );

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputCapturePolicy {
    ObserveOnly,
    ConsumeMatched,
    ConsumeAll,
}

impl Default for InputCapturePolicy {
    #[inline]
    fn default() -> Self {
        Self::ObserveOnly
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputContextLifetime {
    Frame,
    Modal,
    Persistent,
}

impl Default for InputContextLifetime {
    #[inline]
    fn default() -> Self {
        Self::Modal
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputContext {
    pub id: String,
    #[serde(default)]
    pub owner: String,
    #[serde(default)]
    pub priority: i32,
    #[serde(default)]
    pub capture: InputCapturePolicy,
    #[serde(default)]
    pub lifetime: InputContextLifetime,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl InputContext {
    #[inline]
    pub fn new(id: impl Into<String>, owner: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            owner: owner.into(),
            priority: 0,
            capture: InputCapturePolicy::ObserveOnly,
            lifetime: InputContextLifetime::Modal,
            enabled: true,
        }
    }

    #[inline]
    pub fn consuming(mut self) -> Self {
        self.capture = InputCapturePolicy::ConsumeMatched;
        self
    }

    #[inline]
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputContextStack {
    #[serde(default)]
    pub contexts: Vec<InputContext>,
}

impl InputContextStack {
    pub fn canonicalized(mut self) -> Self {
        self.contexts.retain(|ctx| !ctx.id.trim().is_empty());
        self.contexts
            .sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.id.cmp(&b.id)));
        self
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InputContextsServiceInfo {
    pub protocol: String,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub methods: Vec<String>,
}

impl Default for InputContextsServiceInfo {
    fn default() -> Self {
        Self {
            protocol: "newengine.input-contexts/v1".to_owned(),
            features: vec![
                "context-stack".to_owned(),
                "modal-capture".to_owned(),
                "priority-consume-policy".to_owned(),
            ],
            methods: vec![
                INPUT_CONTEXTS_METHOD_INFO.to_owned(),
                INPUT_CONTEXTS_METHOD_INVOKE.to_owned(),
                INPUT_CONTEXTS_METHOD_SHUTDOWN_V1.to_owned(),
                INPUT_CONTEXTS_METHOD_STACK_JSON_V1.to_owned(),
                INPUT_CONTEXTS_METHOD_PUSH_JSON_V1.to_owned(),
                INPUT_CONTEXTS_METHOD_POP_JSON_V1.to_owned(),
            ],
        }
    }
}

/// Provider-neutral input capture state published by UI/input policy.
///
/// Hard contract: UI can consume pointer/text/navigation, but camera sampling
/// remains alive. Runtime camera code receives an input-state frame every tick;
/// `gameplay_navigation_blocked` gates applying deltas/actions, not listener
/// subscription.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct InputCaptureStateV1 {
    pub camera_listener_alive: bool,
    pub gameplay_navigation_blocked: bool,
    pub ui_pointer_capture: bool,
    pub ui_text_capture: bool,
    pub ui_navigation_capture: bool,
    pub owner: String,
    pub reason: String,
}

impl Default for InputCaptureStateV1 {
    fn default() -> Self {
        Self {
            camera_listener_alive: true,
            gameplay_navigation_blocked: false,
            ui_pointer_capture: false,
            ui_text_capture: false,
            ui_navigation_capture: false,
            owner: "engine.input.contexts".to_owned(),
            reason: "observe-only".to_owned(),
        }
    }
}

impl InputCaptureStateV1 {
    #[inline]
    pub fn modal_ui(owner: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            camera_listener_alive: true,
            gameplay_navigation_blocked: true,
            ui_pointer_capture: true,
            ui_text_capture: true,
            ui_navigation_capture: true,
            owner: owner.into(),
            reason: reason.into(),
        }
    }
}

#[inline]
fn default_true() -> bool {
    true
}
