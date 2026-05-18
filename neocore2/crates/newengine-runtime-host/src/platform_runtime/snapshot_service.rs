use std::sync::{Arc, Mutex, OnceLock};

use abi_stable::erased_types::TD_Opaque;
use abi_stable::std_types::{RResult, RString};
use newengine_platform_api::{
    PlatformServiceInfo, PlatformWindowReadyV1, ENGINE_PLATFORM_SERVICE_ID,
    PLATFORM_BACKEND_CAPABILITY_ID, PLATFORM_SERVICE_METHOD_INFO,
    PLATFORM_SERVICE_METHOD_INVOKE, PLATFORM_SERVICE_METHOD_SHUTDOWN_V1,
    PLATFORM_SERVICE_METHOD_WINDOW_SNAPSHOT_JSON_V1,
};
use newengine_plugin_api::{
    Blob, CapabilityId, MethodName, ServiceV1, ServiceV1_TO,
};

static PLATFORM_WINDOW_SNAPSHOT: OnceLock<Arc<Mutex<PlatformWindowReadyV1>>> = OnceLock::new();

#[derive(Clone)]
struct EnginePlatformSnapshotService {
    snapshot: Arc<Mutex<PlatformWindowReadyV1>>,
}

impl EnginePlatformSnapshotService {
    #[inline]
    fn new(snapshot: Arc<Mutex<PlatformWindowReadyV1>>) -> Self {
        Self { snapshot }
    }

    #[inline]
    fn ok_json<T: serde::Serialize>(value: &T) -> RResult<Blob, RString> {
        match serde_json::to_vec(value) {
            Ok(bytes) => RResult::ROk(Blob::from(bytes)),
            Err(e) => RResult::RErr(RString::from(e.to_string())),
        }
    }

    #[inline]
    fn snapshot(&self) -> PlatformWindowReadyV1 {
        match self.snapshot.lock() {
            Ok(v) => *v,
            Err(e) => *e.into_inner(),
        }
    }
}

impl ServiceV1 for EnginePlatformSnapshotService {
    fn id(&self) -> CapabilityId {
        CapabilityId::from(ENGINE_PLATFORM_SERVICE_ID)
    }

    fn describe(&self) -> RString {
        let v = serde_json::json!({
            "id": ENGINE_PLATFORM_SERVICE_ID,
            "version": 1,
            "origin": "engine-owned",
            "owner": "newengine-runtime-host.platform-gateway",
            "capability": PLATFORM_BACKEND_CAPABILITY_ID,
            "methods": [
                PLATFORM_SERVICE_METHOD_INFO,
                PLATFORM_SERVICE_METHOD_INVOKE,
                PLATFORM_SERVICE_METHOD_SHUTDOWN_V1,
                PLATFORM_SERVICE_METHOD_WINDOW_SNAPSHOT_JSON_V1
            ],
            "notes": "Engine-facing platform gateway for native window handles and surface metrics."
        });
        RString::from(serde_json::to_string(&v).unwrap_or_else(|_| "{}".to_string()))
    }

    fn call(&self, method: MethodName, payload: Blob) -> RResult<Blob, RString> {
        match method.as_str() {
            PLATFORM_SERVICE_METHOD_INFO => Self::ok_json(&PlatformServiceInfo::default()),
            PLATFORM_SERVICE_METHOD_INVOKE => Self::ok_json(&serde_json::json!({
                "ok": false,
                "error": "engine.platform invoke_json has no generic command envelope yet",
                "payload_len": payload.as_slice().len()
            })),
            PLATFORM_SERVICE_METHOD_SHUTDOWN_V1 => Self::ok_json(&serde_json::json!({ "ok": true })),
            PLATFORM_SERVICE_METHOD_WINDOW_SNAPSHOT_JSON_V1 => Self::ok_json(&self.snapshot()),
            m => RResult::RErr(RString::from(format!(
                "engine.platform: unknown method '{}'",
                m
            ))),
        }
    }
}

pub(crate) fn register_platform_window_service_best_effort(initial: PlatformWindowReadyV1) {
    let snapshot = PLATFORM_WINDOW_SNAPSHOT
        .get_or_init(|| Arc::new(Mutex::new(initial)))
        .clone();

    match snapshot.lock() {
        Ok(mut guard) => *guard = initial,
        Err(e) => *e.into_inner() = initial,
    }

    if newengine_plugin_host::has_service(ENGINE_PLATFORM_SERVICE_ID) {
        return;
    }

    let svc = EnginePlatformSnapshotService::new(snapshot);
    let dyn_svc = ServiceV1_TO::from_value(svc, TD_Opaque);
    let host = newengine_plugin_host::default_host_api();

    match (host.register_service_v1)(dyn_svc) {
        RResult::ROk(()) => {
            match newengine_plugin_host::register_engine_owned_gateway(
                ENGINE_PLATFORM_SERVICE_ID,
                newengine_service_api::EngineServiceKind::Platform,
                ENGINE_PLATFORM_SERVICE_ID,
                PLATFORM_BACKEND_CAPABILITY_ID,
                0,
                "newengine-runtime-host.platform-gateway",
            ) {
                Ok(()) => log::info!(
                    "engine.platform gateway registered source=engine-owned service='{}' capability='{}'",
                    ENGINE_PLATFORM_SERVICE_ID,
                    PLATFORM_BACKEND_CAPABILITY_ID
                ),
                Err(e) => log::warn!(
                    "engine.platform gateway route registration skipped id='{}' err='{}'",
                    ENGINE_PLATFORM_SERVICE_ID,
                    e
                ),
            }
        }
        RResult::RErr(e) => {
            log::error!(
                "engine.platform service registration failed id='{}' err='{}'",
                ENGINE_PLATFORM_SERVICE_ID,
                e
            );
        }
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
