#![forbid(unsafe_op_in_unsafe_fn)]

use crate::host_api;

use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::{Blob, CapabilityId, MethodName, ServiceV1, ServiceV1Dyn};
use serde_json::{json, Map, Value};
use newengine_math::collections_prelude::{NeBTreeSet as BTreeSet, NeHashMap as HashMap};
use std::env;
use std::sync::OnceLock;

pub const CONFIG_SERVICE_ID: &str = "newengine.config.v1";

const METHOD_GET_PLUGIN_JSON: &str = "get_plugin_json";
const ENV_PLUGIN_PREFIX: &str = "NEWENGINE_PLUGIN_";
const ENV_PATH_SEPARATOR: &str = "__";

#[inline]
fn empty_object() -> Value {
    Value::Object(Map::new())
}

#[inline]
fn ensure_object(value: &mut Value) -> &mut Map<String, Value> {
    if !value.is_object() {
        *value = empty_object();
    }

    value
        .as_object_mut()
        .expect("value must be an object after ensure_object")
}

fn merge_missing_fields(dst: &mut Value, src: &Value) {
    match (dst, src) {
        (Value::Object(dst_map), Value::Object(src_map)) => {
            for (key, src_value) in src_map {
                match dst_map.get_mut(key) {
                    Some(dst_value) => merge_missing_fields(dst_value, src_value),
                    None => {
                        dst_map.insert(key.clone(), src_value.clone());
                    }
                }
            }
        }
        (dst_value, src_value) if dst_value.is_null() => {
            *dst_value = src_value.clone();
        }
        _ => {}
    }
}

fn collect_override_ids(prefix: &str, value: &Value, out: &mut BTreeSet<String>) {
    collect_override_ids_inner(prefix, value, out, true);
}

fn collect_override_ids_inner(prefix: &str, value: &Value, out: &mut BTreeSet<String>, is_root: bool) {
    match value {
        Value::Object(map) => {
            let is_leaf_override =
                map.is_empty() || map.keys().any(|key| key.contains('.')) || map.values().any(|v| !v.is_object());

            if is_leaf_override {
                out.insert(prefix.to_owned());
                return;
            }

            if is_root && map.is_empty() {
                out.insert(prefix.to_owned());
                return;
            }

            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();

            for key in keys {
                if let Some(child) = map.get(key) {
                    let next = format!("{prefix}.{key}");
                    collect_override_ids_inner(&next, child, out, false);
                }
            }
        }
        _ => {
            out.insert(prefix.to_owned());
        }
    }
}

#[derive(Debug, Clone)]
struct PluginConfigStore {
    /// Raw `config.json.plugins` content keyed by top-level root or exact plugin id.
    overrides: HashMap<String, Value>,
}

impl PluginConfigStore {
    #[inline]
    fn new(overrides: HashMap<String, Value>) -> Self {
        Self { overrides }
    }

    fn resolve_plugin_overrides(&self, plugin_id: &str) -> Value {
        let mut resolved = self
            .overrides
            .get(plugin_id)
            .cloned()
            .unwrap_or_else(empty_object);

        if let Some(nested) = self.lookup_nested_override(plugin_id) {
            merge_missing_fields(&mut resolved, nested);
        }

        apply_env_overrides(plugin_id, &mut resolved);
        resolved
    }

    fn lookup_nested_override(&self, plugin_id: &str) -> Option<&Value> {
        let parts: Vec<&str> = plugin_id.split('.').collect();
        if parts.is_empty() {
            return None;
        }

        for split_at in (1..parts.len()).rev() {
            let prefix = parts[..split_at].join(".");
            let Some(value) = self.overrides.get(&prefix) else {
                continue;
            };
            if let Some(found) = lookup_path_flexible(value, &parts[split_at..]) {
                return Some(found);
            }
        }

        let (root, tail) = parts.split_first()?;
        let value = self.overrides.get(*root)?;
        lookup_path_flexible(value, tail)
    }
}

fn lookup_path_flexible<'a>(value: &'a Value, parts: &[&str]) -> Option<&'a Value> {
    if parts.is_empty() {
        return Some(value);
    }

    let object = value.as_object()?;

    for split_at in (1..=parts.len()).rev() {
        let candidate_key = parts[..split_at].join(".");
        let next = object.get(&candidate_key)?;

        if split_at == parts.len() {
            return Some(next);
        }

        if let Some(found) = lookup_path_flexible(next, &parts[split_at..]) {
            return Some(found);
        }
    }

    None
}

struct ConfigService {
    store: &'static PluginConfigStore,
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
                let plugin_id = String::from_utf8_lossy(payload.as_slice()).trim().to_owned();
                if plugin_id.is_empty() {
                    return RResult::RErr(RString::from("plugin_id is empty"));
                }

                let resolved = self.store.resolve_plugin_overrides(&plugin_id);

                match serde_json::to_vec(&resolved) {
                    Ok(bytes) => RResult::ROk(Blob::from(bytes)),
                    Err(error) => {
                        RResult::RErr(RString::from(format!(
                            "config json encode failed: {error}"
                        )))
                    }
                }
            }
            _ => RResult::RErr(RString::from("unknown method")),
        }
    }
}

static STORE: OnceLock<PluginConfigStore> = OnceLock::new();

#[inline]
pub fn get_plugin_overrides_with_env(plugin_id: &str) -> Value {
    STORE
        .get()
        .map(|store| store.resolve_plugin_overrides(plugin_id))
        .unwrap_or_else(empty_object)
}

/// Registers a core service that exposes per-plugin override objects.
///
/// Resolution order:
/// 1. exact flat plugin id: `plugins["newengine.logging"]`
/// 2. nested domain path: `plugins.newengine.logging`
/// 3. dotted leaf under a domain root: `plugins.newengine["platform.winit"]`
/// 4. environment overrides
pub fn init_plugin_config_service(overrides: HashMap<String, Value>) {
    if overrides.is_empty() {
        log::info!("config: no plugin overrides in startup config");
    } else {
        let mut ids = BTreeSet::new();
        for (root, value) in &overrides {
            collect_override_ids(root, value, &mut ids);
        }

        log::info!(
            "config: plugin overrides loaded (count={}): {}",
            ids.len(),
            ids.into_iter().collect::<Vec<_>>().join(", ")
        );
    }

    let store = STORE.get_or_init(|| PluginConfigStore::new(overrides));
    let service = ConfigService { store };
    let dyn_service = ServiceV1Dyn::from_value(service, abi_stable::sabi_trait::TD_Opaque);
    let _ = host_api::host_register_service_impl(dyn_service);
}

fn sanitize_plugin_id_for_env(plugin_id: &str) -> String {
    let mut out = String::with_capacity(plugin_id.len());
    for ch in plugin_id.chars() {
        let upper = ch.to_ascii_uppercase();
        if upper.is_ascii_alphanumeric() {
            out.push(upper);
        } else {
            out.push('_');
        }
    }
    out
}

fn parse_env_value(raw: &str) -> Value {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Value::String(String::new());
    }

    if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
        return parsed;
    }

    Value::String(trimmed.to_owned())
}

fn set_path(root: &mut Value, path: &[&str], value: Value) {
    if path.is_empty() {
        *root = value;
        return;
    }

    let mut current = root;

    for (index, segment) in path.iter().enumerate() {
        let is_last = index + 1 == path.len();

        if is_last {
            let object = ensure_object(current);
            object.insert((*segment).to_owned(), value);
            return;
        }

        let object = ensure_object(current);
        current = object
            .entry((*segment).to_owned())
            .or_insert_with(empty_object);
    }
}

fn apply_env_overrides(plugin_id: &str, root: &mut Value) {
    let sanitized_id = sanitize_plugin_id_for_env(plugin_id);
    let prefix = format!("{ENV_PLUGIN_PREFIX}{sanitized_id}{ENV_PATH_SEPARATOR}");

    for (key, raw_value) in env::vars() {
        if !key.starts_with(&prefix) {
            continue;
        }

        let suffix = &key[prefix.len()..];
        let path: Vec<&str> = suffix
            .split(ENV_PATH_SEPARATOR)
            .map(str::trim)
            .filter(|segment| !segment.is_empty())
            .collect();

        if path.is_empty() {
            continue;
        }

        let parsed_value = parse_env_value(&raw_value);

        log::debug!(
            "config: env override plugin='{}' key='{}' value='{}'",
            plugin_id,
            suffix,
            summarize_value_for_log(&parsed_value)
        );

        set_path(root, &path, parsed_value);
    }
}

const LOG_VALUE_MAX_BYTES: usize = 160;
const LOG_TRUNCATION_SUFFIX: &str = "...";

fn truncate_for_log(input: &str) -> String {
    if input.len() <= LOG_VALUE_MAX_BYTES {
        return input.to_owned();
    }

    let target_len = LOG_VALUE_MAX_BYTES.saturating_sub(LOG_TRUNCATION_SUFFIX.len());
    let mut end = target_len.min(input.len());
    while end > 0 && !input.is_char_boundary(end) {
        end -= 1;
    }

    let mut out = String::with_capacity(end + LOG_TRUNCATION_SUFFIX.len());
    out.push_str(&input[..end]);
    out.push_str(LOG_TRUNCATION_SUFFIX);
    out
}

fn summarize_value_for_log(value: &Value) -> String {
    match value {
        Value::Null => "null".to_owned(),
        Value::Bool(v) => v.to_string(),
        Value::Number(v) => v.to_string(),
        Value::String(v) => truncate_for_log(v),
        _ => match serde_json::to_string(value) {
            Ok(serialized) => truncate_for_log(&serialized),
            Err(_) => "<unprintable>".to_owned(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_store(overrides: HashMap<String, Value>) -> PluginConfigStore {
        PluginConfigStore::new(overrides)
    }

    #[test]
    fn log_summary_truncates_string_values_safely() {
        let long_value = "x".repeat(LOG_VALUE_MAX_BYTES + 32);
        let summarized = summarize_value_for_log(&json!(long_value));

        assert!(summarized.ends_with(LOG_TRUNCATION_SUFFIX));
        assert_eq!(summarized.len(), LOG_VALUE_MAX_BYTES);
    }

    #[test]
    fn log_summary_truncates_multibyte_values_on_char_boundary() {
        let long_value = "Ж".repeat(LOG_VALUE_MAX_BYTES);
        let summarized = summarize_value_for_log(&json!(long_value));

        assert!(summarized.ends_with(LOG_TRUNCATION_SUFFIX));
        assert!(summarized.len() <= LOG_VALUE_MAX_BYTES);
        assert!(summarized.is_char_boundary(summarized.len()));
    }

    #[test]
    fn resolves_exact_flat_plugin_override() {
        let mut overrides = HashMap::default();
        overrides.insert(
            "newengine.logging".to_owned(),
            json!({
                "timestamp": "millis",
                "format": { "preset": "aaa" }
            }),
        );

        let store = make_store(overrides);
        let got = store.resolve_plugin_overrides("newengine.logging");

        assert_eq!(got["timestamp"], json!("millis"));
        assert_eq!(got["format"]["preset"], json!("aaa"));
    }

    #[test]
    fn resolves_nested_domain_wrapped_plugin_override() {
        let mut overrides = HashMap::default();
        overrides.insert(
            "newengine".to_owned(),
            json!({
                "platform": {
                    "winit": {
                        "title": "NewEngine Editor",
                        "placement": {
                            "mode": "centered",
                            "x": 0,
                            "y": -24
                        }
                    }
                }
            }),
        );

        let store = make_store(overrides);
        let got = store.resolve_plugin_overrides("newengine.platform.winit");

        assert_eq!(got["title"], json!("NewEngine Editor"));
        assert_eq!(got["placement"]["mode"], json!("centered"));
        assert_eq!(got["placement"]["y"], json!(-24));
    }

    #[test]
    fn exact_flat_override_wins_and_nested_fills_missing_keys() {
        let mut overrides = HashMap::default();

        overrides.insert(
            "newengine".to_owned(),
            json!({
                "logging": {
                    "timestamp": "millis",
                    "sources": {
                        "console": {
                            "enabled": true,
                            "level": "info"
                        },
                        "file": {
                            "enabled": true,
                            "path": "logs/default.log"
                        }
                    }
                }
            }),
        );

        overrides.insert(
            "newengine.logging".to_owned(),
            json!({
                "sources": {
                    "file": {
                        "path": "logs/log.log",
                        "mode": "truncate"
                    }
                }
            }),
        );

        let store = make_store(overrides);
        let got = store.resolve_plugin_overrides("newengine.logging");

        assert_eq!(got["timestamp"], json!("millis"));
        assert_eq!(got["sources"]["console"]["enabled"], json!(true));
        assert_eq!(got["sources"]["console"]["level"], json!("info"));
        assert_eq!(got["sources"]["file"]["path"], json!("logs/log.log"));
        assert_eq!(got["sources"]["file"]["mode"], json!("truncate"));
        assert_eq!(got["sources"]["file"]["enabled"], json!(true));
    }

    #[test]
    fn resolves_nested_domain_wrapped_dotted_leaf_key() {
        let mut overrides = HashMap::default();

        overrides.insert(
            "newengine".to_owned(),
            json!({
                "platform.winit": {
                    "title": "Editor",
                    "width": 1600,
                    "height": 900,
                    "placement": {
                        "mode": "centered",
                        "x": 0,
                        "y": -24
                    },
                    "icon": "ui/engine.ico"
                }
            }),
        );

        let store = make_store(overrides);
        let got = store.resolve_plugin_overrides("newengine.platform.winit");

        assert_eq!(got["title"], json!("Editor"));
        assert_eq!(got["width"], json!(1600));
        assert_eq!(got["height"], json!(900));
        assert_eq!(got["placement"]["mode"], json!("centered"));
        assert_eq!(got["placement"]["x"], json!(0));
        assert_eq!(got["placement"]["y"], json!(-24));
        assert_eq!(got["icon"], json!("ui/engine.ico"));
    }

    #[test]
    fn resolves_grouped_exact_prefix_override_for_renderer_backend() {
        let mut overrides = HashMap::default();
        overrides.insert(
            "newengine.renderer".to_owned(),
            json!({
                "vulkan": {
                    "clear_color": [0.02, 0.025, 0.035, 1.0],
                    "debug_text": "NewEngine | Vulkan"
                }
            }),
        );

        let store = make_store(overrides);
        let got = store.resolve_plugin_overrides("newengine.renderer.vulkan");

        assert_eq!(got["clear_color"], json!([0.02, 0.025, 0.035, 1.0]));
        assert_eq!(got["debug_text"], json!("NewEngine | Vulkan"));
    }

    #[test]
    fn exact_renderer_backend_override_wins_over_group_prefix() {
        let mut overrides = HashMap::default();
        overrides.insert(
            "newengine.renderer".to_owned(),
            json!({
                "vulkan": {
                    "clear_color": [0.02, 0.025, 0.035, 1.0],
                    "debug_text": "Grouped"
                }
            }),
        );
        overrides.insert(
            "newengine.renderer.vulkan".to_owned(),
            json!({
                "debug_text": "Exact"
            }),
        );

        let store = make_store(overrides);
        let got = store.resolve_plugin_overrides("newengine.renderer.vulkan");

        assert_eq!(got["clear_color"], json!([0.02, 0.025, 0.035, 1.0]));
        assert_eq!(got["debug_text"], json!("Exact"));
    }

    #[test]
    fn returns_empty_object_when_override_is_missing() {
        let store = make_store(HashMap::default());
        let got = store.resolve_plugin_overrides("newengine.missing");

        assert_eq!(got, json!({}));
    }
}