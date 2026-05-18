use std::sync::{Arc, Mutex, OnceLock};

use abi_stable::erased_types::TD_Opaque;
use abi_stable::std_types::{RResult, RString};
use newengine_platform_api::{
    PlatformWindowReadyV1, PLATFORM_WINDOW_BACKEND_CAPABILITY_ID, PLATFORM_WINDOW_SERVICE_ID,
    PLATFORM_WINDOW_SERVICE_METHOD_SNAPSHOT_JSON_V1,
};
use newengine_plugin_api::{
    Blob, CapabilityId, MethodName, ServiceV1, ServiceV1_TO,
};

static PLATFORM_WINDOW_SNAPSHOT: OnceLock<Arc<Mutex<PlatformWindowReadyV1>>> = OnceLock::new();

#[derive(Clone)]
struct PlatformWindowSnapshotService {
    snapshot: Arc<Mutex<PlatformWindowReadyV1>>,
}

impl PlatformWindowSnapshotService {
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
}

impl ServiceV1 for PlatformWindowSnapshotService {
    fn id(&self) -> CapabilityId {
        CapabilityId::from(PLATFORM_WINDOW_SERVICE_ID)
    }

    fn describe(&self) -> RString {
        let v = serde_json::json!({
            "id": PLATFORM_WINDOW_SERVICE_ID,
            "version": 1,
            "origin": "host-owned",
            "owner": "newengine-runtime-host.platform-window",
            "capability": PLATFORM_WINDOW_BACKEND_CAPABILITY_ID,
            "methods": [PLATFORM_WINDOW_SERVICE_METHOD_SNAPSHOT_JSON_V1],
            "notes": "Host-provided platform window snapshot service for runtime plugins."
        });
        RString::from(serde_json::to_string(&v).unwrap_or_else(|_| "{}".to_string()))
    }

    fn call(&self, method: MethodName, _payload: Blob) -> RResult<Blob, RString> {
        match method.as_str() {
            PLATFORM_WINDOW_SERVICE_METHOD_SNAPSHOT_JSON_V1 => {
                let snapshot = match self.snapshot.lock() {
                    Ok(v) => *v,
                    Err(e) => *e.into_inner(),
                };
                Self::ok_json(&snapshot)
            }
            m => RResult::RErr(RString::from(format!(
                "platform window: unknown method '{}'",
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

    if newengine_plugin_host::has_service(PLATFORM_WINDOW_SERVICE_ID) {
        return;
    }

    let svc = PlatformWindowSnapshotService::new(snapshot);
    let dyn_svc = ServiceV1_TO::from_value(svc, TD_Opaque);
    let host = newengine_plugin_host::default_host_api();

    match (host.register_service_v1)(dyn_svc) {
        RResult::ROk(()) => {
            log::info!(
                "platform runtime: host snapshot service registered id='{}'",
                PLATFORM_WINDOW_SERVICE_ID
            );
        }
        RResult::RErr(e) => {
            log::error!(
                "platform runtime: host snapshot service registration failed id='{}' err='{}'",
                PLATFORM_WINDOW_SERVICE_ID,
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