#![forbid(unsafe_op_in_unsafe_fn)]

//! Low-level binary material descriptor payload helpers.
//!
//! Public `.nemat` files are NEF8/ListFile material libraries selected as
//! `file.nemat@entry`. This module is intentionally limited to compact descriptor
//! payload encoding/decoding for tools or inner entry payloads; it is not the
//! top-level `.nemat` runtime file contract.

mod codec;
mod error;
mod format;
mod io;
mod types;

#[cfg(feature = "serde")]
mod json;

pub use codec::{decode_asset, decode_descriptor, encode_asset, encode_descriptor};
pub use error::{MaterialBinaryError, MaterialBinaryResult};
pub use format::{
    MATERIAL_BINARY_HEADER_SIZE,
    MATERIAL_BINARY_MAGIC,
    MATERIAL_BINARY_VERSION,
};
pub use types::MaterialBinaryAsset;

#[cfg(feature = "serde")]
pub use json::{decode_asset_to_json, encode_asset_from_json};
