#![forbid(unsafe_op_in_unsafe_fn)]

//! AssetManager-backed resolver for the bootstrap PreStart UI.
//!
//! The PreStart window must not maintain its own VFS/path search rules. It asks
//! the same `engine.assets` gateway that the runtime uses. When AssetManager is
//! not available yet, callers fall back to their embedded emergency skin; this
//! module never scans directories or parses asset containers itself.

use std::path::{Path, PathBuf};

use abi_stable::std_types::{RResult, RString, RVec};
use newengine_assets_api::{method, ENGINE_ASSET_SERVICE_ID};
use newengine_plugin_host::call_service_v1;
use serde_json::Value;

#[derive(Clone, Debug)]
pub(crate) struct PreStartResolvedAsset {
    pub logical_path: String,
    pub physical_path: PathBuf,
    pub text: String,
}

#[derive(Clone, Debug)]
pub(crate) struct PreStartAssetResolver {
    warnings: Vec<String>,
}

impl PreStartAssetResolver {
    pub(crate) fn from_config(_config_path: &Path, _config: &Value) -> Self {
        Self { warnings: Vec::new() }
    }

    pub(crate) fn warnings(&self) -> &[String] {
        &self.warnings
    }

    pub(crate) fn roots(&self) -> &[PathBuf] {
        &[]
    }

    pub(crate) fn read_text(&self, logical_path: &str) -> Option<PreStartResolvedAsset> {
        let logical_path = normalize_logical_path(logical_path)?;
        let payload = RVec::from(logical_path.as_bytes().to_vec());
        let response = call_service_v1(
            RString::from(ENGINE_ASSET_SERVICE_ID),
            RString::from(method::TEXT_V1),
            payload,
        );

        match response {
            RResult::ROk(bytes) => {
                let text = String::from_utf8(bytes.into_vec()).ok()?;
                Some(PreStartResolvedAsset {
                    logical_path,
                    physical_path: PathBuf::from("engine.assets"),
                    text,
                })
            }
            RResult::RErr(_) => None,
        }
    }

    pub(crate) fn read_prestart_icon_svg(&self, name: &str) -> Option<PreStartResolvedAsset> {
        let normalized = normalize_icon_name(name)?;
        self.read_text(&format!("ui/prestart/icons/{normalized}.svg"))
    }
}

fn normalize_logical_path(path: &str) -> Option<String> {
    let normalized = path.trim().replace('\\', "/");
    if normalized.is_empty()
        || normalized.starts_with('/')
        || normalized.contains(':')
        || normalized.split('/').any(|part| part.is_empty() || part == "." || part == "..")
    {
        return None;
    }
    Some(normalized)
}

fn normalize_icon_name(name: &str) -> Option<String> {
    let normalized = name.trim().to_ascii_lowercase().replace(' ', "_").replace('-', "_");
    if normalized.is_empty()
        || normalized.contains('/')
        || normalized.contains('\\')
        || normalized.contains('.')
        || normalized.contains(':')
    {
        return None;
    }
    Some(normalized)
}
