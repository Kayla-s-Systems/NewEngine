use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use crate::manifest::{StartupIntroManifest, STARTUP_INTRO_SCHEMA};
use crate::model::{ResolvedStartupIntro, ResolvedStartupIntroEntry, ResolvedStartupIntroWindow};

pub fn resolve_descriptor_path(raw: &str, runtime_config_path: &Path, root_dir: &Path) -> PathBuf {
    let raw = raw.trim();
    if let Some(relative) = raw
        .strip_prefix("ROOT-DIR/")
        .or_else(|| raw.strip_prefix("ROOT-DIR\\"))
    {
        return root_dir.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
    }
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        path
    } else {
        runtime_config_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(path)
    }
}

pub(crate) fn load_manifest(path: &Path) -> Result<StartupIntroManifest, String> {
    let source = fs::read_to_string(path).map_err(|error| {
        format!(
            "read startup intro descriptor '{}': {error}",
            path.display()
        )
    })?;
    let manifest: StartupIntroManifest = toml::from_str(&source).map_err(|error| {
        format!(
            "parse startup intro descriptor '{}': {error}",
            path.display()
        )
    })?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

pub(crate) fn validate_manifest(manifest: &StartupIntroManifest) -> Result<(), String> {
    if manifest.format_version != 1 {
        return Err(format!(
            "startup intro format_version must be 1; actual={}",
            manifest.format_version
        ));
    }
    if manifest.schema.trim() != STARTUP_INTRO_SCHEMA {
        return Err(format!(
            "startup intro schema must be '{}'; actual='{}'",
            STARTUP_INTRO_SCHEMA, manifest.schema
        ));
    }
    let mode = manifest.window.mode.trim();
    if !matches!(mode, "fullscreen" | "windowed") {
        return Err(format!(
            "startup intro window.mode must be 'fullscreen' or 'windowed'; actual='{mode}'"
        ));
    }
    if manifest.window.failure_timeout_ms == 0 {
        return Err("startup intro window.failure_timeout_ms must be greater than zero".to_owned());
    }
    let mut enabled_ids = HashSet::new();
    for (index, entry) in manifest.sequence.iter().enumerate() {
        if entry.enabled {
            let id = entry.id.trim();
            if id.is_empty() {
                return Err(format!(
                    "startup intro sequence[{index}].id must not be empty"
                ));
            }
            if !enabled_ids.insert(id) {
                return Err(format!(
                    "startup intro sequence[{index}].id '{id}' is duplicated"
                ));
            }
            if entry.source.trim().is_empty() {
                return Err(format!(
                    "startup intro sequence[{index}].source must not be empty"
                ));
            }
            if entry.max_duration_ms == Some(0) {
                return Err(format!(
                    "startup intro sequence[{index}].max_duration_ms must be greater than zero"
                ));
            }
        }
        if !(0.0..=1.0).contains(&entry.volume) {
            return Err(format!(
                "startup intro sequence[{index}].volume must be within 0.0..=1.0"
            ));
        }
    }
    Ok(())
}

pub(crate) fn resolve_payload(
    manifest: &StartupIntroManifest,
    descriptor_path: &Path,
    root_dir: &Path,
) -> Result<ResolvedStartupIntro, String> {
    let descriptor_dir = descriptor_path.parent().unwrap_or_else(|| Path::new("."));
    let sequence = manifest
        .sequence
        .iter()
        .filter(|entry| entry.enabled)
        .map(|entry| {
            let source = resolve_media_path(entry.source.trim(), descriptor_dir, root_dir);
            if !source.is_file() {
                return Err(format!(
                    "startup intro media '{}' for entry '{}' does not exist",
                    source.display(),
                    entry.id
                ));
            }
            Ok(ResolvedStartupIntroEntry {
                id: entry.id.trim().to_owned(),
                source: source.to_string_lossy().into_owned(),
                skippable: entry.skippable,
                volume: entry.volume.clamp(0.0, 1.0),
                max_duration_ms: entry
                    .max_duration_ms
                    .unwrap_or(manifest.window.failure_timeout_ms)
                    .max(1),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    Ok(ResolvedStartupIntro {
        window: ResolvedStartupIntroWindow {
            mode: manifest.window.mode.trim().to_owned(),
            width: manifest.window.width.max(1),
            height: manifest.window.height.max(1),
            background: manifest.window.background.trim().to_owned(),
            topmost: manifest.window.topmost,
            failure_timeout_ms: manifest.window.failure_timeout_ms.max(1),
        },
        sequence,
    })
}

fn resolve_media_path(raw: &str, descriptor_dir: &Path, root_dir: &Path) -> PathBuf {
    if let Some(relative) = raw
        .strip_prefix("ROOT-DIR/")
        .or_else(|| raw.strip_prefix("ROOT-DIR\\"))
    {
        return root_dir.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
    }
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        path
    } else {
        descriptor_dir.join(path)
    }
}
