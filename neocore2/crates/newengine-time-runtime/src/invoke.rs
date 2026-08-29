use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::Blob;
use newengine_service_kit::{ok_json, payload_json};
use newengine_time_api::{time_method, TimeServiceInfoV1};

use crate::state::RuntimeHostedTimeState;

pub(crate) fn invoke(state: &mut RuntimeHostedTimeState, payload: Blob) -> RResult<Blob, RString> {
    let value = match payload_json(&payload) {
        Ok(value) => value,
        Err(error) => return RResult::RErr(RString::from(error)),
    };
    let method = value
        .get("method")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(time_method::SNAPSHOT_V1);

    match method {
        time_method::SNAPSHOT_V1 => ok_json(state.snapshot()),
        time_method::DESCRIBE_CLOCK_V1 => ok_json(TimeServiceInfoV1::default()),
        time_method::AI_CONTEXT_V1 => ok_json(state.ai_context()),
        other => RResult::RErr(RString::from(format!(
            "engine.time: unknown invoke method '{other}'"
        ))),
    }
}
