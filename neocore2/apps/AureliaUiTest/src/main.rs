#![forbid(unsafe_op_in_unsafe_fn)]

mod app;
mod options;
mod profile;
mod surface_module;
mod ui_document;

fn main() {
    app::run_process();
}
