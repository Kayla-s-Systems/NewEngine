#![forbid(unsafe_op_in_unsafe_fn)]
#![allow(dead_code)]

use crate::asset::{wait_ready, AssetAccess};
use crate::markup::error::UiMarkupError;
use crate::markup::parser::{parse_theme, parse_ui_root};
use crate::markup::theme::UiThemeDesc;
use crate::markup::ui_node::UiNode;
use roxmltree::Document;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct UiMarkupDoc {
    pub(crate) root: UiNode,
    pub(crate) theme: UiThemeDesc,
}

impl UiMarkupDoc {
    pub fn load<A: AssetAccess + ?Sized>(
        assets: &A,
        logical_path: &str,
        timeout: Duration,
    ) -> Result<Self, UiMarkupError> {
        let id_hex32 = assets.load(logical_path).map_err(UiMarkupError::Enqueue)?;

        wait_ready(assets, &id_hex32, timeout).map_err(|_| UiMarkupError::Timeout {
            path: logical_path.to_string(),
        })?;

        let (_meta_json, payload) = assets.blob_wire_v1(&id_hex32).map_err(UiMarkupError::TextRead)?;

        let xml_text = std::str::from_utf8(&payload)
            .map_err(|_| UiMarkupError::TextRead("asset payload is not valid UTF-8".to_string()))?;

        Self::parse(xml_text)
    }

    #[inline]
    pub fn load_dyn(
        assets: &dyn AssetAccess,
        logical_path: &str,
        timeout: Duration,
    ) -> Result<Self, UiMarkupError> {
        Self::load(assets, logical_path, timeout)
    }

    pub fn load_from_fs(root: impl AsRef<Path>, logical_path: &str) -> Result<Self, UiMarkupError> {
        let root: &Path = root.as_ref();
        let rel = logical_path.trim_start_matches(&['/', '\\'][..]);
        let path: PathBuf = root.join(rel);

        let xml_text = std::fs::read_to_string(&path).map_err(|e| {
            UiMarkupError::TextRead(format!("fs read failed: {} ({})", path.display(), e))
        })?;

        Self::parse(&xml_text)
    }

    pub fn load_best_effort(
        assets: Option<&dyn AssetAccess>,
        fs_roots: &[PathBuf],
        logical_path: &str,
        timeout: Duration,
    ) -> Result<Self, UiMarkupError> {
        if let Some(a) = assets {
            if let Ok(doc) = Self::load_dyn(a, logical_path, timeout) {
                return Ok(doc);
            }
        }

        for root in fs_roots {
            if let Ok(doc) = Self::load_from_fs(root, logical_path) {
                return Ok(doc);
            }
        }

        Err(UiMarkupError::TextRead(format!(
            "failed to load UI markup: {} (no assets service, fs roots tried: {})",
            logical_path,
            fs_roots.len()
        )))
    }

    pub fn parse(xml_text: &str) -> Result<Self, UiMarkupError> {
        let parsed = Document::parse(xml_text).map_err(|e| UiMarkupError::XmlParse(e.to_string()))?;

        let root = parse_ui_root(&parsed).map_err(UiMarkupError::Invalid)?;
        let theme = parse_theme(&parsed);

        Ok(Self { root, theme })
    }

    #[inline]
    pub fn theme(&self) -> &UiThemeDesc {
        &self.theme
    }
}