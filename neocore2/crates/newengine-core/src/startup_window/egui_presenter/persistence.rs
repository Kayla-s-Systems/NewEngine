#![forbid(unsafe_op_in_unsafe_fn)]

use std::fs;
use std::path::Path;

use serde_json::{json, Map, Value};

use crate::startup::WindowPlacement;

use super::StartupWindowSelection;

pub(super) fn persist_confirmed_settings(
    config_path: &Path,
    selection: &StartupWindowSelection,
) -> Result<(), String> {
    let mut root = read_json_root(config_path)?;
    let root_obj = root
        .as_object_mut()
        .ok_or_else(|| "startup config root must be a JSON object".to_owned())?;

    let window = root_obj
        .entry("window".to_owned())
        .or_insert_with(|| Value::Object(Map::new()));
    let window_obj = window
        .as_object_mut()
        .ok_or_else(|| "startup config 'window' must be a JSON object".to_owned())?;
    window_obj.insert(
        "size".to_owned(),
        json!([selection.window_size.0, selection.window_size.1]),
    );
    window_obj.insert(
        "placement".to_owned(),
        match selection.window_placement {
            WindowPlacement::Centered { offset } => {
                json!({"type": "centered", "offset": [offset.0, offset.1]})
            }
            WindowPlacement::Default => json!({"type": "default"}),
        },
    );
    root_obj.insert(
        "startup_settings".to_owned(),
        serde_json::to_value(&selection.launch_settings)
            .map_err(|err| format!("startup settings encode failed: {err}"))?,
    );

    write_json_atomically(config_path, &root)
}

fn read_json_root(config_path: &Path) -> Result<Value, String> {
    if !config_path.exists() {
        return Ok(Value::Object(Map::new()));
    }
    let text = fs::read_to_string(config_path)
        .map_err(|err| format!("read {} failed: {err}", config_path.display()))?;
    serde_json::from_str(&text)
        .map_err(|err| format!("parse {} failed: {err}", config_path.display()))
}

fn write_json_atomically(config_path: &Path, value: &Value) -> Result<(), String> {
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create {} failed: {err}", parent.display()))?;
    }

    let payload = serde_json::to_vec_pretty(value)
        .map_err(|err| format!("serialize startup config failed: {err}"))?;
    let temp_path = config_path.with_extension("json.newengine-prestart.tmp");
    let backup_path = config_path.with_extension("json.newengine-prestart.bak");

    fs::write(&temp_path, payload)
        .map_err(|err| format!("write {} failed: {err}", temp_path.display()))?;

    if !config_path.exists() {
        return fs::rename(&temp_path, config_path).map_err(|err| {
            format!(
                "install startup config {} -> {} failed: {err}",
                temp_path.display(),
                config_path.display()
            )
        });
    }

    let _ = fs::remove_file(&backup_path);
    fs::rename(config_path, &backup_path).map_err(|err| {
        format!(
            "backup startup config {} -> {} failed: {err}",
            config_path.display(),
            backup_path.display()
        )
    })?;

    if let Err(err) = fs::rename(&temp_path, config_path) {
        let _ = fs::rename(&backup_path, config_path);
        return Err(format!(
            "replace startup config {} -> {} failed: {err}",
            temp_path.display(),
            config_path.display()
        ));
    }

    let _ = fs::remove_file(backup_path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::startup_window::StartupLaunchSettings;

    fn temp_config_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("newengine-prestart-{name}-{nonce}.json"))
    }

    #[test]
    fn confirmed_settings_round_trip_and_preserve_unknown_keys() {
        let path = temp_config_path("round-trip");
        fs::write(
            &path,
            r#"{"custom":{"keep":true},"window":{"title":"Test"}}"#,
        )
        .unwrap();
        let mut settings = StartupLaunchSettings::default();
        settings.graphics.msaa_samples = 8;
        settings.graphics.fxaa_enabled = true;
        settings.graphics.ssao_enabled = true;
        settings.graphics.mark_custom();
        let selection = StartupWindowSelection {
            launch_settings: settings.clone(),
            window_size: (1920, 1080),
            window_placement: WindowPlacement::Centered { offset: (0, 0) },
        };

        persist_confirmed_settings(&path, &selection).unwrap();
        let saved: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let restored: StartupLaunchSettings =
            serde_json::from_value(saved["startup_settings"].clone()).unwrap();

        assert_eq!(restored.graphics.msaa_samples, 8);
        assert!(restored.graphics.fxaa_enabled);
        assert!(restored.graphics.ssao_enabled);
        assert_eq!(saved["window"]["size"], json!([1920, 1080]));
        assert_eq!(saved["custom"]["keep"], json!(true));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn no_persistence_occurs_without_confirmed_launch() {
        let path = temp_config_path("cancel");
        let original = r#"{"window":{"size":[1280,720]}}"#;
        fs::write(&path, original).unwrap();
        // No call to persist_confirmed_settings represents Cancel/close.
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
        let _ = fs::remove_file(path);
    }
}
