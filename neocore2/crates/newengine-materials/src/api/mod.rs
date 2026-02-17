//! Public contracts for materials.
//!
//! Keep this module free from renderer-specific types.

mod descriptor;
mod material;
mod registry;

pub use self::descriptor::{MaterialDescriptor, MaterialFlags};
pub use self::material::{bump_id, fnv1a64, material_id_from_name, MaterialId, MaterialRef};
pub use self::registry::{MaterialProvider, MaterialRegistryApi, MaterialSnapshotItem};
