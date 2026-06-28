#![allow(dead_code)]

mod dialog_model;
mod menu_model;
mod modal_surface;
mod native;
mod shell_model;
mod startup_model;
mod toolbar_model;

pub use shell_model::*;
pub use startup_model::*;

pub fn run_editor_ui(startup: &EditorStartupModel) -> Result<(), String> {
    native::run(startup)
}
