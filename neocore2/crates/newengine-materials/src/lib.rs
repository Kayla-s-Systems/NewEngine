#![forbid(unsafe_op_in_unsafe_fn)]

//! NewEngine Materials
//!
//! Design goals:
//! - Deterministic ids and stable iteration order.
//! - Clean separation: public API (data contracts) vs runtime registry implementation.
//! - Extensible model: builtins are just providers; plugins can register more.

pub mod api;
pub mod builtin;
pub mod core;
pub mod binary;

#[cfg(feature = "serde")]
pub mod serde;

mod errors;

pub use crate::api::{
    MaterialAssetDocument, MaterialDescriptor, MaterialFlags, MaterialId, MaterialRef,
    MaterialResolved, MaterialTextureBindings,
};
pub use crate::api::{
    MaterialDomain, MaterialInstanceDesc, MaterialOverrides, MaterialPermutationKey, ShadingModel,
};
pub use crate::binary::{decode_asset as decode_material_asset, encode_asset as encode_material_asset};
pub use crate::binary::{decode_descriptor as decode_material_descriptor, encode_descriptor as encode_material_descriptor};
pub use crate::binary::{MaterialBinaryAsset, MaterialBinaryError, MaterialBinaryResult};
pub use crate::core::MaterialRegistry;
pub use crate::errors::{MaterialError, MaterialResult};
