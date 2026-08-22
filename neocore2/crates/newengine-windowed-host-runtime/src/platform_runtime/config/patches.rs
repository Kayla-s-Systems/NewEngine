use abi_stable::std_types::{RString, RVec};
use newengine_plugin_api::{ConfigDiagLevelV1, ConfigPatchSourceV1, ConfigPatchV1};
use serde_json::Value;

use crate::platform_runtime::constants::CT_JSON_MERGE_PATCH;

pub(super) fn config_patch_from_json_merge_patch(
    name: &str,
    priority: i32,
    value: &Value,
) -> ConfigPatchV1 {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    ConfigPatchV1 {
        source: ConfigPatchSourceV1::HostRule,
        content_type: RString::from(CT_JSON_MERGE_PATCH),
        bytes: RVec::from(bytes),
        priority,
        name: RString::from(name),
    }
}

#[inline]
pub(super) fn is_non_empty_object(value: &Value) -> bool {
    matches!(value, Value::Object(map) if !map.is_empty())
}
pub(super) fn log_platform_config_diags(
    plugin_id: &str,
    diags: &[newengine_plugin_api::ConfigDiagV1],
) {
    for diag in diags {
        match diag.level {
            ConfigDiagLevelV1::Info => newengine_ulog_api::ulog::info!(
                "platform runtime: config info id='{}' {} {}",
                plugin_id,
                diag.code,
                diag.message
            ),
            ConfigDiagLevelV1::Warn => newengine_ulog_api::ulog::warn!(
                "platform runtime: config warn id='{}' {} {}",
                plugin_id,
                diag.code,
                diag.message
            ),
            ConfigDiagLevelV1::Error => newengine_ulog_api::ulog::error!(
                "platform runtime: config error id='{}' {} {}",
                plugin_id,
                diag.code,
                diag.message
            ),
        }
    }
}

pub(super) fn extract_string_field(value: &Value, key: &str) -> Option<String> {
    value
        .as_object()
        .and_then(|obj| obj.get(key))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|it| !it.is_empty())
        .map(str::to_owned)
}

pub(super) fn strip_host_only_platform_keys(value: &Value) -> Value {
    let mut value = value.clone();
    if let Some(obj) = value.as_object_mut() {
        obj.remove("icon");
    }
    value
}

#[inline]
fn env_flag(key: &str) -> Option<bool> {
    std::env::var(key).ok().map(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

#[inline]
pub(crate) fn runtime_bootstrap_overlay_enabled() -> bool {
    !env_flag("NEWENGINE_RUNTIME_BOOTSTRAP_OVERLAY_DISABLED").unwrap_or(false)
}

#[inline]
pub(super) fn platform_metadata_probe_enabled() -> bool {
    env_flag("NEWENGINE_PLATFORM_CONFIG_METADATA_PROBE").unwrap_or(false)
}
