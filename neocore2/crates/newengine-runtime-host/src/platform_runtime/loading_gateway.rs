use std::sync::OnceLock;

use abi_stable::std_types::RResult;
use newengine_loading_api::{
    LoadingScreenSnapshot, LoadingServiceInfo, ENGINE_LOADING_SERVICE_ID,
    LOADING_BACKEND_CAPABILITY_ID, LOADING_SERVICE_METHOD_INVOKE,
    LOADING_SERVICE_METHOD_PUBLISH_JSON_V1, LOADING_SERVICE_METHOD_SNAPSHOT_JSON_V1,
};
use newengine_loading_runtime::{
    project_loading_snapshot_from_overlay_fields, SharedLoadingSnapshot,
};
use newengine_platform_api::{PlatformLoadingOverlayV1, PlatformStepResultV1};
use newengine_plugin_api::Blob;
use newengine_service_kit::{
    decode_json_payload, engine_owned_service_description, ok_json,
    register_engine_owned_gateway_service, EngineOwnedGatewayDecl, JsonServiceRouter,
};

static LOADING_SNAPSHOT: OnceLock<SharedLoadingSnapshot> = OnceLock::new();
const LOADING_GATEWAY_OWNER: &str = "newengine-runtime-host.loading-gateway";

fn loading_gateway_service(state: SharedLoadingSnapshot) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let info = LoadingServiceInfo::default();
    let description = engine_owned_service_description(
        ENGINE_LOADING_SERVICE_ID,
        LOADING_GATEWAY_OWNER,
        LOADING_BACKEND_CAPABILITY_ID,
        info.methods.clone(),
    )
    .protocol(info.protocol.clone())
    .features(info.features.clone())
    .notes("Engine-facing loading shell gateway for startup overlay snapshots and native compositor state.");

    let invoke_state = state.clone();
    let snapshot_state = state.clone();
    let publish_state = state;

    JsonServiceRouter::new(ENGINE_LOADING_SERVICE_ID)
        .describe_json(&description)
        .info(LoadingServiceInfo::default)
        .get_json(LOADING_SERVICE_METHOD_INVOKE, move |_| invoke_state.snapshot())
        .get_json(LOADING_SERVICE_METHOD_SNAPSHOT_JSON_V1, move |_| snapshot_state.snapshot())
        .blob(LOADING_SERVICE_METHOD_PUBLISH_JSON_V1, move |_unit, payload: Blob| {
            let snapshot = match decode_json_payload::<LoadingScreenSnapshot>(
                ENGINE_LOADING_SERVICE_ID,
                LOADING_SERVICE_METHOD_PUBLISH_JSON_V1,
                &payload,
            ) {
                Ok(snapshot) => snapshot,
                Err(e) => return RResult::RErr(e),
            };
            publish_state.publish(snapshot);
            ok_json(&serde_json::json!({ "ok": true }))
        })
        .shutdown_json(|_| serde_json::json!({ "ok": true }))
        .into_service_v1()
}

pub(crate) fn register_loading_gateway_service_best_effort() {
    let state = LOADING_SNAPSHOT
        .get_or_init(|| SharedLoadingSnapshot::new(LoadingScreenSnapshot::default()))
        .clone();

    if newengine_plugin_host::has_service(ENGINE_LOADING_SERVICE_ID) {
        return;
    }

    let service = loading_gateway_service(state);
    match register_engine_owned_gateway_service(EngineOwnedGatewayDecl {
        gateway: ENGINE_LOADING_SERVICE_ID,
        service_kind: newengine_service_api::EngineServiceKind::Loading,
        provider_service: ENGINE_LOADING_SERVICE_ID,
        capability: LOADING_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: LOADING_GATEWAY_OWNER,
        service,
    }) {
        Ok(()) => log::info!(
            "engine.loading gateway registered source=engine-owned service='{}' capability='{}'",
            ENGINE_LOADING_SERVICE_ID,
            LOADING_BACKEND_CAPABILITY_ID
        ),
        Err(e) => log::error!(
            "engine.loading gateway registration failed id='{}' err='{}'",
            ENGINE_LOADING_SERVICE_ID,
            e
        ),
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
