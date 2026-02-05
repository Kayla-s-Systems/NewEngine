#![forbid(unsafe_op_in_unsafe_fn)]

use std::thread;
use std::time::{Duration, Instant};

use roxmltree::Document;

use newengine_assets::{AssetKey, AssetState, AssetStore, TextReader};

use crate::markup::error::UiMarkupError;
use crate::markup::parser::{parse_theme, parse_ui_root};
use crate::markup::theme::UiThemeDesc;
use crate::markup::ui_node::UiNode;

#[derive(Debug, Clone)]
pub struct UiMarkupDoc {
    pub(crate) root: UiNode,
    pub(crate) theme: UiThemeDesc,
}

impl UiMarkupDoc {
    pub fn load_from_store<P>(
        store: &AssetStore,
        mut pump: P,
        logical_path: &str,
        timeout: Duration,
    ) -> Result<Self, UiMarkupError>
    where
        P: FnMut(),
    {
        let key = AssetKey::new(logical_path, 0);

        let id = store
            .load(key)
            .map_err(|e| UiMarkupError::Enqueue(e.to_string()))?;

        let t0 = Instant::now();
        let mut spin: u32 = 0;

        loop {
            pump();

            match store.state(id) {
                AssetState::Ready => break,
                AssetState::Failed(msg) => return Err(UiMarkupError::Failed(msg.to_string())),
                AssetState::Loading | AssetState::Unloaded => {}
            }

            if t0.elapsed() >= timeout {
                return Err(UiMarkupError::Timeout {
                    path: logical_path.to_string(),
                });
            }

            spin = spin.saturating_add(1);
            if spin < 32 {
                thread::yield_now();
            } else if spin < 128 {
                thread::sleep(Duration::from_millis(1));
            } else {
                thread::sleep(Duration::from_millis(3));
            }
        }

        let blob = store.get_blob(id).ok_or(UiMarkupError::BlobMissing)?;

        let doc = TextReader::from_blob_parts(&blob.meta_json, &blob.payload)
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

    #[cfg(feature = "egui")]
    pub fn render(&self, ctx: &egui::Context, state: &mut crate::markup::UiState) {
        crate::markup::egui_render::render_doc(self, ctx, state);
    }

    #[inline]
    pub fn theme(&self) -> &UiThemeDesc {
        &self.theme
    }
}