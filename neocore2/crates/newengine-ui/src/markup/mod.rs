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
pub use error::UiMarkupError;
pub use state::{UiEvent, UiEventKind, UiState};
pub use theme::{UiDensity, UiThemeDesc, UiVisuals};
