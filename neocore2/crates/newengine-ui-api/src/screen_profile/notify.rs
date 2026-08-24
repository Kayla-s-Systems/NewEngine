use serde::{Deserialize, Serialize};

use super::{UiToastNotification, UiToastSeverity};

pub const ENGINE_UI_NOTIFY_SERVICE_ID: &str = "engine.ui.notify";
pub const UI_NOTIFY_SERVICE_ID: &str = "ui.notify.runtime";
pub const UI_NOTIFY_BACKEND_CAPABILITY_ID: &str = "ui.notify.backend";
pub const UI_NOTIFY_PROVIDER_ROUTE: &str = "engine.ui.notify.runtime";
pub const UI_NOTIFY_RUNTIME_CONTRACT: &str = "newengine.ui.notify.runtime.v1";

pub const UI_NOTIFY_MESSAGE_ID: &str = "engine.ui.notify";
pub const UI_NOTIFY_INFO_MESSAGE_ID: &str = "engine.ui.notify.info";
pub const UI_NOTIFY_SUCCESS_MESSAGE_ID: &str = "engine.ui.notify.success";
pub const UI_NOTIFY_WARNING_MESSAGE_ID: &str = "engine.ui.notify.warning";
pub const UI_NOTIFY_ERROR_MESSAGE_ID: &str = "engine.ui.notify.error";

pub const UI_NOTIFY_MESSAGE_IDS: &[&str] = &[
    UI_NOTIFY_MESSAGE_ID,
    UI_NOTIFY_INFO_MESSAGE_ID,
    UI_NOTIFY_SUCCESS_MESSAGE_ID,
    UI_NOTIFY_WARNING_MESSAGE_ID,
    UI_NOTIFY_ERROR_MESSAGE_ID,
];

pub mod ui_notify_method {
    pub const INFO_JSON: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
    pub const INVOKE_JSON: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
    pub const SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
    pub const PUSH_V1: &str = "ui.notify.push_v1";
    pub const DISMISS_V1: &str = "ui.notify.dismiss_v1";
    pub const CLEAR_V1: &str = "ui.notify.clear_v1";
    pub const SNAPSHOT_V1: &str = "ui.notify.snapshot_v1";
}

pub const UI_NOTIFY_SERVICE_METHODS: &[&str] = &[
    ui_notify_method::INFO_JSON,
    ui_notify_method::INVOKE_JSON,
    ui_notify_method::SHUTDOWN_V1,
    ui_notify_method::PUSH_V1,
    ui_notify_method::DISMISS_V1,
    ui_notify_method::CLEAR_V1,
    ui_notify_method::SNAPSHOT_V1,
];

pub const UI_NOTIFY_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "ui.notify",
        ENGINE_UI_NOTIFY_SERVICE_ID,
        UI_NOTIFY_SERVICE_ID,
        UI_NOTIFY_BACKEND_CAPABILITY_ID,
    );

pub const UI_NOTIFY_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_UI_NOTIFY_SERVICE_ID,
        UI_NOTIFY_RUNTIME_CONTRACT,
        UI_NOTIFY_SERVICE_METHODS,
    );

pub const UI_NOTIFY_RUNTIME_REQUIREMENT_SPEC: newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        UI_NOTIFY_RUNTIME_CONTRACT_SPEC,
        Some(UI_NOTIFY_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_UI_NOTIFY_BACKEND"),
    );

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiNotifyRequest {
    pub id: String,
    pub title: String,
    pub detail: String,
    pub severity: UiToastSeverity,
    pub source: String,
    /// Zero selects the severity-specific runtime default.
    pub duration_ms: u64,
    pub sticky: bool,
    pub progress_permille: Option<u16>,
    pub replace_existing: bool,
    pub correlation_id: Option<String>,
}

impl Default for UiNotifyRequest {
    fn default() -> Self {
        Self {
            id: String::new(),
            title: String::new(),
            detail: String::new(),
            severity: UiToastSeverity::Info,
            source: "engine".to_owned(),
            duration_ms: 0,
            sticky: false,
            progress_permille: None,
            replace_existing: true,
            correlation_id: None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiNotifyDismissRequest {
    pub id: String,
    pub source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiNotifyClearRequest {
    pub source: Option<String>,
    pub include_sticky: bool,
}

impl Default for UiNotifyClearRequest {
    fn default() -> Self {
        Self {
            source: None,
            include_sticky: false,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiNotifyMutationResponse {
    pub accepted: bool,
    pub affected: usize,
    pub id: String,
    pub queue_depth: usize,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiNotifySnapshotV1 {
    pub version: u32,
    pub generation: u64,
    pub active: usize,
    pub visible_limit: usize,
    pub capacity: usize,
    pub dropped: u64,
    pub notifications: Vec<UiToastNotification>,
}

impl Default for UiNotifySnapshotV1 {
    fn default() -> Self {
        Self {
            version: 1,
            generation: 0,
            active: 0,
            visible_limit: 4,
            capacity: 64,
            dropped: 0,
            notifications: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiNotifyServiceInfoV1 {
    pub service_id: String,
    pub gateway: String,
    pub provider_route: String,
    pub contract: String,
    pub methods: Vec<String>,
    pub message_ids: Vec<String>,
    pub bounded: bool,
    pub message_pipeline_subscription: bool,
}

impl Default for UiNotifyServiceInfoV1 {
    fn default() -> Self {
        Self {
            service_id: UI_NOTIFY_SERVICE_ID.to_owned(),
            gateway: ENGINE_UI_NOTIFY_SERVICE_ID.to_owned(),
            provider_route: UI_NOTIFY_PROVIDER_ROUTE.to_owned(),
            contract: UI_NOTIFY_RUNTIME_CONTRACT.to_owned(),
            methods: UI_NOTIFY_SERVICE_METHODS
                .iter()
                .map(|method| (*method).to_owned())
                .collect(),
            message_ids: UI_NOTIFY_MESSAGE_IDS
                .iter()
                .map(|id| (*id).to_owned())
                .collect(),
            bounded: true,
            message_pipeline_subscription: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notify_contract_is_gateway_first_and_versioned() {
        assert_eq!(ENGINE_UI_NOTIFY_SERVICE_ID, "engine.ui.notify");
        assert_eq!(
            UI_NOTIFY_BACKEND_SERVICE_SPEC.engine_gateway_id,
            ENGINE_UI_NOTIFY_SERVICE_ID
        );
        assert!(UI_NOTIFY_SERVICE_METHODS.contains(&ui_notify_method::PUSH_V1));
        assert!(UI_NOTIFY_SERVICE_METHODS.contains(&ui_notify_method::SNAPSHOT_V1));
    }

    #[test]
    fn notify_request_defaults_to_replaceable_transient_info() {
        let request = UiNotifyRequest::default();
        assert_eq!(request.severity, UiToastSeverity::Info);
        assert!(request.replace_existing);
        assert!(!request.sticky);
        assert_eq!(request.duration_ms, 0);
    }
}
