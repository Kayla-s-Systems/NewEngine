#![forbid(unsafe_op_in_unsafe_fn)]

use crate::plugins::host_api;
use crate::startup::StartupLoggingConfig;

use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::{Blob, CapabilityId, MethodName, ServiceV1, ServiceV1Dyn};
use serde_json::json;
use std::sync::{Arc, OnceLock};

pub const STARTUP_CONFIG_SERVICE_ID: &str = "newengine.startup.config.v1";
pub const METHOD_GET_JSON: &str = "get_json";

struct StartupConfigService {
    logging: StartupLoggingConfig,
}

impl ServiceV1 for StartupConfigService {
    fn id(&self) -> CapabilityId {
        RString::from(STARTUP_CONFIG_SERVICE_ID)
    }

    fn describe(&self) -> RString {
        RString::from(
            json!({
                "id": STARTUP_CONFIG_SERVICE_ID,
                "version": 1,
                "methods": [
                    {"name": METHOD_GET_JSON, "payload": "utf8 key", "returns": "utf8 json"}
                ],
                "keys": [
                    {"name": "logging", "type": "StartupLoggingConfig"}
                ]
            })
                .to_string(),
        )
    }

    fn call(&self, method: MethodName, payload: Blob) -> RResult<Blob, RString> {
        match method.to_string().as_str() {
            METHOD_GET_JSON => {
                let key = String::from_utf8_lossy(payload.as_slice()).trim().to_owned();
                if key == "logging" {
                    // Keep it schema-stable and simple for plugins: a single JSON object.
                    let v = json!({
                        "filter": self.logging.filter,
                        "level": self.logging.level,
                        "style": self.logging.style,
                        "colors": self.logging.colors,
                        "include_module_path": self.logging.include_module_path,
                        "include_target": self.logging.include_target,
                        "include_file": self.logging.include_file,
                        "include_line_number": self.logging.include_line_number,
                        "timestamp": self.logging.timestamp,
                        "indent": self.logging.indent,
                        "console_target": self.logging.console_target,
                        "file_path": self.logging.file_path,
                        "tee": self.logging.tee,
                        "roll_max_bytes": self.logging.roll_max_bytes,
                        "roll_max_files": self.logging.roll_max_files,
                        "roll_keep_days": self.logging.roll_keep_days
                    });

                    let s = v.to_string();
                    return RResult::ROk(Blob::from(s.into_bytes()));
                }

                RResult::RErr(RString::from("unknown key"))
            }

            _ => RResult::RErr(RString::from("unknown method")),
        }
    }
}

static LOGGING_CFG: OnceLock<Arc<StartupLoggingConfig>> = OnceLock::new();

/// Registers a core service that exposes resolved startup configuration to plugins.
///
/// This is intentionally small and stable: currently only `logging` is exposed.
pub fn init_startup_config_service(logging: StartupLoggingConfig) {
    let cfg = LOGGING_CFG.get_or_init(|| Arc::new(logging)).clone();

    let svc = StartupConfigService {
        logging: (*cfg).clone(),
    };

    let dyn_svc = ServiceV1Dyn::from_value(svc, abi_stable::sabi_trait::TD_Opaque);
    let _ = host_api::host_register_service_impl(dyn_svc);
}
