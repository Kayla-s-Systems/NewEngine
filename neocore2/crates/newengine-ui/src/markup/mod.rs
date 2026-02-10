#![forbid(unsafe_op_in_unsafe_fn)]

mod actions;
mod doc;
mod egui_render;
mod error;
mod parser;
mod state;
mod substitute;
mod theme;
mod ui_node;

pub use doc::UiMarkupDoc;
#[cfg(feature = "egui")]
pub use egui_render::render_doc_with_widgets;
pub use error::UiMarkupError;
pub use state::{UiEvent, UiEventKind, UiState};
pub use theme::{UiDensity, UiThemeDesc, UiVisuals};

/// Custom widget bridge for the Egui backend.
///
/// Markup remains engine-agnostic: unknown tags are routed through this interface.
/// The provider may inspect attributes and mutate the `UiState`.
#[cfg(feature = "egui")]
pub trait EguiWidgetProvider {
    /// Attempts to render a custom widget for an unknown tag.
    ///
    /// Returning `true` marks the tag as handled and prevents the fallback renderer
    /// from counting it as unknown.
    fn render(
        &mut self,
        tag: &str,
        attrs: &[(String, String)],
        ui: &mut egui::Ui,
        state: &mut UiState,
    ) -> bool;
}

#[cfg(feature = "egui")]
pub(crate) struct NullEguiWidgetProvider;

#[cfg(feature = "egui")]
impl EguiWidgetProvider for NullEguiWidgetProvider {
    #[inline]
    fn render(
        &mut self,
        _tag: &str,
        _attrs: &[(String, String)],
        _ui: &mut egui::Ui,
        _state: &mut UiState,
    ) -> bool {
        false
    }
}
