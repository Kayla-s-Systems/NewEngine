//! Public contracts for materials.
//!
//! Keep this module free from renderer-specific types.

mod authored;
mod descriptor;
mod instance;
mod material;
mod registry;
mod textures;

mod types;

pub use self::authored::{validate_authored_material_library, AuthoredMaterialDescriptor, AuthoredMaterialLibrary, AuthoredMaterialSurface, AuthoredMaterialValidation, MaterialEntryV1, MaterialParamValue, MaterialTextureBindingV1, NematMaterialLibraryBodyV1};
pub use self::descriptor::{MaterialDescriptor, MaterialFlags};
pub use self::instance::{MaterialInstanceDesc, MaterialOverrides};
pub use self::material::{
    bump_id, fnv1a64, material_id_from_name, material_instance_id, MaterialId, MaterialRef,
};
pub use self::registry::{MaterialProvider, MaterialRegistryApi, MaterialSnapshotItem};
pub use self::textures::{
    MaterialAssetDocument, MaterialResolved, MaterialTextureBindings, MaterialTextureResidency,
    MaterialTextureResidencyState, MaterialTextureSlot,
};
pub use self::types::{MaterialDomain, MaterialPermutationKey, ShadingModel};
