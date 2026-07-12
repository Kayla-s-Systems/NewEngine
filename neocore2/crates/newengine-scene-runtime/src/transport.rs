use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::Blob;
use newengine_service_kit::{ok_json, payload_json};
use serde_json::Value;

pub(crate) fn parse_payload(payload: &Blob) -> Result<Value, String> {
    payload_json(payload)
}

pub(crate) fn json_result(result: Result<Value, String>) -> RResult<Blob, RString> {
    match result {
        Ok(value) => ok_json(value),
        Err(error) => RResult::RErr(RString::from(error)),
    }
}
