#![forbid(unsafe_op_in_unsafe_fn)]

//! Binary material format.
//!
//! This module defines a deterministic, little-endian, forward-compatible container for
//! storing materials on disk and in caches.
//!
//! The format is intentionally conservative:
//! - no platform-dependent struct layout;
//! - no `unsafe`;
//! - explicit versioning;
//! - a fixed-size descriptor payload for stable caches.
//!
//! Recommended file extension: `.nemat`.

mod codec;
mod error;
mod format;
mod io;
#[cfg(test)]
mod tests;
mod types;

#[cfg(feature = "serde")]
mod json;

pub use codec::{decode_asset, decode_descriptor, encode_asset, encode_descriptor};
pub use error::{MaterialBinaryError, MaterialBinaryResult};
pub use format::{
    MATERIAL_BINARY_HEADER_SIZE,
    MATERIAL_BINARY_MAGIC,
    MATERIAL_BINARY_VERSION,
    MATERIAL_DESCRIPTOR_SIZE,
};
pub use types::MaterialBinaryAsset;

#[cfg(feature = "serde")]
pub use json::{decode_asset_to_json, encode_asset_from_json};
