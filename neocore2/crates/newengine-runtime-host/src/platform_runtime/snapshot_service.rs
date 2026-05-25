use std::sync::{Arc, Mutex, OnceLock};

use newengine_platform_api::{
    PlatformServiceInfo, PlatformWindowReadyV1, ENGINE_PLATFORM_SERVICE_ID,
    PLATFORM_BACKEND_CAPABILITY_ID, PLATFORM_SERVICE_METHOD_INVOKE,
    PLATFORM_SERVICE_METHOD_WINDOW_SNAPSHOT_JSON_V1,
};
use newengine_service_kit::{
    engine_owned_service_description, ok_json, register_engine_owned_gateway_service,
    EngineOwnedGatewayDecl, JsonServiceRouter,
};

static PLATFORM_WINDOW_SNAPSHOT: OnceLock<Arc<Mutex<PlatformWindowReadyV1>>> = OnceLock::new();
const PLATFORM_GATEWAY_OWNER: &str = "newengine-runtime-host.platform-gateway";

fn read_platform_window_snapshot(snapshot: &Arc<Mutex<PlatformWindowReadyV1>>) -> PlatformWindowReadyV1 {
    match snapshot.lock() {
        Ok(v) => *v,
        Err(e) => *e.into_inner(),
    }
}

fn platform_window_service(snapshot: Arc<Mutex<PlatformWindowReadyV1>>) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let info = PlatformServiceInfo::default();
    let description = engine_owned_service_description(
        ENGINE_PLATFORM_SERVICE_ID,
        PLATFORM_GATEWAY_OWNER,
        PLATFORM_BACKEND_CAPABILITY_ID,
        info.methods.clone(),
    )
    .protocol(info.protocol.clone())
    .features(info.features.clone())
    .notes("Engine-facing platform gateway for native window handles and surface metrics.");

    let window_snapshot = snapshot.clone();
    JsonServiceRouter::new(ENGINE_PLATFORM_SERVICE_ID)
        .describe_json(&description)
        .info(PlatformServiceInfo::default)
        .blob(PLATFORM_SERVICE_METHOD_INVOKE, |_unit, payload| {
            ok_json(&serde_json::json!({
                "ok": false,
                "error": "engine.platform invoke_json has no generic command envelope yet",
                "payload_len": payload.as_slice().len()
            }))
        })
        .get_json(PLATFORM_SERVICE_METHOD_WINDOW_SNAPSHOT_JSON_V1, move |_| {
            read_platform_window_snapshot(&window_snapshot)
        })
        .shutdown_json(|_| serde_json::json!({ "ok": true }))
        .into_service_v1()
}

pub(crate) fn register_platform_window_service_best_effort(initial: PlatformWindowReadyV1) {
    let snapshot = PLATFORM_WINDOW_SNAPSHOT
        .get_or_init(|| Arc::new(Mutex::new(initial)))
        .clone();

    match snapshot.lock() {
        Ok(mut guard) => *guard = initial,
        Err(e) => *e.into_inner() = initial,
    }

    if newengine_core::has_engine_gateway_route(ENGINE_PLATFORM_SERVICE_ID) {
        return;
    }

    let service = platform_window_service(snapshot);
    match register_engine_owned_gateway_service(EngineOwnedGatewayDecl {
        gateway: ENGINE_PLATFORM_SERVICE_ID,
        service_kind: newengine_service_api::EngineServiceKind::Platform,
        provider_service: ENGINE_PLATFORM_SERVICE_ID,
        capability: PLATFORM_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: PLATFORM_GATEWAY_OWNER,
        service,
    }) {
        Ok(()) => log::info!(
            "engine.platform gateway registered source=engine-owned service='{}' capability='{}'",
            ENGINE_PLATFORM_SERVICE_ID,
            PLATFORM_BACKEND_CAPABILITY_ID
        ),
        Err(e) => log::error!(
            "engine.platform gateway registration failed id='{}' err='{}'",
            ENGINE_PLATFORM_SERVICE_ID,
            e
        ),
    }
}

pub(crate) fn update_platform_window_snapshot(ready: PlatformWindowReadyV1) {
    if let Some(snapshot) = PLATFORM_WINDOW_SNAPSHOT.get() {
        match snapshot.lock() {
            Ok(mut guard) => *guard = ready,
            Err(e) => *e.into_inner() = ready,
        }
    }
}
