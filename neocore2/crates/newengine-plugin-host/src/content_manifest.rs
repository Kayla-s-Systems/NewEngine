#![forbid(unsafe_op_in_unsafe_fn)]

//! Plugin-owned declarative content catalog.
//!
//! The engine host should not hard-code demo maps, prefab placement rules or
//! importer-side scene payloads. Runtime/editor layers can ask this module for
//! plugin-published content blobs and then adapt those blobs to their own
//! strongly typed domain.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

use crate::manager::PluginLoadError;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PluginContentCatalog {
    #[serde(default)]
    pub schema: String,
    #[serde(default)]
    pub scenes: Vec<PluginContentBlob>,
    #[serde(default)]
    pub prefabs: Vec<PluginContentBlob>,
    #[serde(default)]
    pub materials: Vec<PluginContentBlob>,
    #[serde(default)]
    pub generators: Vec<PluginContentBlob>,
}

impl PluginContentCatalog {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.scenes.is_empty()
            && self.prefabs.is_empty()
            && self.materials.is_empty()
            && self.generators.is_empty()
    }

    #[inline]
    pub fn find_scene(&self, id: &str) -> Option<&PluginContentBlob> {
        self.scenes.iter().find(|blob| blob.id == id)
    }

    #[inline]
    pub fn find_prefab(&self, id: &str) -> Option<&PluginContentBlob> {
        self.prefabs.iter().find(|blob| blob.id == id)
    }

    #[inline]
    pub fn find_generator(&self, id: &str) -> Option<&PluginContentBlob> {
        self.generators.iter().find(|blob| blob.id == id)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PluginContentBlob {
    pub id: String,
    #[serde(default)]
    pub provider_plugin: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub required_capability: String,
    #[serde(default)]
    pub payload: Value,
}

#[derive(Debug, Clone)]
pub struct PluginContentLoadReport {
    pub path: PathBuf,
    pub catalog: PluginContentCatalog,
}

#[derive(Debug, Deserialize)]
struct DeploymentManifestWithContent {
    #[serde(default)]
    schema: String,
    #[serde(default)]
    content: PluginContentCatalog,
}

pub fn load_plugin_content_catalog_default() -> Result<PluginContentLoadReport, PluginLoadError> {
    let dir = crate::paths::default_plugins_dir()?;
    load_plugin_content_catalog_from_dir(&dir)
}

pub fn load_plugin_content_catalog_from_dir(
    dir: impl AsRef<Path>,
) -> Result<PluginContentLoadReport, PluginLoadError> {
    let dir = crate::paths::resolve_plugins_dir(dir.as_ref())?;
    let path = dir.join("plugin-content.manifest.json");
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(PluginContentLoadReport {
                path,
                catalog: PluginContentCatalog::default(),
            });
        }
        Err(e) => {
            return Err(PluginLoadError {
                path,
                message: format!("content manifest read failed: {e}"),
            });
        }
    };

    let mut manifest =
        serde_json::from_slice::<DeploymentManifestWithContent>(&bytes).map_err(|e| {
            PluginLoadError {
                path: path.clone(),
                message: format!("content manifest parse failed: {e}"),
            }
        })?;

    if manifest.content.schema.trim().is_empty() {
        manifest.content.schema = manifest.schema;
    }

    retain_valid_blobs(&mut manifest.content.scenes);
    retain_valid_blobs(&mut manifest.content.prefabs);
    retain_valid_blobs(&mut manifest.content.materials);
    retain_valid_blobs(&mut manifest.content.generators);

    newengine_ulog_api::ulog::info!(
        "plugins: content catalog path='{}' scenes={} prefabs={} materials={} generators={}",
        crate::path_fmt::display_clean(&path),
        manifest.content.scenes.len(),
        manifest.content.prefabs.len(),
        manifest.content.materials.len(),
        manifest.content.generators.len(),
    );

    Ok(PluginContentLoadReport {
        path,
        catalog: manifest.content,
    })
}

#[inline]
fn retain_valid_blobs(blobs: &mut Vec<PluginContentBlob>) {
    blobs.retain(|blob| !blob.id.trim().is_empty() && !blob.payload.is_null());
    blobs.sort_by(|a, b| a.id.cmp(&b.id).then(b.version.cmp(&a.version)));
    blobs.dedup_by(|a, b| a.id == b.id);
}
