use super::*;

impl StartupLoader {
    /// Loads the complete persisted startup JSON document without presenting UI,
    /// normalizing typed settings, or publishing process/global state. ProjectBrowser
    /// uses this raw document as its source of truth so unknown and future fields can
    /// survive semantic player-setting edits unchanged.
    pub fn load_startup_document_preview(paths: &ConfigPaths) -> EngineResult<serde_json::Value> {
        let raw_path = paths.startup_path();
        let Some((resolved, _)) = resolve_startup_file_optional(paths, raw_path)? else {
            return Ok(serde_json::json!({}));
        };
        let data = fs::read_to_string(&resolved).map_err(|e| {
            EngineError::Other(format!(
                "startup config document preview read failed: path={:?} err={}",
                resolved, e
            ))
        })?;
        let document: serde_json::Value = serde_json::from_str(&data).map_err(|e| {
            EngineError::Other(format!(
                "startup config document preview parse failed: path={:?} err={}",
                resolved, e
            ))
        })?;
        if !document.is_object() {
            return Err(EngineError::Other(format!(
                "startup config document must be a JSON object: path={:?}",
                resolved
            )));
        }
        Ok(document)
    }

    /// Persists the complete startup JSON document atomically. Unlike
    /// `persist_launch_settings`, this never reconstructs `startup_settings` from a
    /// typed Rust struct and therefore preserves unknown/future fields in every branch.
    pub fn persist_startup_document(
        paths: &ConfigPaths,
        document: &serde_json::Value,
    ) -> EngineResult<()> {
        if !document.is_object() {
            return Err(EngineError::Other(
                "startup config document persistence requires a JSON object".to_owned(),
            ));
        }
        let target = resolve_startup_file_optional(paths, paths.startup_path())?
            .map(|(path, _)| path)
            .unwrap_or_else(|| PathBuf::from(paths.startup_path()));
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                EngineError::Other(format!(
                    "startup config document persist create dir failed: path={:?} err={}",
                    parent, e
                ))
            })?;
        }
        let bytes = serde_json::to_vec_pretty(document)
            .map_err(|e| EngineError::Other(format!("encode startup config document: {e}")))?;
        let temp = target.with_extension("json.newengine-document.tmp");
        fs::write(&temp, bytes).map_err(|e| {
            EngineError::Other(format!(
                "startup config document persist stage failed: path={:?} err={}",
                temp, e
            ))
        })?;
        if target.exists() {
            let _ = fs::remove_file(&target);
        }
        fs::rename(&temp, &target).map_err(|e| {
            EngineError::Other(format!(
                "startup config document persist commit failed: from={:?} to={:?} err={}",
                temp, target, e
            ))
        })?;
        Ok(())
    }

    /// Persists only the core-owned `startup_settings` object while preserving
    /// unrelated startup config keys. Used after the Project Browser Settings tab
    /// is confirmed so the next launch starts from the same values.
    pub fn persist_launch_settings(
        paths: &ConfigPaths,
        settings: &crate::startup_window::StartupLaunchSettings,
    ) -> EngineResult<()> {
        let target = resolve_startup_file_optional(paths, paths.startup_path())?
            .map(|(path, _)| path)
            .unwrap_or_else(|| PathBuf::from(paths.startup_path()));
        let mut root = if target.is_file() {
            let text = fs::read_to_string(&target).map_err(|e| {
                EngineError::Other(format!(
                    "startup config settings persist read failed: path={:?} err={}",
                    target, e
                ))
            })?;
            serde_json::from_str::<serde_json::Value>(&text).map_err(|e| {
                EngineError::Other(format!(
                    "startup config settings persist parse failed: path={:?} err={}",
                    target, e
                ))
            })?
        } else {
            serde_json::json!({})
        };
        let Some(object) = root.as_object_mut() else {
            return Err(EngineError::Other(format!(
                "startup config settings persist requires a JSON object: path={:?}",
                target
            )));
        };
        let mut normalized = settings.clone();
        normalized.normalize();
        object.insert(
            "startup_settings".to_owned(),
            serde_json::to_value(&normalized).map_err(|e| {
                EngineError::Other(format!("encode startup settings for persistence: {e}"))
            })?,
        );
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                EngineError::Other(format!(
                    "startup config settings persist create dir failed: path={:?} err={}",
                    parent, e
                ))
            })?;
        }
        let bytes = serde_json::to_vec_pretty(&root)
            .map_err(|e| EngineError::Other(format!("encode startup config persistence: {e}")))?;
        let temp = target.with_extension("json.newengine-settings.tmp");
        fs::write(&temp, bytes).map_err(|e| {
            EngineError::Other(format!(
                "startup config settings persist stage failed: path={:?} err={}",
                temp, e
            ))
        })?;
        if target.exists() {
            let _ = fs::remove_file(&target);
        }
        fs::rename(&temp, &target).map_err(|e| {
            EngineError::Other(format!(
                "startup config settings persist commit failed: from={:?} to={:?} err={}",
                temp, target, e
            ))
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod startup_document_tests {
    use super::*;
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn startup_document_test_path(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock must be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "northstar-{name}-{}-{stamp}.json",
            std::process::id()
        ))
    }

    #[test]
    fn startup_document_preview_preserves_unknown_config_and_future_settings() {
        let path = startup_document_test_path("preview");
        let original = json!({
            "schema_version": 88,
            "engine": {"modules_dir": "mods", "future": {"keep": true}},
            "plugins": {"engine.render.vulkan.host": {"policy": "strict"}},
            "startup_settings": {
                "schema_version": 91,
                "display": {"vsync": false, "future_display": [1, 2, 3]},
                "graphics": {"preset": "ultra", "volumetrics_quality": "cinematic"}
            }
        });
        fs::write(&path, serde_json::to_vec_pretty(&original).unwrap()).unwrap();
        let paths = ConfigPaths::from_startup_str(path.to_string_lossy().as_ref());

        let loaded = StartupLoader::load_startup_document_preview(&paths)
            .expect("raw startup document must load");
        assert_eq!(loaded, original);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn persist_startup_document_round_trips_full_json_without_typed_field_loss() {
        let path = startup_document_test_path("persist");
        let document = json!({
            "schema_version": 42,
            "engine": {"modules_dir": "mods", "future": "preserve"},
            "plugins": {"custom.plugin": {"schema": {"future": true}}},
            "startup_settings": {
                "schema_version": 4,
                "display": {"vsync": false, "future_display": {"keep": true}},
                "graphics": {
                    "preset": "high",
                    "bloom_enabled": false,
                    "volumetrics_quality": "high",
                    "future_graphics": {"keep": [4, 5, 6]}
                }
            }
        });
        let paths = ConfigPaths::from_startup_str(path.to_string_lossy().as_ref());

        StartupLoader::persist_startup_document(&paths, &document)
            .expect("full startup document must persist");
        let persisted: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(persisted, document);

        let _ = fs::remove_file(path);
    }
}
