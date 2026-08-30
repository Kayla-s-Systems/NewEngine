#![forbid(unsafe_op_in_unsafe_fn)]

mod controller;
mod error_policy;
mod gpu;
mod material_bindings;
mod material_plan_cache;
mod metrics;
mod module_impl;
mod render_quality;
mod resource_cache;
mod resource_lifetime;
mod runtime_profile;
mod state;
mod viewport;

pub use controller::RuntimeRenderController;

pub use newengine_material_domain_api::{MaterialGpuPipelineKey, MaterialGpuPipelineProvider};
