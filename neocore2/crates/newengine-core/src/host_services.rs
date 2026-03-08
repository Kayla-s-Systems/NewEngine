#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::{Blob, CapabilityId, MethodName};

use newengine_plugin_host::{call_service_v1 as host_call_service_v1, describe_service as host_describe_service, has_service as host_has_service, list_services as host_list_services};

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
    // Fast path: avoid any allocation if missing.
    if !host_has_service(capability_id) {
        return Ok(None);
    }
    call_service_v1(capability_id, method, payload).map(Some)
}

pub fn list_service_ids() -> Vec<String> {
    host_list_services()
}

#[inline]
pub fn describe_service(service_id: &str) -> Option<String> {
    host_describe_service(service_id)
}
