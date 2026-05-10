#![forbid(unsafe_op_in_unsafe_fn)]
#![allow(dead_code)]

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
pub use error::UiMarkupError;
pub use state::{UiEvent, UiEventKind, UiState};
pub use theme::{UiDensity, UiThemeDesc, UiVisuals};

#[cfg(feature = "egui")]
pub fn render_egui(doc: &UiMarkupDoc, ctx: &egui::Context, state: &mut UiState) {
    egui_render::render_doc(doc, ctx, state);
}
