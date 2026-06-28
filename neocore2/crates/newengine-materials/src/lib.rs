#![forbid(unsafe_op_in_unsafe_fn)]

//! NewEngine Materials
//!
//! Design goals:
//! - Deterministic ids and stable iteration order.
//! - Clean separation: public API (data contracts) vs runtime registry implementation.
//! - Extensible model: builtins are just providers; plugins can register more.

pub mod api;
pub mod binary;
pub mod builtin;
pub mod core;
pub mod service;
pub mod texture_refs;

#[cfg(feature = "serde")]
pub mod source;

#[cfg(feature = "serde")]
pub mod serde;

mod errors;

pub use crate::api::{
    validate_authored_material_library, AuthoredMaterialDescriptor, AuthoredMaterialLibrary,
    AuthoredMaterialSurface, AuthoredMaterialValidation, MaterialAssetDocument, MaterialDescriptor,
    MaterialEntryV1, MaterialFlags, MaterialId, MaterialParamValue, MaterialRef, MaterialResolved,
    MaterialTextureBindingV1, MaterialTextureBindings, MaterialTextureResidency,
    MaterialTextureResidencyState, MaterialTextureSlot, NematMaterialLibraryBodyV1,
};
pub use crate::api::{
    MaterialDomain, MaterialInstanceDesc, MaterialOverrides, MaterialPermutationKey, ShadingModel,
};
pub use crate::binary::{
    decode_asset as decode_material_asset, encode_asset as encode_material_asset,
};
pub use crate::binary::{
    decode_descriptor as decode_material_descriptor,
    encode_descriptor as encode_material_descriptor,
};
pub use crate::binary::{MaterialBinaryAsset, MaterialBinaryError, MaterialBinaryResult};
pub use crate::core::MaterialRegistry;
pub use crate::errors::{MaterialError, MaterialResult};
pub use crate::texture_refs::{
    is_material_texture_reference, normalize_material_texture_reference,
    validate_material_texture_reference, MaterialTextureReference,
};

#[cfg(feature = "serde")]
pub use crate::source::{
    material_source_from_parts, parse_material_source_json, parse_material_source_slice,
    MaterialSourceDocument,
};

pub use service::*;
