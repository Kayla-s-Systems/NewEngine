#![forbid(unsafe_op_in_unsafe_fn)]

mod binder;
mod client;
mod resolver;

#[derive(Clone, Debug)]
pub(crate) struct BackendSelection {
    pub(crate) provider_plugin_id: String,
    pub(crate) provider_state: String,
    pub(crate) matched_by: String,
}

pub(crate) use binder::bind_backend_info;
pub(crate) use client::GenericJsonServiceClient;
