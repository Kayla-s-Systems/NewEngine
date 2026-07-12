#![forbid(unsafe_op_in_unsafe_fn)]

use super::*;

#[path = "foliage/diagnostics.rs"]
mod diagnostics;
#[path = "foliage/material_binding.rs"]
mod material_binding;
#[path = "foliage/placement.rs"]
mod placement;
#[path = "foliage/spawn.rs"]
mod spawn;
#[path = "foliage/types.rs"]
mod types;
#[path = "foliage/ydd_mesh.rs"]
mod ydd_mesh;

use self::diagnostics::*;
use self::material_binding::*;
use self::placement::{choose_foliage_prefab, collect_tree_placements};
use self::types::*;
use self::ydd_mesh::*;

pub(super) use self::placement::terrain_height;
pub(super) use self::spawn::spawn_foliage_prefabs;
pub(super) use self::types::{DecodedPrefabMeshPart, SKYDOME_PRIMITIVE_ID};
pub(super) use self::ydd_mesh::decode_runtime_ydd_prefab;
