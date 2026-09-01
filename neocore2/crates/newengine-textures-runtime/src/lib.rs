#![forbid(unsafe_op_in_unsafe_fn)]

//! Provider-neutral `engine.assets.textures` semantic service implementation.
//!
//! Concrete file formats such as `.ytd` are discovered independently through
//! StarVault format modules. This crate builds the semantic service but owns no
//! engine-plugin identity and performs no Host registration by itself.

mod dictionary_cache;
mod dto;
mod handlers;
mod manifest;
mod references;
mod router;
mod service;
mod state;
mod wire;

pub use dto::{
    TextureManifestRequest, TexturePacketSummary, TextureRefRequest, TextureRefValidation,
    TexturesServiceInfo,
};
pub use router::textures_gateway_service;
pub use service::{textures_service_info, TEXTURES_GATEWAY_OWNER};
pub use state::TextureRuntimeState;

#[cfg(test)]
mod tests;
