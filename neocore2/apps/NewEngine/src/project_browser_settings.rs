use serde_json::{Map, Value};

/// Prepares the raw startup configuration document for the Project Browser.
///
/// The browser works on semantic player-facing settings under
/// `startup_settings.display` and `startup_settings.graphics`, but the launcher
/// must preserve every unknown/future field in the original JSON document.
pub fn prepare_project_browser_config_document(mut document: Value) -> Result<Value, String> {
    if !document.is_object() {
        return Err("Project Browser startup config document must be a JSON object".to_owned());
    }

    let mut defaults = serde_json::to_value(newengine_core::StartupLaunchSettings::default())
        .map_err(|error| format!("encode default Project Browser startup settings: {error}"))?;
    if let Some(settings) = defaults.as_object_mut() {
        // schema_version is core-owned metadata. It is useful to preserve when the
        // document is missing it, but the UI only discovers display/graphics leaves.
        let _ = settings;
    }

    let root = document.as_object_mut().expect("object checked above");
    let startup = root
        .entry("startup_settings".to_owned())
        .or_insert_with(|| Value::Object(Map::new()));
    if !startup.is_object() {
        return Err("startup_settings must be a JSON object for Project Browser".to_owned());
    }

    merge_missing(startup, &defaults);
    ensure_object_branch(startup, "display")?;
    ensure_object_branch(startup, "graphics")?;
    Ok(document)
}

/// Applies the narrow settings patch returned by the Project Browser.
///
/// Only the two player-facing branches are writable here. This deliberately
/// rejects attempts to patch engine/plugin/private configuration.
pub fn apply_project_browser_settings_patch(
    response: &Value,
    document: &mut Value,
) -> Result<usize, String> {
    if !document.is_object() {
        return Err("Project Browser startup config document must be a JSON object".to_owned());
    }

    if let Some(patch) = response.get("settings_patch") {
        let entries = patch
            .as_array()
            .ok_or_else(|| "Project Browser settings_patch must be an array".to_owned());
        let entries = match entries {
            Ok(v) => v,
            Err(e) => return Err(e),
        };
        let mut changed = 0usize;
        for (index, entry) in entries.iter().enumerate() {
            let object = entry
                .as_object()
                .ok_or_else(|| "Project Browser settings_patch entry must be an object".to_owned());
            let object = match object {
                Ok(v) => v,
                Err(e) => return Err(format!("{e} at index {index}")),
            };
            let path = object
                .get("path")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    format!("Project Browser settings_patch[{index}].path must be an array")
                })?;
            if path.len() != 3 {
                return Err(format!(
                    "Project Browser settings_patch[{index}] path must contain exactly 3 segments"
                ));
            }
            let segments = path
                .iter()
                .enumerate()
                .map(|(segment_index, value)| {
                    value.as_str().map(str::to_owned).ok_or_else(|| {
                        format!(
                            "Project Browser settings_patch[{index}].path[{segment_index}] must be a string"
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            validate_player_setting_path(&segments, index)?;
            let value = object.get("value").cloned().ok_or_else(|| {
                format!("Project Browser settings_patch[{index}] is missing value")
            })?;
            changed += usize::from(set_existing_value(document, &segments, value)?);
        }
        return Ok(changed);
    }

    // Compatibility with older Project Browser providers that returned the
    // complete graphics object instead of a path patch.
    let Some(legacy_graphics) = response.get("settings") else {
        return Ok(0);
    };
    let Some(legacy_graphics) = legacy_graphics.as_object() else {
        return Err("Project Browser legacy settings payload must be an object".to_owned());
    };
    let mut changed = 0usize;
    for (key, value) in legacy_graphics {
        let path = vec![
            "startup_settings".to_owned(),
            "graphics".to_owned(),
            key.clone(),
        ];
        if value_at_path(document, &path).is_some() {
            changed += usize::from(set_existing_value(document, &path, value.clone())?);
        }
    }
    Ok(changed)
}

fn merge_missing(target: &mut Value, defaults: &Value) {
    let (Some(target), Some(defaults)) = (target.as_object_mut(), defaults.as_object()) else {
        return;
    };
    for (key, default_value) in defaults {
        match target.get_mut(key) {
            Some(existing) if existing.is_object() && default_value.is_object() => {
                merge_missing(existing, default_value);
            }
            Some(_) => {}
            None => {
                target.insert(key.clone(), default_value.clone());
            }
        }
    }
}

fn ensure_object_branch(startup: &mut Value, branch: &str) -> Result<(), String> {
    let object = startup
        .as_object_mut()
        .ok_or_else(|| "startup_settings must be a JSON object".to_owned())?;
    match object.get(branch) {
        Some(value) if value.is_object() => Ok(()),
        Some(_) => Err(format!(
            "startup_settings.{branch} must be a JSON object for Project Browser"
        )),
        None => {
            object.insert(branch.to_owned(), Value::Object(Map::new()));
            Ok(())
        }
    }
}

fn validate_player_setting_path(path: &[String], index: usize) -> Result<(), String> {
    if path[0] != "startup_settings" {
        return Err(format!(
            "Project Browser settings_patch[{index}] rejected non-startup path '{}'",
            path.join(".")
        ));
    }
    if path[1] != "display" && path[1] != "graphics" {
        return Err(format!(
            "Project Browser settings_patch[{index}] rejected non-player branch '{}'",
            path.join(".")
        ));
    }
    if path[2].trim().is_empty() {
        return Err(format!(
            "Project Browser settings_patch[{index}] contains an empty setting name"
        ));
    }
    Ok(())
}

fn value_at_path<'a>(document: &'a Value, path: &[String]) -> Option<&'a Value> {
    let mut current = document;
    for segment in path {
        current = current.get(segment)?;
    }
    Some(current)
}

fn set_existing_value(document: &mut Value, path: &[String], value: Value) -> Result<bool, String> {
    let Some((leaf, parents)) = path.split_last() else {
        return Err("Project Browser settings patch contains an empty path".to_owned());
    };
    let mut current = document;
    for segment in parents {
        current = current.get_mut(segment).ok_or_else(|| {
            format!(
                "Project Browser settings path '{}' no longer exists",
                path.join(".")
            )
        })?;
    }
    let object = current.as_object_mut().ok_or_else(|| {
        format!(
            "Project Browser settings parent '{}' is not an object",
            parents.join(".")
        )
    })?;
    let existing = object.get(leaf).ok_or_else(|| {
        format!(
            "Project Browser settings path '{}' no longer exists",
            path.join(".")
        )
    })?;
    if existing == &value {
        return Ok(false);
    }
    object.insert(leaf.clone(), value);
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn prepare_adds_player_defaults_without_losing_unknown_fields() {
        let original = json!({
            "engine": {"future": {"keep": true}},
            "startup_settings": {
                "graphics": {"bloom_enabled": false, "future": 17}
            }
        });
        let prepared = prepare_project_browser_config_document(original).unwrap();
        assert_eq!(prepared["engine"]["future"]["keep"], json!(true));
        assert_eq!(
            prepared["startup_settings"]["graphics"]["bloom_enabled"],
            json!(false)
        );
        assert_eq!(
            prepared["startup_settings"]["graphics"]["future"],
            json!(17)
        );
        assert!(prepared["startup_settings"]["display"].is_object());
        assert_eq!(
            prepared["startup_settings"]["display"]["resolution"],
            json!([0, 0])
        );
    }

    #[test]
    fn patch_updates_default_injected_display_resolution() {
        let mut document = prepare_project_browser_config_document(json!({
            "startup_settings": {
                "display": {"vsync": true},
                "graphics": {}
            }
        }))
        .unwrap();

        let changed = apply_project_browser_settings_patch(
            &json!({
                "settings_patch": [{
                    "path": ["startup_settings", "display", "resolution"],
                    "value": [2560, 1440]
                }]
            }),
            &mut document,
        )
        .unwrap();

        assert_eq!(changed, 1);
        assert_eq!(
            document["startup_settings"]["display"]["resolution"],
            json!([2560, 1440])
        );
    }

    #[test]
    fn patch_is_narrow_and_preserves_unrelated_json() {
        let mut document = prepare_project_browser_config_document(json!({
            "engine": {"keep": 1},
            "startup_settings": {
                "graphics": {"bloom_enabled": true}
            }
        }))
        .unwrap();
        let changed = apply_project_browser_settings_patch(
            &json!({
                "settings_patch": [{
                    "path": ["startup_settings", "graphics", "bloom_enabled"],
                    "value": false
                }]
            }),
            &mut document,
        )
        .unwrap();
        assert_eq!(changed, 1);
        assert_eq!(
            document["startup_settings"]["graphics"]["bloom_enabled"],
            json!(false)
        );
        assert_eq!(document["engine"]["keep"], json!(1));
    }

    #[test]
    fn patch_rejects_private_engine_paths() {
        let mut document =
            prepare_project_browser_config_document(json!({"engine": {"modules_dir": "x"}}))
                .unwrap();
        let error = apply_project_browser_settings_patch(
            &json!({
                "settings_patch": [{
                    "path": ["engine", "modules_dir", "x"],
                    "value": "bad"
                }]
            }),
            &mut document,
        )
        .unwrap_err();
        assert!(error.contains("rejected"));
    }
}
