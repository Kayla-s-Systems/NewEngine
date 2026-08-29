#![forbid(unsafe_op_in_unsafe_fn)]

//! Low-level compact material descriptor payload helpers.
//!
//! Public `.nemat` files are always NEF8/ListFile material libraries selected as
//! `file.nemat@entry`. There is no standalone material-file magic in this module;
//! these helpers only encode/decode renderer-agnostic descriptor payloads and
//! compact inner named descriptor blobs.

mod codec;
mod error;
mod format;
mod io;
mod types;

#[cfg(feature = "serde")]
mod json;

pub use codec::{decode_asset, decode_descriptor, encode_asset, encode_descriptor};
pub use error::{MaterialBinaryError, MaterialBinaryResult};
pub use format::MATERIAL_BINARY_VERSION;
pub use types::MaterialBinaryAsset;

#[cfg(feature = "serde")]
pub use json::{decode_asset_to_json, encode_asset_from_json};
