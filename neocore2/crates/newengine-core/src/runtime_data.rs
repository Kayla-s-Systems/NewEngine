#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::{Path, PathBuf};

use crate::StartupConfig;

/// Runtime-authored files are declared by their owning provider/profile in
/// `config.json.plugins.<owner>.runtime_data`. Engine/runtime code keeps only
/// stable owner/key identifiers; physical and logical paths remain authored data.
pub const RUNTIME_DATA_OBJECT_KEY: &str = "runtime_data";

pub fn plugin_runtime_data_value<'a>(
    startup: &'a StartupConfig,
    owner: &str,
    key: &str,
) -> Result<&'a str, String> {
    let plugin = startup
        .plugins
        .get(owner)
        .ok_or_else(|| format!("runtime data owner '{owner}' is not configured"))?;
    let runtime_data = plugin
        .get(RUNTIME_DATA_OBJECT_KEY)
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| {
            format!(
                "runtime data owner '{owner}' must declare an object field '{RUNTIME_DATA_OBJECT_KEY}'"
            )
        })?;
    let value = runtime_data
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("runtime data owner '{owner}' has no non-empty key '{key}'"))?;
    Ok(value)
}

/// Resolve one configured runtime-data file. Relative paths are resolved under
/// the configured durable CONFIG root; absolute paths are accepted only when
/// explicitly authored in config. No source-tree or executable-relative probing
/// is performed here.
pub fn plugin_runtime_data_path(
    startup: &StartupConfig,
    owner: &str,
    key: &str,
) -> Result<PathBuf, String> {
    let authored = plugin_runtime_data_value(startup, owner, key)?;
    let path = Path::new(authored);
    Ok(if path.is_absolute() {
        path.to_path_buf()
    } else {
        startup.config_child(path)
    })
}

pub fn read_plugin_runtime_data_bytes(
    startup: &StartupConfig,
    owner: &str,
    key: &str,
) -> Result<(PathBuf, Vec<u8>), String> {
    let path = plugin_runtime_data_path(startup, owner, key)?;
    let bytes = std::fs::read(&path).map_err(|error| {
        format!(
            "runtime data read failed owner='{owner}' key='{key}' path='{}': {error}",
            path.display()
        )
    })?;
    Ok((path, bytes))
}

pub fn read_plugin_runtime_data_string(
    startup: &StartupConfig,
    owner: &str,
    key: &str,
) -> Result<(PathBuf, String), String> {
    let path = plugin_runtime_data_path(startup, owner, key)?;
    let text = std::fs::read_to_string(&path).map_err(|error| {
        format!(
            "runtime data read failed owner='{owner}' key='{key}' path='{}': {error}",
            path.display()
        )
    })?;
    Ok((path, text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn runtime_data_path_is_config_owned_not_source_tree_owned() {
        let mut startup = StartupConfig::default();
        startup.config = PathBuf::from("runtime-config-root");
        startup.plugins.insert(
            "example.provider".to_owned(),
            json!({"runtime_data": {"catalog": "domain/catalog.json"}}),
        );
        let path = plugin_runtime_data_path(&startup, "example.provider", "catalog").unwrap();
        assert!(path.ends_with(Path::new("runtime-config-root/domain/catalog.json")));
    }

    #[test]
    fn missing_runtime_data_is_an_explicit_error() {
        let startup = StartupConfig::default();
        let error = plugin_runtime_data_path(&startup, "missing.provider", "catalog").unwrap_err();
        assert!(error.contains("missing.provider"));
    }
}
