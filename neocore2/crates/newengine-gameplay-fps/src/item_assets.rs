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
    ItemUseEffect, WorldItemDefinition,
};
use newengine_primitives::builtins as primitive_builtins;
use serde::{Deserialize, Serialize};

pub const AUTHORED_ITEM_PACKAGE_SCHEMA: &str = "newengine.items.package.v1";
pub const AUTHORED_ITEM_PACKAGE_VERSION: u32 = 1;
pub const NEITEMS_LOGICAL_PATH: &str = "items/fps_items.neitems";

pub const EMBEDDED_FPS_ITEM_PACKAGE_BYTES: &[u8] =
    include_bytes!("../../../../../gameAssets/items/fps_items.neitems");

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

pub fn compiled_embedded_fps_item_package() -> Result<CompiledItemPackage, String> {
    let authored = decode_authored_item_package_nef8(EMBEDDED_FPS_ITEM_PACKAGE_BYTES)?;
    compile_authored_item_package(&authored)
}

#[cfg(test)]
include!("item_assets/tests.rs");
