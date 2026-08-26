use super::foliage::{decode_runtime_ydd_prefab, DecodedPrefabMeshPart};
use super::*;

use newengine_engine_runtime::gameplay::{
    CharacterBody, DisplayMode, DisplayVisibility, EquippedWeaponBinding, EquippedWeaponMuzzle,
    HitscanWeaponTuning, ItemCatalog, PlayerCommandFrame, PlayerModelAssignment,
    PlayerModelBinding, PlayerSkinBinding, PlayerSkinVertex, PlayerStanceState, PlayerViewState,
    PlayerViewVisibility, PlayerViewVisibilityPolicy, PlayerVisualKind, PlayerVisualPart,
    PlayerWeaponState,
};

// Equipment rendering is decomposed by policy, spawn path, and presentation update.
include!("equipment_visual/policy.rs");
include!("equipment_visual/spawn_skinned.rs");
include!("equipment_visual/spawn_static.rs");
include!("equipment_visual/presentation.rs");
include!("equipment_visual/tests.rs");
