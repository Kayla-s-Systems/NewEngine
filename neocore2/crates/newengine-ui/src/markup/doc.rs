#![forbid(unsafe_op_in_unsafe_fn)]
#![allow(dead_code)]

use crate::asset::AssetAccess;
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
        let _ = timeout;
        let payload = assets
            .text_v1(logical_path)
            .map_err(UiMarkupError::TextRead)?;

        let xml_text = std::str::from_utf8(&payload).map_err(|_| {
            UiMarkupError::TextRead("asset.text_v1 payload is not valid UTF-8".to_string())
        })?;

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
        let _ = root.as_ref();
        Err(UiMarkupError::TextRead(format!(
            "filesystem UI markup loading is disabled; use AssetManager.text_v1 for '{}'",
            logical_path
        )))
    }

    pub fn load_best_effort(
        assets: Option<&dyn AssetAccess>,
        fs_roots: &[PathBuf],
        logical_path: &str,
        timeout: Duration,
    ) -> Result<Self, UiMarkupError> {
        let _ = fs_roots;
        if let Some(a) = assets {
            return Self::load_dyn(a, logical_path, timeout);
        }

        Err(UiMarkupError::TextRead(format!(
            "failed to load UI markup: {} (AssetManager service unavailable)",
            logical_path
        )))
    }

    pub fn parse(xml_text: &str) -> Result<Self, UiMarkupError> {
        let parsed =
            Document::parse(xml_text).map_err(|e| UiMarkupError::XmlParse(e.to_string()))?;

        let root = parse_ui_root(&parsed).map_err(UiMarkupError::Invalid)?;
        let theme = parse_theme(&parsed);

        Ok(Self { root, theme })
    }

    #[inline]
    pub fn theme(&self) -> &UiThemeDesc {
        &self.theme
    }
}
