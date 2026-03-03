#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::{Blob, CapabilityId, MethodName};

use crate::plugins::host_api;
use crate::plugins::host_context;

#[inline]
pub fn call_service_v1(
    capability_id: &str,
    method: &str,
    payload: &[u8],
) -> Result<Vec<u8>, String> {
    let cap: CapabilityId = RString::from(capability_id);
    let m: MethodName = RString::from(method);
    let blob: Blob = Blob::from(payload.to_vec());

    match host_api::call_service_v1(cap, m, blob) {
        RResult::ROk(v) => Ok(v.into_vec()),
        RResult::RErr(e) => Err(e.to_string()),
    }
}

#[inline]
#[inline]
pub fn call_service_v1_optional(
    capability_id: &str,
    method: &str,
    payload: &[u8],
) -> Result<Option<Vec<u8>>, String> {
    // Fast path: check registry first (no allocation if missing).
    let c = host_context::ctx();
    let g = match c.services.lock() {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    if !g.contains_key(capability_id) {
        return Ok(None);
    }
    drop(g);
    call_service_v1(capability_id, method, payload).map(Some)
}

pub fn list_service_ids() -> Vec<String> {
    let c = host_context::ctx();
    let g = match c.services.lock() {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let mut out: Vec<String> = g.keys().cloned().collect();
    out.sort();
    out
}

#[inline]
pub fn describe_service(service_id: &str) -> Option<String> {
    let c = host_context::ctx();
    let g = c.services.lock().ok()?;
    let svc = g.get(service_id)?.clone();
    Some(svc.describe_json.to_string())
}
