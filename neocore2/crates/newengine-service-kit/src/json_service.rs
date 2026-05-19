#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::Blob;
use serde::de::DeserializeOwned;
use serde::Serialize;

#[inline]
pub fn ok_json<T: Serialize>(value: T) -> RResult<Blob, RString> {
    match serde_json::to_vec(&value) {
        Ok(bytes) => RResult::ROk(Blob::from(bytes)),
        Err(e) => RResult::RErr(RString::from(e.to_string())),
    }
}

#[inline]
pub fn ok_empty_blob() -> RResult<Blob, RString> {
    RResult::ROk(Blob::from(Vec::<u8>::new()))
}

#[inline]
pub fn empty_payload_json() -> serde_json::Value {
    serde_json::json!({})
}

#[inline]
pub fn payload_json(payload: &Blob) -> Result<serde_json::Value, String> {
    if payload.is_empty() {
        return Ok(empty_payload_json());
    }
    serde_json::from_slice(payload.as_slice()).map_err(|e| e.to_string())
}

#[inline]
pub fn decode_json_payload<T: DeserializeOwned>(service_id: &str, method: &str, payload: &Blob) -> Result<T, RString> {
    serde_json::from_slice::<T>(payload.as_slice()).map_err(|e| {
        RString::from(format!(
            "{service_id}: invalid {method} payload: {e}"
        ))
    })
}
