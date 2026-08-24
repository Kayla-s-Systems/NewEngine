#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::{RResult, RString};
use newengine_host_capabilities_api::{
    method, ENGINE_HOST_CAPABILITIES_GATEWAY_ID, HOST_CAPABILITIES_BACKEND_CAPABILITY_ID,
    HOST_CAPABILITIES_PROVIDER_ROUTE, HOST_CAPABILITIES_PROVIDER_SERVICE_ID,
    HOST_CAPABILITIES_RUNTIME_CONTRACT,
};
use newengine_plugin_api::{Blob, CapabilityId, MethodName, ServiceV1, ServiceV1Dyn};
use serde_json::json;

struct NativeHostCapabilitiesService;

impl ServiceV1 for NativeHostCapabilitiesService {
    fn id(&self) -> CapabilityId {
        RString::from(HOST_CAPABILITIES_PROVIDER_SERVICE_ID)
    }

    fn describe(&self) -> RString {
        RString::from(
            json!({
                "id": HOST_CAPABILITIES_PROVIDER_SERVICE_ID,
                "gateway": ENGINE_HOST_CAPABILITIES_GATEWAY_ID,
                "provider_route": HOST_CAPABILITIES_PROVIDER_ROUTE,
                "capability": HOST_CAPABILITIES_BACKEND_CAPABILITY_ID,
                "contract": HOST_CAPABILITIES_RUNTIME_CONTRACT,
                "methods": [{
                    "name": method::SNAPSHOT,
                    "payload": "empty",
                    "returns": "json HostPreInitSnapshot"
                }]
            })
            .to_string(),
        )
    }

    fn call(&self, method_name: MethodName, _payload: Blob) -> RResult<Blob, RString> {
        match method_name.to_string().as_str() {
            method::SNAPSHOT => match serde_json::to_vec(&super::discover_preinit_snapshot()) {
                Ok(bytes) => RResult::ROk(Blob::from(bytes)),
                Err(error) => RResult::RErr(RString::from(format!(
                    "HostPreInitSnapshot serialization failed: {error}"
                ))),
            },
            other => RResult::RErr(RString::from(format!(
                "engine.host.capabilities: unknown method '{other}'"
            ))),
        }
    }
}

/// Builds the native provider service without registering it into a Host.
pub fn native_host_capabilities_service() -> ServiceV1Dyn<'static> {
    ServiceV1Dyn::from_value(
        NativeHostCapabilitiesService,
        abi_stable::sabi_trait::TD_Opaque,
    )
}
