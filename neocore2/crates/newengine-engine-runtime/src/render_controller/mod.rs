#![forbid(unsafe_op_in_unsafe_fn)]

mod controller;
mod gpu;
mod material_bindings;
mod metrics;
mod module_impl;
mod resource_lifetime;
mod viewport;

pub use controller::RuntimeRenderController;
pub type EditorRenderController = RuntimeRenderController;
