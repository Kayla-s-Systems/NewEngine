#![forbid(unsafe_op_in_unsafe_fn)]

use roxmltree::Document;
use std::time::Duration;

use crate::markup::error::UiMarkupError;
use crate::markup::parser::{parse_theme, parse_ui_root};
use crate::markup::theme::UiThemeDesc;
use crate::markup::ui_node::UiNode;

use crate::asset_access::{wait_ready, AssetAccess};
// см. п.1
use newengine_assets::TextReader;
// если TextReader остаётся в отдельном types crate

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
        let id_hex32 = assets
            .load(logical_path)
            .map_err(UiMarkupError::Enqueue)?;

        wait_ready(assets, &id_hex32, timeout)
            .map_err(|_| UiMarkupError::Timeout { path: logical_path.to_string() })?;

        let (meta_json, payload) = assets
            .blob_wire_v1(&id_hex32)
            .map_err(|e| UiMarkupError::TextRead(e))?;

        let doc = TextReader::from_blob_parts(&meta_json, &payload)
            .map_err(|e| UiMarkupError::TextRead(e.to_string()))?;

        Self::parse(&doc.text)
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