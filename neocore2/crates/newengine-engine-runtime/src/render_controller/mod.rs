#![forbid(unsafe_op_in_unsafe_fn)]

mod controller;
mod gpu;
mod material_bindings;
mod metrics;
mod module_impl;
mod resource_lifetime;
mod state;
mod resource_cache;
mod render_quality;
mod viewport;

pub use controller::RuntimeRenderController;

pub use newengine_material_domain_api::{MaterialGpuPipelineKey, MaterialGpuPipelineProvider};
