#![forbid(unsafe_op_in_unsafe_fn)]

//! SVG asset registry for the PreStart UI.
//!
//! Primary source is AssetManager/VFS-style logical assets resolved from
//! canonical `config.json` by `PreStartAssetResolver`. The embedded SVG strings
//! fallback/default skin data is embedded as string literals so the crate can
//! compile even when project asset packs are not present yet.

use std::collections::HashMap;
use std::path::Path;

use serde_json::Value;

use super::prestart_asset_resolver::PreStartAssetResolver;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SvgIconAsset {
    pub name: &'static str,
    pub path: &'static str,
    pub source: &'static str,
}

const FALLBACK_SVG_SOURCE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#8fb8ff" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3 20 7.5v9L12 21 4 16.5v-9L12 3Z"/><path d="M12 12 20 7.5"/><path d="M12 12v9"/><path d="M12 12 4 7.5"/></svg>"##;

#[derive(Clone, Debug)]
pub(crate) struct RuntimeSvgIconAsset {
    pub name: String,
    pub logical_path: String,
    pub physical_path: String,
    pub source: String,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct SvgIconRegistry {
    runtime: HashMap<String, RuntimeSvgIconAsset>,
    resolver_warnings: Vec<String>,
    resolver_roots: Vec<String>,
}

macro_rules! svg_asset {
    ($name:literal) => {
        SvgIconAsset {
            name: $name,
            path: concat!("ui/prestart/icons/", $name, ".svg"),
            // Do not use include_str! here. PreStart icons are runtime assets
            // resolved through PreStartAssetResolver from AssetManager/VFS-style
            // roots. A tiny embedded SVG exists only as an emergency fallback so
            // the crate never fails to compile when project assets are absent.
            source: FALLBACK_SVG_SOURCE,
        }
    };
}

pub(crate) const PRESTART_SVG_ICONS: &[SvgIconAsset] = &[
    svg_asset!("animation"),
    svg_asset!("audio"),
    svg_asset!("bookmark"),
    svg_asset!("cancel"),
    svg_asset!("check"),
    svg_asset!("clock"),
    svg_asset!("core"),
    svg_asset!("folder"),
    svg_asset!("fullscreen"),
    svg_asset!("input"),
    svg_asset!("launch"),
    svg_asset!("logo"),
    svg_asset!("monitor"),
    svg_asset!("physics"),
    svg_asset!("project_cube"),
    svg_asset!("puzzle"),
    svg_asset!("renderer"),
    svg_asset!("renderer_chip"),
    svg_asset!("save"),
    svg_asset!("screen_mode"),
    svg_asset!("script"),
    svg_asset!("settings"),
    svg_asset!("status_disabled"),
    svg_asset!("status_enabled"),
    svg_asset!("terminal"),
    svg_asset!("ui"),
];

impl SvgIconRegistry {
    pub(crate) fn load(config_path: &Path, config: &Value) -> Self {
        let resolver = PreStartAssetResolver::from_config(config_path, config);
        let mut registry = Self {
            runtime: HashMap::new(),
            resolver_warnings: resolver.warnings().to_vec(),
            resolver_roots: resolver
                .roots()
                .iter()
                .map(|p| p.display().to_string())
                .collect(),
        };

        for icon in PRESTART_SVG_ICONS {
            if let Some(resolved) = resolver.read_prestart_icon_svg(icon.name) {
                registry.runtime.insert(
                    icon.name.to_owned(),
                    RuntimeSvgIconAsset {
                        name: icon.name.to_owned(),
                        logical_path: resolved.logical_path,
                        physical_path: resolved.physical_path.display().to_string(),
                        source: resolved.text,
                    },
                );
            }
        }

        registry
    }

    pub(crate) fn runtime_count(&self) -> usize {
        self.runtime.len()
    }

    pub(crate) fn warnings(&self) -> &[String] {
        &self.resolver_warnings
    }

    pub(crate) fn roots(&self) -> &[String] {
        &self.resolver_roots
    }

    pub(crate) fn source_label(&self) -> String {
        if self.runtime.is_empty() {
            "icons: embedded fallback skin".to_owned()
        } else {
            format!("icons: AssetManager roots ({}/{})", self.runtime.len(), PRESTART_SVG_ICONS.len())
        }
    }

    pub(crate) fn find(&self, name: &str) -> Option<&RuntimeSvgIconAsset> {
        self.runtime.get(name)
    }
}

pub(crate) fn find_fallback_svg_icon(name: &str) -> Option<&'static SvgIconAsset> {
    PRESTART_SVG_ICONS.iter().find(|asset| asset.name == name)
}
