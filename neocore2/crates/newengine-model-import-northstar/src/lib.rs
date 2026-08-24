#![forbid(unsafe_op_in_unsafe_fn)]

//! Offline source importer for North Star TLOU2 PC `.pak` character assets.
//!
//! The parser owns source-format details only. Native YDD serialization is delegated
//! to `newengine-asset-format-nef8`; runtime crates never parse North Star data.

mod compile;
mod geometry;
mod pak;
mod skeleton;

pub use compile::{compile_character, CharacterCompileReport, CharacterCompileRequest};
pub use geometry::{decode_geometry_lod0, DecodedGeometry, ImportMesh, SkinLossStats};
pub use pak::{PakFile, PakResource};
pub use skeleton::{
    decode_skeleton, decode_skeleton_with_profile, DecodedSkeleton, ImportedJoint, SkeletonProfile,
};
