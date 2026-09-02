use std::io::{Read, Write};

use flate2::{read::DeflateDecoder, write::DeflateEncoder, Compression};
use newengine_assets_api::{
    encode_list_file, parse_list_file_header, ListFileEncodeRequest,
    LIST_FILE_CONTENT_KIND_NEITEMS, LIST_FILE_FULL_HASH_BODY_THRESHOLD, LIST_FILE_MAGIC_NEF8,
};
use newengine_ecs::World;
use newengine_engine_runtime::gameplay::{
    preload_weapon_audio_definition, AmmoDefinition, AmmoProjectileType, EquipmentSlot,
    FiringPatternDefinition, FiringPatternKind, HitscanWeaponTuning, InventoryEventBus,
    InventoryLoadout, InventoryLoadoutCatalog, InventoryLoadoutEntry, ItemCatalog, ItemDefinition,
    ItemId, ItemKind, ItemUseEffect, MeleeWeaponTuning, WeaponAnimationDefinition,
    WeaponAudioDefinition, WeaponCasingDefinition, WeaponComponentDefinition,
    WeaponComponentGraphDefinition, WeaponComponentModifiers, WeaponComponentPointDefinition,
    WeaponFireMode, WeaponItemDefinition, WeaponPresentationDefinition, WeaponRecoilStateProfile,
    WeaponReloadTopology, WeaponRuntimeProfiles, WeaponSpreadDistribution,
    WeaponSpreadStateProfile, WeaponStatId, WeaponStatModifier, WeaponStatModifierOp,
    WeaponStatModifierStack, WeaponType, WeaponVfxDefinition, WorldItemDefinition,
};
use newengine_primitives::builtins as primitive_builtins;
use serde::{Deserialize, Serialize};

pub const AUTHORED_ITEM_PACKAGE_SCHEMA: &str = "newengine.items.package.v1";
pub const AUTHORED_ITEM_PACKAGE_VERSION: u32 = 1;

#[path = "item_assets/compile.rs"]
mod compile;
#[path = "item_assets/nef8.rs"]
mod nef8;
#[path = "item_assets/types.rs"]
mod types;
#[path = "item_assets/weapon_profiles.rs"]
mod weapon_profiles;
#[path = "item_assets/ytyp.rs"]
mod ytyp;
#[path = "item_assets/ytyp_offline.rs"]
mod ytyp_offline;

pub use compile::{
    compile_authored_item_package, install_compiled_item_package, parse_authored_item_package_json,
};
pub use nef8::{
    decode_authored_item_package, decode_authored_item_package_nef8,
    encode_authored_item_package_nef8,
};
pub use ytyp::hydrate_item_package_from_ytyp;
pub use ytyp_offline::hydrate_item_package_from_ytyp_source_roots;

pub use weapon_profiles::{
    AuthoredWeaponAdsProfile, AuthoredWeaponHandlingProfile, AuthoredWeaponRecoilProfile,
    AuthoredWeaponRecoilStateProfile, AuthoredWeaponRuntimeProfiles, AuthoredWeaponSpreadProfile,
    AuthoredWeaponSpreadStateProfile, AuthoredWeaponStatModifier, AuthoredWeaponSwayProfile,
};

pub use types::{
    AuthoredAmmoDefinition, AuthoredItemDefinition, AuthoredItemPackage, AuthoredLoadoutDefinition,
    AuthoredLoadoutEntry, AuthoredUseEffect, AuthoredWeaponAnimationDefinition,
    AuthoredWeaponAudioDefinition, AuthoredWeaponCasingDefinition,
    AuthoredWeaponComponentDefinition, AuthoredWeaponComponentGraphDefinition,
    AuthoredWeaponComponentModifiers, AuthoredWeaponComponentPointDefinition,
    AuthoredWeaponDefinition, AuthoredWeaponPresentationDefinition, AuthoredWeaponVfxDefinition,
    AuthoredWorldItemDefinition, CompiledItemPackage,
};

use compile::validate_package_header;

#[cfg(any(test, feature = "test-support"))]
pub fn test_fps_item_package() -> AuthoredItemPackage {
    AuthoredItemPackage {
        items: vec![
            // Embedded test content must remain self-contained: production injects/hydrates
            // the Shared YTYP baseline, while this fixture authors the same engine type inline.
            AuthoredItemDefinition {
                id: "weapon.unarmed".to_owned(),
                display_name: "Unarmed".to_owned(),
                kind: "weapon".to_owned(),
                weapon: Some(AuthoredWeaponDefinition {
                    weapon_type: "unarmed".to_owned(),
                    rank: Some(0),
                    ..AuthoredWeaponDefinition::default()
                }),
                ..AuthoredItemDefinition::default()
            },
            AuthoredItemDefinition {
                id: "ammo.rifle.standard".to_owned(),
                display_name: "Test Rifle Ammo".to_owned(),
                kind: "ammo".to_owned(),
                max_stack: 240,
                ammo_profile: Some(AuthoredAmmoDefinition {
                    caliber: "7.62x39mm".to_owned(),
                    projectile_mass_kg: 0.0080,
                    muzzle_velocity_mps: 715.0,
                    penetration_energy_j: 1500.0,
                    max_penetration_m: 0.55,
                    damage_multiplier: 1.0,
                    impulse_multiplier: 1.0,
                    ..AuthoredAmmoDefinition::default()
                }),
                unit_weight: 0.012,
                ..AuthoredItemDefinition::default()
            },
            AuthoredItemDefinition {
                id: "ammo.sidearm.standard".to_owned(),
                display_name: "Test Sidearm Ammo".to_owned(),
                kind: "ammo".to_owned(),
                max_stack: 180,
                ammo_profile: Some(AuthoredAmmoDefinition {
                    caliber: "9x19mm".to_owned(),
                    projectile_mass_kg: 0.0080,
                    muzzle_velocity_mps: 360.0,
                    penetration_energy_j: 420.0,
                    max_penetration_m: 0.28,
                    damage_multiplier: 1.0,
                    impulse_multiplier: 0.75,
                    ..AuthoredAmmoDefinition::default()
                }),
                ..AuthoredItemDefinition::default()
            },
            AuthoredItemDefinition {
                id: "weapon.rifle.standard".to_owned(),
                display_name: "Test Rifle".to_owned(),
                kind: "weapon".to_owned(),
                equipment_slot: "primary".to_owned(),
                weapon: Some(AuthoredWeaponDefinition {
                    rank: Some(300),
                    ammo: "ammo.rifle.standard".to_owned(),
                    damage: 25.0,
                    ..AuthoredWeaponDefinition::default()
                }),
                weapon_casing: Some(AuthoredWeaponCasingDefinition {
                    model_dictionary: "test/models/rifle_shells.ydd".to_owned(),
                    variants: vec![
                        "shell_a".to_owned(),
                        "shell_b".to_owned(),
                        "shell_c".to_owned(),
                        "shell_d".to_owned(),
                        "shell_e".to_owned(),
                    ],
                    material_ref: "test/materials/rifle_shell.nemat@brass".to_owned(),
                    half_extents: [0.00635, 0.00625, 0.02940],
                    ejection_delay_seconds: 1.0 / 30.0,
                    ejection_joint: "shell_eject".to_owned(),
                    inherit_socket_linear_velocity: 1.0,
                    inherit_socket_angular_velocity: 0.35,
                    origin_local: [0.050, 0.025, -0.430],
                    velocity_local: [1.85, 1.25, -0.22],
                    velocity_jitter: [0.35, 0.25, 0.0],
                    axis_local: [0.85, 0.15, 0.0],
                    angular_velocity: [18.0, 11.0, 23.0],
                    angular_velocity_jitter: [4.0, 0.0, -5.0],
                    friction: 0.38,
                    restitution: 0.22,
                    density: 8.5,
                    linear_damping: 0.015,
                    angular_damping: 0.025,
                    contact_min_impulse: 0.002,
                    contact_medium_impulse: 0.012,
                    contact_hard_impulse: 0.035,
                    soft_surface_contains: vec!["dirt".to_owned(), "sand".to_owned()],
                }),
                world: Some(AuthoredWorldItemDefinition {
                    model: "shared/models/weapon/rifle/rifle.ydd@rifle".to_owned(),
                    material_library: "shared/materials/weapon_rifle.nemat".to_owned(),
                    scale: [1.0, 1.0, 1.0],
                    pickup_half_extents: [0.14, 0.55, 0.14],
                    ..AuthoredWorldItemDefinition::default()
                }),
                ..AuthoredItemDefinition::default()
            },
            AuthoredItemDefinition {
                id: "weapon.pistol.standard".to_owned(),
                display_name: "Test Pistol".to_owned(),
                kind: "weapon".to_owned(),
                equipment_slot: "sidearm".to_owned(),
                weapon: Some(AuthoredWeaponDefinition {
                    rank: Some(200),
                    ammo: "ammo.sidearm.standard".to_owned(),
                    ..AuthoredWeaponDefinition::default()
                }),
                ..AuthoredItemDefinition::default()
            },
            AuthoredItemDefinition {
                id: "consumable.medkit.standard".to_owned(),
                display_name: "Test Medkit".to_owned(),
                kind: "consumable".to_owned(),
                max_stack: 5,
                use_effect: Some(AuthoredUseEffect {
                    kind: "heal".to_owned(),
                    amount: 45.0,
                }),
                ..AuthoredItemDefinition::default()
            },
        ],
        loadouts: vec![AuthoredLoadoutDefinition {
            id: "loadout.fps.default".to_owned(),
            display_name: "Test FPS Loadout".to_owned(),
            entries: vec![
                AuthoredLoadoutEntry {
                    item: "weapon.pistol.standard".to_owned(),
                    quantity: 1,
                    equip: true,
                },
                AuthoredLoadoutEntry {
                    item: "weapon.rifle.standard".to_owned(),
                    quantity: 1,
                    equip: true,
                },
                AuthoredLoadoutEntry {
                    item: "ammo.rifle.standard".to_owned(),
                    quantity: 90,
                    equip: false,
                },
                AuthoredLoadoutEntry {
                    item: "ammo.sidearm.standard".to_owned(),
                    quantity: 45,
                    equip: false,
                },
                AuthoredLoadoutEntry {
                    item: "consumable.medkit.standard".to_owned(),
                    quantity: 2,
                    equip: false,
                },
            ],
            ..AuthoredLoadoutDefinition::default()
        }],
        ..AuthoredItemPackage::default()
    }
}

#[cfg(test)]
include!("item_assets/tests.rs");
