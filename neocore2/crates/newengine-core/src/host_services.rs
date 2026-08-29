#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::{Blob, CapabilityId, MethodName};

use newengine_plugin_host::{
    call_service_v1 as host_call_service_v1, describe_service as host_describe_service,
    list_services as host_list_services,
};

#[derive(Clone)]
pub struct StableServiceCall {
    capability_id: CapabilityId,
    method: MethodName,
}

impl StableServiceCall {
    #[inline]
    pub fn new(capability_id: &str, method: &str) -> Self {
        Self {
            capability_id: RString::from(capability_id),
            method: RString::from(method),
        }
    }

    #[inline]
    pub fn call(&self, payload: &[u8]) -> Result<Vec<u8>, String> {
        match host_call_service_v1(
            self.capability_id.clone(),
            self.method.clone(),
            Blob::from(payload.to_vec()),
        ) {
            RResult::ROk(value) => Ok(value.into_vec()),
            RResult::RErr(error) => Err(error.to_string()),
        }
    }

    #[inline]
    pub fn call_optional(&self, payload: &[u8]) -> Result<Option<Vec<u8>>, String> {
        match self.call(payload) {
            Ok(value) => Ok(Some(value)),
            Err(error) if is_missing_service_error(&error) => Ok(None),
            Err(error) => Err(error),
        }
    }
}

#[inline]
fn is_missing_service_error(error: &str) -> bool {
    error.trim_start().starts_with("service not found:")
}

#[inline]
pub fn call_service_v1(
    capability_id: &str,
    method: &str,
    payload: &[u8],
) -> Result<Vec<u8>, String> {
    let cap: CapabilityId = RString::from(capability_id);
    let m: MethodName = RString::from(method);
    let blob: Blob = Blob::from(payload.to_vec());

    match host_call_service_v1(cap, m, blob) {
        RResult::ROk(v) => Ok(v.into_vec()),
        RResult::RErr(e) => Err(e.to_string()),
    }
}

pub fn call_service_v1_optional(
    capability_id: &str,
    method: &str,
    payload: &[u8],
) -> Result<Option<Vec<u8>>, String> {
    // Successful optional calls are the common runtime path. Avoid the old
    // `has_service()` preflight, which acquired the service registry before the
    // actual dispatch and doubled synchronization for input/UI polling.
    match call_service_v1(capability_id, method, payload) {
        Ok(value) => Ok(Some(value)),
        Err(error) if is_missing_service_error(&error) => Ok(None),
        Err(error) => Err(error),
    }
}

pub fn list_service_ids() -> Vec<String> {
    host_list_services()
}

#[inline]
pub fn describe_service(service_id: &str) -> Option<String> {
    host_describe_service(service_id)
}

/// Resolves a canonical `engine.*` gateway to the active provider service, if any.
///
/// Reusable runtime code should depend on this engine-runtime query instead of
/// naming concrete providers/backends or calling plugin-host internals directly.
pub fn resolve_service_for_engine_gateway(gateway_id: &str) -> Option<String> {
    newengine_plugin_host::resolve_service_for_engine_gateway(gateway_id)
}

/// Returns true when the active route for `gateway_id` declares `capability_id`.
///
/// This is the capability-as-option boundary: the engine owns the question, the
/// provider declares availability, and callers degrade according to their policy.
pub fn engine_gateway_has_capability(gateway_id: &str, capability_id: &str) -> bool {
    newengine_plugin_host::engine_gateway_has_capability(gateway_id, capability_id)
}

/// Returns true when an `engine.*` gateway currently has an active route.
pub fn has_engine_gateway_route(gateway_id: &str) -> bool {
    resolve_service_for_engine_gateway(gateway_id).is_some()
}
