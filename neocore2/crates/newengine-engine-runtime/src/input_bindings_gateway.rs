#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::sabi_trait::TD_Opaque;
use abi_stable::std_types::{RResult, RString};
use newengine_input_bindings::{
    InputBindingsProfile, InputBindingsServiceInfo, ENGINE_INPUT_BINDINGS_SERVICE_ID,
    INPUT_BINDINGS_BACKEND_CAPABILITY_ID, INPUT_BINDINGS_METHOD_INFO,
    INPUT_BINDINGS_METHOD_INVOKE, INPUT_BINDINGS_METHOD_PROFILE_JSON_V1,
    INPUT_BINDINGS_METHOD_SHUTDOWN_V1,
};
use newengine_plugin_api::{Blob, CapabilityId, MethodName, ServiceV1, ServiceV1Dyn};
use parking_lot::Mutex;
use std::sync::{Arc, OnceLock};

static INPUT_BINDINGS_GATEWAY: OnceLock<Arc<Mutex<InputBindingsGatewayState>>> = OnceLock::new();

#[derive(Clone, Debug)]
struct InputBindingsGatewayState {
    profile: InputBindingsProfile,
}

impl Default for InputBindingsGatewayState {
    #[inline]
    fn default() -> Self {
        Self { profile: InputBindingsProfile::gameplay_default() }
    }
}

#[derive(Clone)]
struct InputBindingsGatewayService {
    state: Arc<Mutex<InputBindingsGatewayState>>,
}

impl ServiceV1 for InputBindingsGatewayService {
    fn id(&self) -> CapabilityId {
        CapabilityId::from(ENGINE_INPUT_BINDINGS_SERVICE_ID)
    }

    fn describe(&self) -> RString {
        let info = InputBindingsServiceInfo::default();
        let json = serde_json::json!({
            "id": ENGINE_INPUT_BINDINGS_SERVICE_ID,
            "version": 1,
            "protocol": info.protocol,
            "features": info.features,
            "methods": info.methods,
            "origin": "engine-owned",
            "owner": "newengine-engine-runtime.input-bindings-gateway",
            "capability": INPUT_BINDINGS_BACKEND_CAPABILITY_ID,
            "gateway": "engine-owned engine.input.bindings profile service"
        });
        RString::from(json.to_string())
    }

    fn call(&self, method: MethodName, payload: Blob) -> RResult<Blob, RString> {
        match method.as_str() {
            INPUT_BINDINGS_METHOD_INFO => ok_json(&InputBindingsServiceInfo::default()),
            INPUT_BINDINGS_METHOD_PROFILE_JSON_V1 => {
                let profile = self.state.lock().profile.clone();
                ok_json(&profile)
            }
            INPUT_BINDINGS_METHOD_INVOKE => {
                let bytes = payload.as_slice();
                if bytes.is_empty() {
                    let profile = self.state.lock().profile.clone();
                    return ok_json(&profile);
                }
                match serde_json::from_slice::<InputBindingsProfile>(&bytes) {
                    Ok(profile) => {
                        self.state.lock().profile = profile.clone();
                        ok_json(&profile)
                    }
                    Err(e) => RResult::RErr(RString::from(format!(
                        "engine.input.bindings: invalid profile payload: {}",
                        e
                    ))),
                }
            }
            INPUT_BINDINGS_METHOD_SHUTDOWN_V1 => RResult::ROk(Blob::from(Vec::<u8>::new())),
            other => RResult::RErr(RString::from(format!(
                "engine.input.bindings: unknown method '{}'",
                other
            ))),
        }
    }
}

pub fn register_input_bindings_gateway_best_effort() {
    if newengine_plugin_host::has_service(ENGINE_INPUT_BINDINGS_SERVICE_ID) {
        return;
    }
    let state = Arc::clone(INPUT_BINDINGS_GATEWAY.get_or_init(|| Arc::new(Mutex::new(InputBindingsGatewayState::default()))));
    let dyn_svc = ServiceV1Dyn::from_value(InputBindingsGatewayService { state }, TD_Opaque);
    match newengine_plugin_host::host_register_service_impl(dyn_svc) {
        RResult::ROk(()) => {
            match newengine_plugin_host::register_engine_owned_gateway(
                ENGINE_INPUT_BINDINGS_SERVICE_ID,
                newengine_service_api::EngineServiceKind::InputBindings,
                ENGINE_INPUT_BINDINGS_SERVICE_ID,
                INPUT_BINDINGS_BACKEND_CAPABILITY_ID,
                0,
                "newengine-engine-runtime.input-bindings-gateway",
            ) {
                Ok(()) => log::info!(
                    "input bindings gateway: engine-owned route registered id='{}' capability='{}'",
                    ENGINE_INPUT_BINDINGS_SERVICE_ID,
                    INPUT_BINDINGS_BACKEND_CAPABILITY_ID
                ),
                Err(e) => log::warn!(
                    "input bindings gateway: route registration skipped id='{}' err='{}'",
                    ENGINE_INPUT_BINDINGS_SERVICE_ID,
                    e
                ),
            }
        }
        RResult::RErr(e) => log::warn!(
            "input bindings gateway: host service registration skipped id='{}' err='{}'",
            ENGINE_INPUT_BINDINGS_SERVICE_ID,
            e
        ),
    }
}

#[inline]
fn ok_json<T: serde::Serialize>(value: &T) -> RResult<Blob, RString> {
    match serde_json::to_vec(value) {
        Ok(bytes) => RResult::ROk(Blob::from(bytes)),
        Err(e) => RResult::RErr(RString::from(e.to_string())),
    }
}
