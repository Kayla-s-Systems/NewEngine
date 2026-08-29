use std::sync::OnceLock;

use newengine_console_api::{
    method, CommandExecResponse, CommandSuggestResponse, ENGINE_COMMAND_GATEWAY_ID,
};
use newengine_core::StableServiceCall;

static EXEC_CALL: OnceLock<StableServiceCall> = OnceLock::new();
static SUGGEST_CALL: OnceLock<StableServiceCall> = OnceLock::new();

#[inline]
fn exec_call() -> &'static StableServiceCall {
    EXEC_CALL.get_or_init(|| StableServiceCall::new(ENGINE_COMMAND_GATEWAY_ID, method::EXEC))
}

#[inline]
fn suggest_call() -> &'static StableServiceCall {
    SUGGEST_CALL.get_or_init(|| StableServiceCall::new(ENGINE_COMMAND_GATEWAY_ID, method::SUGGEST))
}

pub(super) fn execute(line: &str) -> Result<CommandExecResponse, String> {
    let bytes = exec_call()
        .call_optional(line.as_bytes())
        .map_err(|error| format!("engine.command exec failed: {error}"))?
        .ok_or_else(|| "engine.command route is unavailable".to_owned())?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("engine.command returned invalid exec response: {error}"))
}

pub(super) fn suggest(input: &str) -> Result<CommandSuggestResponse, String> {
    let bytes = suggest_call()
        .call_optional(input.as_bytes())
        .map_err(|error| format!("engine.command suggest failed: {error}"))?
        .ok_or_else(|| "engine.command route is unavailable".to_owned())?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("engine.command returned invalid suggestion response: {error}"))
}
