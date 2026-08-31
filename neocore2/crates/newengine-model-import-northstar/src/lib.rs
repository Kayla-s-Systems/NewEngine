#![forbid(unsafe_op_in_unsafe_fn)]

//! Offline source importer for NorthStar PC `.pak` character assets.
//!
//! The parser owns source-format details only. Native YDD serialization is delegated
//! to `newengine-asset-format-nef8`; runtime crates never parse North Star data.

mod compile;
mod geometry;
mod pak;
mod skeleton;
mod textures;
mod vfx_compile;

pub use compile::{
    compile_character, compile_rigid_joint_variants, CharacterCompileReport,
    CharacterCompileRequest, PackageSkinSubsetRule, RigidJointVariantsCompileReport,
    RigidJointVariantsCompileRequest,
};
pub use geometry::{decode_geometry_lod0, DecodedGeometry, ImportMesh, SkinLossStats};
pub use pak::{PakFile, PakResource};
pub use skeleton::{
    decode_skeleton, decode_skeleton_with_profile, DecodedSkeleton, ImportedJoint, SkeletonProfile,
};

pub use textures::{decode_vram_textures, ImportedTextureFormat, ImportedVramTexture};

pub use vfx_compile::{
    compile_vfx_texture_dictionary, CompiledVfxTextureEntry, VfxTextureDictionaryCompileReport,
    VfxTextureDictionaryCompileRequest, VfxTextureSelection,
};
