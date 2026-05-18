use std::sync::OnceLock;

use abi_stable::erased_types::TD_Opaque;
use abi_stable::std_types::{RResult, RString};
use newengine_loading_api::{
    LoadingScreenSnapshot, LoadingServiceInfo, ENGINE_LOADING_SERVICE_ID,
    LOADING_BACKEND_CAPABILITY_ID, LOADING_SERVICE_METHOD_INFO,
    LOADING_SERVICE_METHOD_INVOKE, LOADING_SERVICE_METHOD_PUBLISH_JSON_V1,
    LOADING_SERVICE_METHOD_SHUTDOWN_V1, LOADING_SERVICE_METHOD_SNAPSHOT_JSON_V1,
};
use newengine_loading_runtime::{
    project_loading_snapshot_from_overlay_fields, SharedLoadingSnapshot,
};
use newengine_platform_api::{PlatformLoadingOverlayV1, PlatformStepResultV1};
use newengine_plugin_api::{Blob, CapabilityId, MethodName, ServiceV1, ServiceV1_TO};

static LOADING_SNAPSHOT: OnceLock<SharedLoadingSnapshot> = OnceLock::new();

#[derive(Clone)]
struct EngineLoadingGatewayService {
    state: SharedLoadingSnapshot,
}

impl EngineLoadingGatewayService {
    #[inline]
    fn new(state: SharedLoadingSnapshot) -> Self {
        Self { state }
    }

    #[inline]
    fn ok_json<T: serde::Serialize>(value: &T) -> RResult<Blob, RString> {
        match serde_json::to_vec(value) {
            Ok(bytes) => RResult::ROk(Blob::from(bytes)),
            Err(e) => RResult::RErr(RString::from(e.to_string())),
        }
    }
}

impl ServiceV1 for EngineLoadingGatewayService {
    fn id(&self) -> CapabilityId {
        CapabilityId::from(ENGINE_LOADING_SERVICE_ID)
    }

    fn describe(&self) -> RString {
        let v = serde_json::json!({
            "id": ENGINE_LOADING_SERVICE_ID,
            "version": 1,
            "origin": "engine-owned",
            "owner": "newengine-runtime-host.loading-gateway",
            "capability": LOADING_BACKEND_CAPABILITY_ID,
            "methods": [
                LOADING_SERVICE_METHOD_INFO,
                LOADING_SERVICE_METHOD_INVOKE,
                LOADING_SERVICE_METHOD_SHUTDOWN_V1,
                LOADING_SERVICE_METHOD_SNAPSHOT_JSON_V1,
                LOADING_SERVICE_METHOD_PUBLISH_JSON_V1
            ],
            "notes": "Engine-facing loading shell gateway for startup overlay snapshots and native compositor state."
        });
        RString::from(serde_json::to_string(&v).unwrap_or_else(|_| "{}".to_owned()))
    }

    fn call(&self, method: MethodName, payload: Blob) -> RResult<Blob, RString> {
        match method.as_str() {
            LOADING_SERVICE_METHOD_INFO => Self::ok_json(&LoadingServiceInfo::default()),
            LOADING_SERVICE_METHOD_SNAPSHOT_JSON_V1 | LOADING_SERVICE_METHOD_INVOKE => {
                Self::ok_json(&self.state.snapshot())
            }
            LOADING_SERVICE_METHOD_PUBLISH_JSON_V1 => {
                match serde_json::from_slice::<LoadingScreenSnapshot>(payload.as_slice()) {
                    Ok(snapshot) => {
                        self.state.publish(snapshot);
                        Self::ok_json(&serde_json::json!({ "ok": true }))
                    }
                    Err(e) => RResult::RErr(RString::from(format!(
                        "engine.loading: invalid publish_json_v1 payload: {e}"
                    ))),
                }
            }
            LOADING_SERVICE_METHOD_SHUTDOWN_V1 => Self::ok_json(&serde_json::json!({ "ok": true })),
            other => RResult::RErr(RString::from(format!(
                "engine.loading: unknown method '{}'",
                other
            ))),
        }
    }
}

pub(crate) fn register_loading_gateway_service_best_effort() {
    let state = LOADING_SNAPSHOT
        .get_or_init(|| SharedLoadingSnapshot::new(LoadingScreenSnapshot::default()))
        .clone();

    if newengine_plugin_host::has_service(ENGINE_LOADING_SERVICE_ID) {
        return;
    }

    let svc = EngineLoadingGatewayService::new(state);
    let dyn_svc = ServiceV1_TO::from_value(svc, TD_Opaque);
    let host = newengine_plugin_host::default_host_api();

    match (host.register_service_v1)(dyn_svc) {
        RResult::ROk(()) => {
            match newengine_plugin_host::register_engine_owned_gateway(
                ENGINE_LOADING_SERVICE_ID,
                newengine_service_api::EngineServiceKind::Loading,
                ENGINE_LOADING_SERVICE_ID,
                LOADING_BACKEND_CAPABILITY_ID,
                0,
                "newengine-runtime-host.loading-gateway",
            ) {
                Ok(()) => log::info!(
                    "engine.loading gateway registered source=engine-owned service='{}' capability='{}'",
                    ENGINE_LOADING_SERVICE_ID,
                    LOADING_BACKEND_CAPABILITY_ID
                ),
                Err(e) => log::warn!(
                    "engine.loading gateway route registration skipped id='{}' err='{}'",
                    ENGINE_LOADING_SERVICE_ID,
                    e
                ),
            }
        }
        RResult::RErr(e) => {
            log::error!(
                "engine.loading service registration failed id='{}' err='{}'",
                ENGINE_LOADING_SERVICE_ID,
                e
            );
        }
    }
}

pub(crate) fn publish_platform_step_result(step: &PlatformStepResultV1) {
    publish_platform_loading_overlay(&step.loading_overlay, "platform-step-result");
}

pub(crate) fn publish_platform_loading_overlay(overlay: &PlatformLoadingOverlayV1, source: &'static str) {
    let Some(state) = LOADING_SNAPSHOT.get() else {
        return;
    };
    let snapshot = project_loading_snapshot_from_overlay_fields(
        overlay.active,
        overlay.title.as_str(),
        overlay.status.as_str(),
        overlay.detail.as_str(),
        overlay.progress_01,
        overlay.spinner_phase,
        overlay.view_json.as_str(),
        source,
        "engine-owned-native-shell",
    );
    state.publish(snapshot);
}
