#![forbid(unsafe_op_in_unsafe_fn)]

mod backend_match;
mod client;
mod null_api;
mod runtime_module;
mod service_api;
mod types;

pub use runtime_module::RenderBackendRuntimeModule;
pub use types::{
    ResolvedRenderBackendConfig,
    DEFAULT_RENDER_BACKEND_CLEAR_COLOR,
    DEFAULT_RENDER_BACKEND_ID,
    NULL_RENDER_BACKEND_ID,
};
