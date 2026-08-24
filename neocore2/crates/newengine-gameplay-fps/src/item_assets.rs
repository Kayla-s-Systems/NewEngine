use std::io::{Read, Write};

use flate2::{read::DeflateDecoder, write::DeflateEncoder, Compression};
use newengine_assets_api::{
    encode_list_file, parse_list_file_header, ListFileEncodeRequest,
    LIST_FILE_CONTENT_KIND_NEITEMS, LIST_FILE_FULL_HASH_BODY_THRESHOLD, LIST_FILE_MAGIC_NEF8,
};
use newengine_ecs::World;
use newengine_engine_runtime::gameplay::{
    EquipmentSlot, HitscanWeaponTuning, InventoryEventBus, InventoryLoadout,
    InventoryLoadoutCatalog, InventoryLoadoutEntry, ItemCatalog, ItemDefinition, ItemId, ItemKind,
    ItemUseEffect, WeaponFireMode, WorldItemDefinition,
};
use newengine_primitives::builtins as primitive_builtins;
use serde::{Deserialize, Serialize};

pub const AUTHORED_ITEM_PACKAGE_SCHEMA: &str = "newengine.items.package.v1";
pub const AUTHORED_ITEM_PACKAGE_VERSION: u32 = 1;
pub const NEITEMS_LOGICAL_PATH: &str = newengine_game_data::DEFAULT_ITEM_PACKAGE_ASSET;

#[path = "item_assets/compile.rs"]
mod compile;
#[path = "item_assets/nef8.rs"]
mod nef8;
#[path = "item_assets/types.rs"]
mod types;

pub use compile::{
    compile_authored_item_package, install_compiled_item_package, parse_authored_item_package_json,
};
pub use nef8::{
    decode_authored_item_package, decode_authored_item_package_nef8,
    encode_authored_item_package_nef8,
};
pub use types::{
    AuthoredItemDefinition, AuthoredItemPackage, AuthoredLoadoutDefinition, AuthoredLoadoutEntry,
    AuthoredUseEffect, AuthoredWeaponDefinition, CompiledItemPackage,
};

use compile::validate_package_header;

#[cfg(test)]
pub(crate) fn test_fps_item_package() -> AuthoredItemPackage {
    AuthoredItemPackage {
        items: vec![
            AuthoredItemDefinition {
                id: "ammo.rifle.standard".to_owned(),
                display_name: "Test Rifle Ammo".to_owned(),
                kind: "ammo".to_owned(),
                max_stack: 240,
                ..AuthoredItemDefinition::default()
            },
            AuthoredItemDefinition {
                id: "ammo.sidearm.standard".to_owned(),
                display_name: "Test Sidearm Ammo".to_owned(),
                kind: "ammo".to_owned(),
                max_stack: 180,
                ..AuthoredItemDefinition::default()
            },
            AuthoredItemDefinition {
                id: "weapon.rifle.standard".to_owned(),
                display_name: "Test Rifle".to_owned(),
                kind: "weapon".to_owned(),
                equipment_slot: "primary".to_owned(),
                weapon: Some(AuthoredWeaponDefinition {
                    ammo: "ammo.rifle.standard".to_owned(),
                    damage: 25.0,
                    ..AuthoredWeaponDefinition::default()
                }),
                ..AuthoredItemDefinition::default()
            },
            AuthoredItemDefinition {
                id: "weapon.pistol.standard".to_owned(),
                display_name: "Test Pistol".to_owned(),
                kind: "weapon".to_owned(),
                equipment_slot: "sidearm".to_owned(),
                weapon: Some(AuthoredWeaponDefinition {
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
