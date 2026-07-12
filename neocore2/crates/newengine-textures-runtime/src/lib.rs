#![forbid(unsafe_op_in_unsafe_fn)]

//! Runtime-hosted `engine.assets.textures` service for canonical `.ytd` dictionaries.

mod dictionary_cache;
mod dto;
mod handlers;
mod manifest;
mod references;
mod registration;
mod router;
mod service;
mod state;
mod wire;

pub use dto::{
    TextureManifestRequest, TexturePacketSummary, TextureRefRequest, TextureRefValidation,
    TexturesServiceInfo,
};
pub use registration::register_textures_gateway_best_effort;
pub use router::textures_gateway_service;
pub use service::{textures_service_info, TEXTURES_GATEWAY_OWNER};
pub use state::TextureRuntimeState;

#[cfg(test)]
mod tests;
