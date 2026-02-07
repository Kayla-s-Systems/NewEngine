#![forbid(unsafe_op_in_unsafe_fn)]

use super::method::{method, COMMAND_SERVICE_ID};
use super::runtime::ConsoleRuntime;
use super::types::SuggestResponse;

use crate::plugins::host_api;

use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::{Blob, CapabilityId, MethodName, ServiceV1, ServiceV1Dyn};
use serde_json::json;
use std::sync::{Arc, OnceLock};

struct CommandService {
    rt: Arc<ConsoleRuntime>,
}

impl ServiceV1 for CommandService {
    fn id(&self) -> CapabilityId {
        RString::from(COMMAND_SERVICE_ID)
    }

    fn describe(&self) -> RString {
        RString::from(
            json!({
                "id": COMMAND_SERVICE_ID,
                "version": 2,
                "methods": [
                    { "name": method::EXEC, "payload": "utf8 line", "returns": "json {ok, output?, error?}" },
                    { "name": method::COMPLETE, "payload": "utf8 prefix", "returns": "json {items:[string]}" },
                    { "name": method::SUGGEST, "payload": "utf8 input", "returns": "json SuggestResponse" },
                    { "name": method::REFRESH, "payload": "empty", "returns": "json {ok:true}" }
                ],
                "console": {
                    "commands": [
                        { "name": "help", "help": "List commands", "usage": "help" },
                        { "name": "services", "help": "List services", "usage": "services" },
                        { "name": "refresh", "help": "Refresh console commands", "usage": "refresh" },
                        { "name": "describe", "help": "Describe a service", "usage": "describe <service_id>" },
                        { "name": "call", "help": "Call a service method", "usage": "call <service_id> <method> [payload]" },
                        { "name": "quit", "help": "Exit engine", "usage": "quit" }
                    ]
                }
            })
                .to_string(),
        )
    }

    fn call(&self, method: MethodName, payload: Blob) -> RResult<Blob, RString> {
        match method.to_string().as_str() {
            method::EXEC => {
                let line = String::from_utf8_lossy(payload.as_slice());
                let out = self.rt.exec(&line);

                let resp = match out {
                    Ok(v) => json!({ "ok": true, "output": v }),
                    Err(e) => json!({ "ok": false, "error": e }),
                };

                RResult::ROk(Blob::from(resp.to_string().into_bytes()))
            }

            method::COMPLETE => {
                let p = String::from_utf8_lossy(payload.as_slice());
                let v = self.rt.complete(&p);
                RResult::ROk(Blob::from(json!({ "items": v }).to_string().into_bytes()))
            }

            method::SUGGEST => {
                let p = String::from_utf8_lossy(payload.as_slice());
                let r: SuggestResponse = self.rt.suggest(&p);
                let bytes = serde_json::to_vec(&r).unwrap_or_default();
                RResult::ROk(Blob::from(bytes))
            }

            method::REFRESH => {
                self.rt.refresh_dyn_commands();
                RResult::ROk(Blob::from(json!({ "ok": true }).to_string().into_bytes()))
            }

            _ => RResult::RErr(RString::from("unknown method")),
        }
    }
}

static RT: OnceLock<Arc<ConsoleRuntime>> = OnceLock::new();

pub fn init_console_service() {
    let rt = RT.get_or_init(|| Arc::new(ConsoleRuntime::new())).clone();

    // Prebuild caches once at boot.
    rt.refresh_dyn_commands();

    let svc = CommandService { rt };
    let dyn_svc = ServiceV1Dyn::from_value(svc, abi_stable::sabi_trait::TD_Opaque);

    let _ = host_api::host_register_service_impl(dyn_svc, false);
}

pub fn take_exit_requested() -> bool {
    RT.get().map(|r| r.take_exit_requested()).unwrap_or(false)
}