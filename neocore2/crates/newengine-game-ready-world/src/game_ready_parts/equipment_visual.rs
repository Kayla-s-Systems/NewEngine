use super::foliage::{decode_runtime_ydd_prefab, DecodedPrefabMeshPart};
use super::*;

use newengine_engine_runtime::gameplay::{
    DisplayMode, DisplayVisibility, EquippedWeaponBinding, EquippedWeaponEntity,
    EquippedWeaponMuzzle, HitscanWeaponTuning, ItemCatalog, PlayerCommandFrame, PlayerModelBinding,
    PlayerSkinBinding, PlayerSkinVertex, PlayerViewState, PlayerViewVisibility,
    PlayerViewVisibilityPolicy, PlayerVisualKind, PlayerVisualPart, PlayerWeaponState,
    WeaponEntityRuntime, WeaponEntitySockets, WeaponObstructionState, WeaponSocketPose,
};

// Equipment rendering is decomposed by policy, spawn path, and presentation update.
include!("equipment_visual/policy.rs");
include!("equipment_visual/spawn_skinned.rs");
include!("equipment_visual/spawn_static.rs");
include!("equipment_visual/presentation.rs");
include!("equipment_visual/tests.rs");
