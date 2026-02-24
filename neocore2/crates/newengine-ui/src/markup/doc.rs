#![forbid(unsafe_op_in_unsafe_fn)]

use crate::markup::error::UiMarkupError;
use crate::markup::parser::{parse_theme, parse_ui_root};
use crate::markup::theme::UiThemeDesc;
use crate::markup::ui_node::UiNode;
use newengine_assets::{wait_ready, AssetAccess};
use roxmltree::Document;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct UiMarkupDoc {
    pub(crate) root: UiNode,
    pub(crate) theme: UiThemeDesc,
}

impl UiMarkupDoc {
    pub fn load<A: AssetAccess>(
        assets: &A,
        logical_path: &str,
        timeout: Duration,
    ) -> Result<Self, UiMarkupError> {
        let id_hex32 = assets.load(logical_path).map_err(UiMarkupError::Enqueue)?;

        wait_ready(assets, &id_hex32, timeout).map_err(|_| UiMarkupError::Timeout {
            path: logical_path.to_string(),
        })?;

        let (_meta_json, payload) = assets
            .blob_wire_v1(&id_hex32)
            .map_err(|e| UiMarkupError::TextRead(e))?;

        let xml_text = std::str::from_utf8(&payload)
            .map_err(|_| UiMarkupError::TextRead("asset payload is not valid UTF-8".to_string()))?;

        Self::parse(xml_text)
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
