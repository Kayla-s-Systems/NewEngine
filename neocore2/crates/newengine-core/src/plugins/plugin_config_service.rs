#![forbid(unsafe_op_in_unsafe_fn)]

use crate::plugins::host_api;

use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::{Blob, CapabilityId, MethodName, ServiceV1, ServiceV1Dyn};
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::env;
use std::sync::{Arc, OnceLock};

pub const CONFIG_SERVICE_ID: &str = "newengine.config.v1";

const METHOD_GET_PLUGIN_JSON: &str = "get_plugin_json";

#[derive(Debug, Clone)]
struct PluginConfigStore {
    /// Raw overrides from the engine config file: config.json.plugins[plugin_id].
    overrides: HashMap<String, Value>,
}

impl PluginConfigStore {
    fn plugin_overrides_with_env(&self, plugin_id: &str) -> Value {
        let mut root = match self.overrides.get(plugin_id) {
            Some(v) => v.clone(),
            None => Value::Object(Map::new()),
        };

        apply_env_overrides(plugin_id, &mut root);
        root
    }
}

struct ConfigService {
    store: Arc<PluginConfigStore>,
}

impl ServiceV1 for ConfigService {
    fn id(&self) -> CapabilityId {
        CapabilityId::from(CONFIG_SERVICE_ID)
    }

    fn describe(&self) -> RString {
        RString::from(
            json!({
                "id": CONFIG_SERVICE_ID,
                "version": 1,
                "methods": [
                    {
                        "name": METHOD_GET_PLUGIN_JSON,
                        "payload": "utf8 plugin_id",
                        "returns": "utf8 json object"
                    }
                ],
                "env": {
                    "prefix": "NEWENGINE_PLUGIN_<SANITIZED_PLUGIN_ID>__",
                    "path_sep": "__",
                    "value": "json if parsable, otherwise string"
                }
            })
                .to_string(),
        )
    }

    fn call(&self, method: MethodName, payload: Blob) -> RResult<Blob, RString> {
        match method.as_str() {
            METHOD_GET_PLUGIN_JSON => {
                let plugin_id = String::from_utf8_lossy(payload.as_slice())
                    .trim()
                    .to_owned();
                if plugin_id.is_empty() {
                    return RResult::RErr(RString::from("plugin_id is empty"));
                }

                let v = self.store.plugin_overrides_with_env(&plugin_id);

                match serde_json::to_vec(&v) {
                    Ok(bytes) => RResult::ROk(Blob::from(bytes)),
                    Err(e) => {
                        RResult::RErr(RString::from(format!("config json encode failed: {e}")))
                    }
                }
            }
            _ => RResult::RErr(RString::from("unknown method")),
        }
    }
}

static STORE: OnceLock<Arc<PluginConfigStore>> = OnceLock::new();

#[inline]
pub fn get_plugin_overrides_with_env(plugin_id: &str) -> Value {
    STORE
        .get()
        .map(|s| s.plugin_overrides_with_env(plugin_id))
        .unwrap_or_else(|| Value::Object(Map::new()))
}

/// Registers a core service that exposes per-plugin overrides to plugins.
///
/// The service returns the *override object* for the requested plugin id.
/// Plugins are expected to merge these overrides into their own base configs.
///
/// Environment variables can override individual fields using the convention:
///
/// - Prefix: `NEWENGINE_PLUGIN_<SANITIZED_PLUGIN_ID>__`
/// - Nested object keys separated with `__`
///
/// Example:
/// - `NEWENGINE_PLUGIN_NEWENGINE_LOGGING__level=debug`
/// - `NEWENGINE_PLUGIN_NEWENGINE_ASSETS__assets_root="D:/Data/Assets"`
pub fn init_plugin_config_service(overrides: HashMap<String, Value>) {
    if !overrides.is_empty() {
        let mut ids: Vec<String> = overrides.keys().cloned().collect();
        ids.sort();
        log::info!(
            "config: plugin overrides loaded (count={}): {}",
            ids.len(),
            ids.join(", ")
        );
    } else {
        log::info!("config: no plugin overrides in startup config");
    }

    let store = STORE
        .get_or_init(|| Arc::new(PluginConfigStore { overrides }))
        .clone();

    let svc = ConfigService { store };
    let dyn_svc = ServiceV1Dyn::from_value(svc, abi_stable::sabi_trait::TD_Opaque);
    let _ = host_api::host_register_service_impl(dyn_svc);
}

fn sanitize_plugin_id_for_env(id: &str) -> String {
    let mut out = String::with_capacity(id.len());
    for ch in id.chars() {
        let up = ch.to_ascii_uppercase();
        if up.is_ascii_alphanumeric() {
            out.push(up);
        } else {
            out.push('_');
        }
    }
    out
}

fn parse_env_value(raw: &str) -> Value {
    let s = raw.trim();
    if s.is_empty() {
        return Value::String(String::new());
    }

    // First: attempt JSON parse (numbers, bool, null, arrays/objects, quoted strings).
    if let Ok(v) = serde_json::from_str::<Value>(s) {
        return v;
    }

    // Fallback: plain string.
    Value::String(s.to_owned())
}

fn set_path(root: &mut Value, path: &[&str], value: Value) {
    if path.is_empty() {
        *root = value;
        return;
    }

    let mut cur = root;
    for (i, key) in path.iter().enumerate() {
        let last = i + 1 == path.len();

        if last {
            if !cur.is_object() {
                *cur = Value::Object(Map::new());
            }
            let Some(obj) = cur.as_object_mut() else { return; };
            obj.insert((*key).to_owned(), value);
            return;
        }

        if !cur.is_object() {
            *cur = Value::Object(Map::new());
        }
        let Some(obj) = cur.as_object_mut() else { return; };
        cur = obj
            .entry((*key).to_owned())
            .or_insert_with(|| Value::Object(Map::new()));
    }
}

fn apply_env_overrides(plugin_id: &str, root: &mut Value) {
    let pid = sanitize_plugin_id_for_env(plugin_id);
    let prefix = format!("NEWENGINE_PLUGIN_{pid}__");

    for (k, v) in env::vars() {
        if !k.starts_with(&prefix) {
            continue;
        }

        let rest = &k[prefix.len()..];
        let path: Vec<&str> = rest
            .split("__")
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        if path.is_empty() {
            continue;
        }

        let val = parse_env_value(&v);
        log::debug!(
            "config: env override plugin='{}' key='{}' value='{}'",
            plugin_id,
            rest,
            summarize_value_for_log(&val)
        );
        set_path(root, &path, val);
    }
}

fn summarize_value_for_log(v: &Value) -> String {
    const MAX: usize = 160;
    match v {
        Value::Null => "null".to_owned(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => {
            if s.len() <= MAX {
                s.clone()
            } else {
                let mut out = s[..MAX].to_owned();
                out.push_str("…");
                out
            }
        }
        _ => match serde_json::to_string(v) {
            Ok(s) if s.len() <= MAX => s,
            Ok(s) => {
                let mut out = s;
                out.truncate(MAX);
                out.push_str("…");
                out
            }
            Err(_) => "<unprintable>".to_owned(),
        },
    }
}
