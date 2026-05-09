#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::{Path, PathBuf};

use newengine_plugin_api::{PluginBootstrapPhase, PluginKind};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub(super) struct PluginManifest {
    #[serde(default)]
    pub(super) schema: String,
    #[serde(default)]
    pub(super) plugins: Vec<ManifestPluginEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct ManifestPluginEntry {
    pub(super) id: String,
    #[serde(default)]
    pub(super) file_pattern: String,
    #[serde(default)]
    pub(super) phase: String,
    #[serde(default)]
    pub(super) kind: String,
    #[serde(default)]
    pub(super) required: bool,
    #[serde(default)]
    pub(super) provides: Vec<String>,
}

impl PluginManifest {
    pub(super) fn load_from_plugins_dir(dir: &Path) -> Option<Self> {
        let path = dir.join("plugins.manifest.json");
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(_) => return None,
        };
        match serde_json::from_slice::<Self>(&bytes) {
            Ok(mut manifest) => {
                manifest.plugins.retain(|p| !p.id.trim().is_empty());
                log::info!(
                    "plugins: manifest loaded path='{}' schema='{}' entries={}",
                    crate::path_fmt::display_clean(&path),
                    manifest.schema,
                    manifest.plugins.len(),
                );
                Some(manifest)
            }
            Err(e) => {
                log::warn!(
                    "plugins: manifest ignored path='{}' err='{}'",
                    crate::path_fmt::display_clean(&path),
                    e,
                );
                None
            }
        }
    }

    pub(super) fn match_file_name(&self, file_name: &str) -> Option<&ManifestPluginEntry> {
        self.plugins
            .iter()
            .find(|entry| wildcard_match(&entry.file_pattern, file_name))
    }

    pub(super) fn required_entries_missing_from(&self, files: &[PathBuf]) -> Vec<String> {
        let file_names: Vec<String> = files
            .iter()
            .filter_map(|p| p.file_name().and_then(|s| s.to_str()).map(str::to_owned))
            .collect();

        let mut out = Vec::new();
        for entry in self.plugins.iter().filter(|entry| entry.required) {
            if !file_names
                .iter()
                .any(|file_name| wildcard_match(&entry.file_pattern, file_name))
            {
                out.push(format!("{} expected='{}'", entry.id, entry.file_pattern));
            }
        }
        out
    }
}

impl ManifestPluginEntry {
    #[inline]
    pub(super) fn phase_value(&self) -> PluginBootstrapPhase {
        match self.phase.trim().to_ascii_lowercase().as_str() {
            "bootstrap" => PluginBootstrapPhase::Bootstrap,
            "platform" => PluginBootstrapPhase::Platform,
            _ => PluginBootstrapPhase::Engine,
        }
    }

    #[inline]
    pub(super) fn kind_value(&self) -> PluginKind {
        match self.kind.trim().to_ascii_lowercase().as_str() {
            "runtime" => PluginKind::Runtime,
            "importer" => PluginKind::Importer,
            "tool" => PluginKind::Tool,
            "editor" => PluginKind::Editor,
            _ => PluginKind::Other,
        }
    }
}

fn wildcard_match(pattern: &str, value: &str) -> bool {
    if pattern.is_empty() {
        return false;
    }
    if pattern == "*" {
        return true;
    }

    let pattern = pattern.to_ascii_lowercase();
    let value = value.to_ascii_lowercase();

    let mut parts = pattern.split('*');
    let Some(first) = parts.next() else {
        return value == pattern;
    };

    if !first.is_empty() && !value.starts_with(first) {
        return false;
    }

    let mut cursor = first.len();
    let mut last_part = first;
    for part in parts {
        last_part = part;
        if part.is_empty() {
            continue;
        }
        let Some(found) = value[cursor..].find(part) else {
            return false;
        };
        cursor = cursor.saturating_add(found).saturating_add(part.len());
    }

    pattern.ends_with('*') || last_part.is_empty() || value.ends_with(last_part)
}

#[cfg(test)]
mod tests {
    use super::wildcard_match;

    #[test]
    fn wildcard_match_supports_simple_file_patterns() {
        assert!(wildcard_match("vulkan_renderer-*-dev.dll", "vulkan_renderer-0.3.2-dev.dll"));
        assert!(wildcard_match("assetManager-*.dll", "assetManager-0.5.6-dev.dll"));
        assert!(!wildcard_match("assetManager-*.dll", "input-0.3.5-dev.dll"));
    }
}
