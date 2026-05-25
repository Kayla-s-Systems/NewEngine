#![forbid(unsafe_op_in_unsafe_fn)]
#![allow(dead_code)]

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

const FALLBACK_SVG_SOURCE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32" fill="none"><defs><linearGradient id="ns" x1="16" y1="3" x2="16" y2="29" gradientUnits="userSpaceOnUse"><stop stop-color="#9BE8FF"/><stop offset="1" stop-color="#406BFF"/></linearGradient></defs><path d="M16 2.5 18.8 12.7 29.5 16 18.8 19.3 16 29.5 13.2 19.3 2.5 16 13.2 12.7 16 2.5Z" fill="#4C91FF" fill-opacity=".18" stroke="url(#ns)" stroke-width="1.8" stroke-linejoin="round"/><path d="M16 5v22M5 16h22" stroke="#79DFFF" stroke-width="1.2" stroke-linecap="round"/><circle cx="16" cy="16" r="1.9" fill="#F2FBFF"/></svg>"##;

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
