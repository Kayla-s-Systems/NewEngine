#![forbid(unsafe_op_in_unsafe_fn)]

mod describe;
pub(crate) mod host_api;
pub mod host_context;
mod manager;
mod paths;

pub use host_api::default_host_api;
pub use host_context::init_host_context;
pub use manager::PluginManager;
