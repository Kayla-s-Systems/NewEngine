use newengine_plugin_api::{ConfigPatchSourceV1, ConfigPatchV1};

pub(crate) fn config_patch_from_json_merge_patch(
    plugin_id: &str,
    name: &str,
    priority: i32,
    json: &serde_json::Value,
) -> ConfigPatchV1 {
    ConfigPatchV1 {
        source: ConfigPatchSourceV1::File,
        content_type: "application/merge-patch+json".into(),
        bytes: serde_json::to_vec(json).unwrap_or_default().into(),
        priority,
        name: format!("{plugin_id}:{name}").into(),
    }
}