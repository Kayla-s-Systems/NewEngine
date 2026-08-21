#![forbid(unsafe_op_in_unsafe_fn)]

mod binder;
mod client;
mod resolver;

#[derive(Clone, Debug)]
pub struct BackendSelection {
    pub provider_plugin_id: String,
    pub provider_state: String,
    pub matched_by: String,
}

pub use binder::bind_backend_info;
pub use client::GenericJsonServiceClient;
