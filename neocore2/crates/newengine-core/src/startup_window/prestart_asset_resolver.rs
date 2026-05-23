#![forbid(unsafe_op_in_unsafe_fn)]

//! Bootstrap AssetManager-style resolver for PreStart UI assets.
//!
//! PreStart runs before the runtime plugin graph is alive, so it cannot call the
//! active `engine.assets` gateway yet. This resolver deliberately mirrors the
//! AssetManager/VFS configuration surface from canonical `config.json` and reads
//! loose UI assets from the same logical roots. Once the engine is running, the
//! same logical paths can be served by the normal AssetManager provider.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde_json::Value;

#[derive(Clone, Debug)]
pub(crate) struct PreStartResolvedAsset {
    pub logical_path: String,
    pub physical_path: PathBuf,
    pub text: String,
}

#[derive(Clone, Debug)]
pub(crate) struct PreStartAssetResolver {
    roots: Vec<PathBuf>,
    warnings: Vec<String>,
}

impl PreStartAssetResolver {
    pub(crate) fn from_config(config_path: &Path, config: &Value) -> Self {
        let mut roots = BTreeSet::<PathBuf>::new();
        let mut warnings = Vec::<String>::new();

        for root in candidate_base_dirs(config_path) {
            push_existing_or_plausible(&mut roots, root.join("assets"));
            push_existing_or_plausible(&mut roots, root.join("NewEngine").join("neocore2").join("assets"));
        }

        for layer in asset_layers(config) {
            let layer_type = layer.get("type").and_then(Value::as_str).unwrap_or("");
            match layer_type {
                "dir" => {
                    if let Some(path) = layer.get("path").and_then(Value::as_str) {
                        for root in candidate_base_dirs(config_path) {
                            push_existing_or_plausible(&mut roots, root.join(path));
                        }
                    }
                }
                "container_glob" | "container" => {
                    warnings.push(format!(
                        "PreStart icon resolver skipped AssetManager container layer type='{layer_type}'; runtime AssetManager will mount it after startup"
                    ));
                }
                "" => {}
                other => warnings.push(format!(
                    "PreStart icon resolver skipped unsupported AssetManager layer type='{other}'"
                )),
            }
        }

        Self {
            roots: roots.into_iter().collect(),
            warnings,
        }
    }

    pub(crate) fn warnings(&self) -> &[String] {
        &self.warnings
    }

    pub(crate) fn roots(&self) -> &[PathBuf] {
        &self.roots
    }

    pub(crate) fn read_text(&self, logical_path: &str) -> Option<PreStartResolvedAsset> {
        let safe = safe_relative_path(logical_path)?;
        for root in &self.roots {
            let physical_path = root.join(&safe);
            if physical_path.is_file() {
                match fs::read_to_string(&physical_path) {
                    Ok(text) => {
                        return Some(PreStartResolvedAsset {
                            logical_path: logical_path.to_owned(),
                            physical_path,
                            text,
                        });
                    }
                    Err(_) => continue,
                }
            }
        }
        None
    }

    pub(crate) fn read_prestart_icon_svg(&self, name: &str) -> Option<PreStartResolvedAsset> {
        let normalized = normalize_icon_name(name)?;
        for prefix in [
            "ui/prestart/icons",
            "ui/icons/prestart",
            "startup_window/assets/icons/prestart",
        ] {
            let logical = format!("{prefix}/{normalized}.svg");
            if let Some(asset) = self.read_text(&logical) {
                return Some(asset);
            }
        }
        None
    }
}

fn asset_layers(config: &Value) -> Vec<&Value> {
    config
        .get("plugins")
        .and_then(|v| v.get("newengine"))
        .and_then(|v| v.get("assets"))
        .and_then(|v| v.get("layers"))
        .and_then(Value::as_array)
        .map(|arr| arr.iter().collect())
        .unwrap_or_default()
}

fn candidate_base_dirs(config_path: &Path) -> Vec<PathBuf> {
    let mut out = Vec::<PathBuf>::new();
    if let Some(parent) = config_path.parent() {
        out.push(parent.to_path_buf());
        for ancestor in parent.ancestors().take(8) {
            out.push(ancestor.to_path_buf());
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        out.push(cwd.clone());
        for ancestor in cwd.ancestors().take(8) {
            out.push(ancestor.to_path_buf());
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            out.push(parent.to_path_buf());
            for ancestor in parent.ancestors().take(8) {
                out.push(ancestor.to_path_buf());
            }
        }
    }
    out
}

fn push_existing_or_plausible(roots: &mut BTreeSet<PathBuf>, path: PathBuf) {
    // Keep plausible roots too: the user may create the icon pack after the
    // current run, and diagnostics should still show where PreStart looked.
    roots.insert(path);
}

fn normalize_icon_name(name: &str) -> Option<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        Some(trimmed.to_ascii_lowercase())
    } else {
        None
    }
}

fn safe_relative_path(logical_path: &str) -> Option<PathBuf> {
    let normalized = logical_path.replace('\\', "/");
    if normalized.is_empty() || normalized.starts_with('/') || normalized.contains('\0') {
        return None;
    }
    let mut out = PathBuf::new();
    for component in Path::new(&normalized).components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    if out.as_os_str().is_empty() { None } else { Some(out) }
}
