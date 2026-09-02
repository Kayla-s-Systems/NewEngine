#![forbid(unsafe_op_in_unsafe_fn)]

use crate::host_api;

use abi_stable::std_types::{RResult, RString};
use newengine_math::collections_prelude::{NeBTreeSet as BTreeSet, NeHashMap as HashMap};
use newengine_plugin_api::{Blob, CapabilityId, MethodName, ServiceV1, ServiceV1Dyn};
use serde_json::{json, Map, Value};
#[cfg(test)]
use std::env;
use std::sync::{Arc, Mutex};

pub const CONFIG_SERVICE_ID: &str = "newengine.config.v1";

const METHOD_GET_PLUGIN_JSON: &str = "get_plugin_json";
const ENV_PLUGIN_PREFIX: &str = "NEWENGINE_PLUGIN_";
const ENV_PATH_SEPARATOR: &str = "__";
const RESOLVED_CACHE_MAX_ENTRIES: usize = 256;

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

fn collect_override_ids_inner(
    prefix: &str,
    value: &Value,
    out: &mut BTreeSet<String>,
    is_root: bool,
) {
    match value {
        Value::Object(map) => {
            let is_leaf_override = map.is_empty()
                || map.keys().any(|key| key.contains('.'))
                || map.values().any(|v| !v.is_object());

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

#[derive(Debug)]
pub(crate) struct PluginConfigStore {
    /// Raw `config.json.plugins` content keyed by top-level root or exact plugin id.
    overrides: HashMap<String, Value>,
    /// Environment snapshot captured for this Engine instance.
    environment: HashMap<String, String>,
    /// Immutable store results are reused across discovery, selection and load.
    resolved_cache: Mutex<HashMap<String, Value>>,
}

impl PluginConfigStore {
    #[cfg(test)]
    #[inline]
    fn new(overrides: HashMap<String, Value>) -> Self {
        Self::new_with_environment(overrides, env::vars().collect())
    }

    fn new_with_environment(
        overrides: HashMap<String, Value>,
        environment: HashMap<String, String>,
    ) -> Self {
        Self {
            overrides,
            environment,
            resolved_cache: Mutex::new(HashMap::default()),
        }
    }

    fn resolve_plugin_overrides(&self, plugin_id: &str) -> Value {
        {
            let cache = match self.resolved_cache.lock() {
                Ok(cache) => cache,
                Err(poisoned) => poisoned.into_inner(),
            };
            if let Some(resolved) = cache.get(plugin_id) {
                return resolved.clone();
            }
        }

        let mut resolved = self
            .overrides
            .get(plugin_id)
            .cloned()
            .unwrap_or_else(empty_object);

        if let Some(nested) = self.lookup_nested_override(plugin_id) {
            merge_missing_fields(&mut resolved, nested);
        }

        apply_env_overrides_from(&self.environment, plugin_id, &mut resolved);

        let mut cache = match self.resolved_cache.lock() {
            Ok(cache) => cache,
            Err(poisoned) => poisoned.into_inner(),
        };
        if cache.len() < RESOLVED_CACHE_MAX_ENTRIES {
            cache
                .entry(plugin_id.to_owned())
                .or_insert_with(|| resolved.clone());
        }
        resolved
    }

    fn lookup_nested_override(&self, plugin_id: &str) -> Option<&Value> {
        let parts: Vec<&str> = plugin_id.split('.').collect();
        if parts.is_empty() {
            return None;
        }

        if let Some(found) = self.lookup_nested_override_parts(&parts) {
            return Some(found);
        }

        // Public engine-facing plugin ids use the `engine.*` gateway namespace,
        // while profile/config files can group host-owned defaults under the
        // historical `newengine.*` root. Keep that alias explicit here instead
        // of forcing every runtime provider to duplicate both key families.
        if parts.first() == Some(&"engine") {
            let mut aliased = Vec::with_capacity(parts.len());
            aliased.push("newengine");
            aliased.extend_from_slice(&parts[1..]);
            if let Some(found) = self.lookup_nested_override_parts(&aliased) {
                return Some(found);
            }
        }

        None
    }

    fn lookup_nested_override_parts(&self, parts: &[&str]) -> Option<&Value> {
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

fn value_is_leaf_override(value: &Value) -> bool {
    match value {
        Value::Object(map) => {
            map.is_empty()
                || map.keys().any(|key| key.contains('.'))
                || map.values().any(|v| !v.is_object())
        }
        _ => true,
    }
}

fn lookup_path_flexible<'a>(value: &'a Value, parts: &[&str]) -> Option<&'a Value> {
    if parts.is_empty() {
        return Some(value);
    }

    let object = value.as_object()?;

    for split_at in (1..=parts.len()).rev() {
        let candidate_key = parts[..split_at].join(".");
        let Some(next) = object.get(&candidate_key) else {
            continue;
        };

        if split_at == parts.len() {
            return Some(next);
        }

        if let Some(found) = lookup_path_flexible(next, &parts[split_at..]) {
            return Some(found);
        }

        // `plugins.newengine.logging` is a grouped override for concrete
        // providers such as `engine.logging.chronicle`. If the next object is
        // already a leaf override block, treat it as the best prefix match.
        if value_is_leaf_override(next) {
            return Some(next);
        }
    }

    None
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

                let resolved = self.store.resolve_plugin_overrides(&plugin_id);

                match serde_json::to_vec(&resolved) {
                    Ok(bytes) => RResult::ROk(Blob::from(bytes)),
                    Err(error) => {
                        RResult::RErr(RString::from(format!("config json encode failed: {error}")))
                    }
                }
            }
            _ => RResult::RErr(RString::from("unknown method")),
        }
    }
}

#[inline]
pub fn get_plugin_overrides_with_env(plugin_id: &str) -> Value {
    let context = crate::host_context::ctx();
    if let Ok(store) = context.plugin_config_store.lock() {
        if let Some(store) = store.as_ref() {
            return store.resolve_plugin_overrides(plugin_id);
        }
    }

    let environment = crate::host_context::environment_snapshot_utf8();
    let mut resolved = empty_object();
    apply_env_overrides_from(&environment, plugin_id, &mut resolved);
    resolved
}

#[inline]
pub fn plugin_enabled_by_config(plugin_id: &str) -> bool {
    let resolved = get_plugin_overrides_with_env(plugin_id);
    resolved
        .get("host")
        .and_then(|host| host.get("enabled"))
        .and_then(Value::as_bool)
        .or_else(|| resolved.get("enabled").and_then(Value::as_bool))
        .unwrap_or(true)
}

/// Registers an instance-owned config service. Each Engine universe receives its
/// own startup override and environment snapshot.
pub fn init_plugin_config_service(overrides: HashMap<String, Value>) {
    if overrides.is_empty() {
        newengine_ulog_api::ulog::info!("config: no plugin overrides in startup config");
    } else {
        let mut ids = BTreeSet::new();
        for (root, value) in &overrides {
            collect_override_ids(root, value, &mut ids);
        }
        newengine_ulog_api::ulog::info!(
            "config: plugin overrides loaded (count={}): {}",
            ids.len(),
            ids.into_iter().collect::<Vec<_>>().join(", ")
        );
    }

    let store = Arc::new(PluginConfigStore::new_with_environment(
        overrides,
        crate::host_context::environment_snapshot_utf8(),
    ));
    let context = crate::host_context::ctx();
    match context.plugin_config_store.lock() {
        Ok(mut slot) => *slot = Some(Arc::clone(&store)),
        Err(poisoned) => *poisoned.into_inner() = Some(Arc::clone(&store)),
    }
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

fn apply_env_overrides_from(
    environment: &HashMap<String, String>,
    plugin_id: &str,
    root: &mut Value,
) {
    let sanitized_id = sanitize_plugin_id_for_env(plugin_id);
    let prefix = format!("{ENV_PLUGIN_PREFIX}{sanitized_id}{ENV_PATH_SEPARATOR}");

    for (key, raw_value) in environment {
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

        let parsed_value = parse_env_value(raw_value);

        newengine_ulog_api::ulog::debug!(
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
    include!("plugin_config_service/tests.rs");
}
