use std::io::{Read, Write};

use flate2::{read::DeflateDecoder, write::DeflateEncoder, Compression};
use newengine_assets_api::{
    parse_list_file_header_v1, LIST_FILE_COMPRESSION_DEFLATE, LIST_FILE_CONTENT_KIND_NEITEMS,
    LIST_FILE_FLAG_BODY_DEFLATE, LIST_FILE_HEADER_LEN_V1, LIST_FILE_MAGIC_NEF8,
    LIST_FILE_VERSION_V1,
};
use newengine_ecs::World;
use newengine_math::fnv1a_64;
use newengine_primitives::builtins as primitive_builtins;
use serde::{Deserialize, Serialize};

use super::combat::HitscanWeaponTuning;
use super::inventory::{
    EquipmentSlot, InventoryEventBus, InventoryLoadout, InventoryLoadoutCatalog,
    InventoryLoadoutEntry, ItemCatalog, ItemDefinition, ItemId, ItemKind, ItemUseEffect,
    WorldItemDefinition,
};

pub const AUTHORED_ITEM_PACKAGE_SCHEMA: &str = "newengine.items.package.v1";
pub const AUTHORED_ITEM_PACKAGE_VERSION: u32 = 1;
pub const NEITEMS_LOGICAL_PATH: &str = "items/fps_items.neitems";

pub const EMBEDDED_FPS_ITEM_PACKAGE_BYTES: &[u8] =
    include_bytes!("../../../../../../gameAssets/items/fps_items.neitems");

pub fn compiled_embedded_fps_item_package() -> Result<CompiledItemPackage, String> {
    let authored = decode_authored_item_package_nef8(EMBEDDED_FPS_ITEM_PACKAGE_BYTES)?;
    compile_authored_item_package(&authored)
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AuthoredItemPackage {
    pub schema: String,
    pub version: u32,
    pub items: Vec<AuthoredItemDefinition>,
    pub loadouts: Vec<AuthoredLoadoutDefinition>,
}

impl Default for AuthoredItemPackage {
    fn default() -> Self {
        Self {
            schema: AUTHORED_ITEM_PACKAGE_SCHEMA.to_owned(),
            version: AUTHORED_ITEM_PACKAGE_VERSION,
            items: Vec::new(),
            loadouts: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AuthoredItemDefinition {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub icon: String,
    pub tags: Vec<String>,
    pub kind: String,
    pub max_stack: u32,
    pub unit_weight: f32,
    pub equipment_slot: String,
    pub weapon: Option<AuthoredWeaponDefinition>,
    pub use_effect: Option<AuthoredUseEffect>,
    pub world: Option<AuthoredWorldItemDefinition>,
}

impl Default for AuthoredItemDefinition {
    fn default() -> Self {
        Self {
            id: String::new(),
            display_name: String::new(),
            description: String::new(),
            icon: String::new(),
            tags: Vec::new(),
            kind: "generic".to_owned(),
            max_stack: 1,
            unit_weight: 0.0,
            equipment_slot: String::new(),
            weapon: None,
            use_effect: None,
            world: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AuthoredWeaponDefinition {
    pub ammo: String,
    pub magazine_capacity: u32,
    pub reserve_capacity: u32,
    pub fire_interval: f32,
    pub reload_duration: f32,
    pub damage: f32,
    pub range: f32,
    pub hip_spread_degrees: f32,
    pub aim_spread_degrees: f32,
    pub recoil_pitch_degrees: f32,
    pub recoil_yaw_degrees: f32,
    pub muzzle_forward_offset: f32,
}

impl Default for AuthoredWeaponDefinition {
    fn default() -> Self {
        let tuning = HitscanWeaponTuning::default();
        Self {
            ammo: String::new(),
            magazine_capacity: tuning.magazine_capacity,
            reserve_capacity: tuning.reserve_capacity,
            fire_interval: tuning.fire_interval,
            reload_duration: tuning.reload_duration,
            damage: tuning.damage,
            range: tuning.range,
            hip_spread_degrees: tuning.hip_spread_radians.to_degrees(),
            aim_spread_degrees: tuning.aim_spread_radians.to_degrees(),
            recoil_pitch_degrees: tuning.recoil_pitch_radians.to_degrees(),
            recoil_yaw_degrees: tuning.recoil_yaw_radians.to_degrees(),
            muzzle_forward_offset: tuning.muzzle_forward_offset,
        }
    }
}

impl AuthoredWeaponDefinition {
    fn tuning(&self) -> HitscanWeaponTuning {
        HitscanWeaponTuning {
            magazine_capacity: self.magazine_capacity,
            reserve_capacity: self.reserve_capacity,
            fire_interval: self.fire_interval,
            reload_duration: self.reload_duration,
            damage: self.damage,
            range: self.range,
            hip_spread_radians: self.hip_spread_degrees.to_radians(),
            aim_spread_radians: self.aim_spread_degrees.to_radians(),
            recoil_pitch_radians: self.recoil_pitch_degrees.to_radians(),
            recoil_yaw_radians: self.recoil_yaw_degrees.to_radians(),
            muzzle_forward_offset: self.muzzle_forward_offset,
        }
        .sanitized()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AuthoredUseEffect {
    pub kind: String,
    pub amount: f32,
}

impl Default for AuthoredUseEffect {
    fn default() -> Self {
        Self {
            kind: "none".to_owned(),
            amount: 0.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AuthoredWorldItemDefinition {
    pub model: String,
    pub fallback_primitive: String,
    pub scale: [f32; 3],
    pub color_rgba: [f32; 4],
    pub pickup_half_extents: [f32; 3],
    pub respawn_seconds: f32,
}

impl Default for AuthoredWorldItemDefinition {
    fn default() -> Self {
        Self {
            model: String::new(),
            fallback_primitive: "cube".to_owned(),
            scale: [0.2, 0.2, 0.2],
            color_rgba: [0.55, 0.60, 0.68, 1.0],
            pickup_half_extents: [0.2, 0.2, 0.2],
            respawn_seconds: 0.0,
        }
    }
}

impl AuthoredWorldItemDefinition {
    fn compile(&self, kind: ItemKind) -> Result<WorldItemDefinition, String> {
        let fallback_primitive = match self.fallback_primitive.trim().to_ascii_lowercase().as_str()
        {
            "" | "cube" => primitive_builtins::ID_CUBE,
            "sphere" | "sphere_uv" => primitive_builtins::ID_SPHERE_UV,
            "cylinder" => primitive_builtins::ID_CYLINDER,
            "capsule" => primitive_builtins::ID_CAPSULE,
            "cone" => primitive_builtins::ID_CONE,
            "torus" => primitive_builtins::ID_TORUS,
            "disc" => primitive_builtins::ID_DISC,
            other => return Err(format!("unsupported world fallback primitive '{other}'")),
        };
        let mut definition = WorldItemDefinition::for_kind(kind);
        definition.model_ref =
            (!self.model.trim().is_empty()).then(|| self.model.trim().to_owned());
        definition.fallback_primitive = fallback_primitive;
        definition.scale = self.scale;
        definition.color = self.color_rgba;
        definition.pickup_half_extents = self.pickup_half_extents;
        definition.respawn_seconds = self.respawn_seconds;
        Ok(definition.sanitized())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AuthoredLoadoutDefinition {
    pub id: String,
    pub display_name: String,
    pub clear_existing: bool,
    pub entries: Vec<AuthoredLoadoutEntry>,
}

impl Default for AuthoredLoadoutDefinition {
    fn default() -> Self {
        Self {
            id: String::new(),
            display_name: String::new(),
            clear_existing: true,
            entries: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct AuthoredLoadoutEntry {
    pub item: String,
    pub quantity: u32,
    pub equip: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledItemPackage {
    pub catalog: ItemCatalog,
    pub loadouts: InventoryLoadoutCatalog,
}

pub fn parse_authored_item_package_json(bytes: &[u8]) -> Result<AuthoredItemPackage, String> {
    let package: AuthoredItemPackage = serde_json::from_slice(bytes)
        .map_err(|error| format!("authored item package JSON parse failed: {error}"))?;
    validate_package_header(&package)?;
    Ok(package)
}

pub fn compile_authored_item_package(
    package: &AuthoredItemPackage,
) -> Result<CompiledItemPackage, String> {
    validate_package_header(package)?;
    let mut catalog = ItemCatalog::default();

    for authored in &package.items {
        let definition = compile_item_definition(authored)?;
        catalog.register(definition)?;
    }

    for definition in catalog.definitions() {
        if let Some(weapon) = definition.weapon {
            let ammo = catalog.get(weapon.ammo_item).ok_or_else(|| {
                format!(
                    "weapon '{}' references missing ammo item {:016x}",
                    definition.name,
                    weapon.ammo_item.raw()
                )
            })?;
            if ammo.kind != ItemKind::Ammo {
                return Err(format!(
                    "weapon '{}' ammo reference '{}' is not kind=ammo",
                    definition.name, ammo.name
                ));
            }
        }
    }

    let mut loadouts = InventoryLoadoutCatalog::default();
    for authored in &package.loadouts {
        let mut loadout = InventoryLoadout::new(&authored.id)?;
        loadout.name = if authored.display_name.trim().is_empty() {
            authored.id.trim().to_owned()
        } else {
            authored.display_name.trim().to_owned()
        };
        loadout.clear_existing = authored.clear_existing;
        for entry in &authored.entries {
            let definition = catalog.find(&entry.item).ok_or_else(|| {
                format!(
                    "loadout '{}' references missing item '{}'",
                    authored.id, entry.item
                )
            })?;
            loadout.entries.push(InventoryLoadoutEntry {
                item: definition.id,
                quantity: entry.quantity,
                equip: entry.equip,
            });
        }
        loadouts.register(loadout)?;
    }

    Ok(CompiledItemPackage { catalog, loadouts })
}

pub fn install_compiled_item_package(world: &mut World, package: CompiledItemPackage) {
    world.insert_resource(package.catalog);
    world.insert_resource(package.loadouts);
    if world.resource::<InventoryEventBus>().is_none() {
        world.insert_resource(InventoryEventBus::default());
    }
}

pub fn encode_authored_item_package_nef8(
    package: &AuthoredItemPackage,
    logical_path: &str,
) -> Result<Vec<u8>, String> {
    validate_package_header(package)?;
    let body = serde_json::to_vec(package)
        .map_err(|error| format!("item package JSON encode failed: {error}"))?;
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(&body)
        .map_err(|error| format!("NEITEMS deflate write failed: {error}"))?;
    let compressed = encoder
        .finish()
        .map_err(|error| format!("NEITEMS deflate finish failed: {error}"))?;

    let mut output = vec![0u8; LIST_FILE_HEADER_LEN_V1];
    output[0..4].copy_from_slice(&LIST_FILE_MAGIC_NEF8);
    write_u16(&mut output, 4, LIST_FILE_VERSION_V1);
    write_u16(&mut output, 6, LIST_FILE_HEADER_LEN_V1 as u16);
    write_u16(&mut output, 8, LIST_FILE_CONTENT_KIND_NEITEMS as u16);
    write_u16(&mut output, 10, LIST_FILE_FLAG_BODY_DEFLATE);
    write_u16(&mut output, 12, LIST_FILE_COMPRESSION_DEFLATE);
    write_u64(&mut output, 16, LIST_FILE_HEADER_LEN_V1 as u64);
    write_u64(&mut output, 24, compressed.len() as u64);
    write_u64(&mut output, 32, body.len() as u64);
    write_u64(
        &mut output,
        40,
        package.items.len().saturating_add(package.loadouts.len()) as u64,
    );
    output[64..96].copy_from_slice(blake3::hash(&body).as_bytes());
    write_u64(&mut output, 96, fnv1a_64(logical_path.as_bytes()));
    write_u64(
        &mut output,
        104,
        fnv1a_64(b"newengine.items.package.compiler.v1"),
    );
    write_u64(&mut output, 112, AUTHORED_ITEM_PACKAGE_VERSION as u64);
    output.extend_from_slice(&compressed);
    Ok(output)
}

pub fn decode_authored_item_package_nef8(bytes: &[u8]) -> Result<AuthoredItemPackage, String> {
    let header = parse_list_file_header_v1(bytes)?;
    if header.content_kind != LIST_FILE_CONTENT_KIND_NEITEMS {
        return Err(format!(
            "NEITEMS content kind mismatch: got={} expected={}",
            header.content_kind, LIST_FILE_CONTENT_KIND_NEITEMS
        ));
    }
    let start = usize::try_from(header.body_offset)
        .map_err(|_| "NEITEMS body offset does not fit usize".to_owned())?;
    let length = usize::try_from(header.body_len)
        .map_err(|_| "NEITEMS body length does not fit usize".to_owned())?;
    let end = start
        .checked_add(length)
        .ok_or_else(|| "NEITEMS body range overflow".to_owned())?;
    let compressed = bytes.get(start..end).ok_or_else(|| {
        format!(
            "NEITEMS body range outside file: offset={start} len={length} file={}",
            bytes.len()
        )
    })?;
    let mut decoder = DeflateDecoder::new(compressed);
    let mut body = Vec::with_capacity(header.body_uncompressed_len as usize);
    decoder
        .read_to_end(&mut body)
        .map_err(|error| format!("NEITEMS deflate decode failed: {error}"))?;
    if body.len() != header.body_uncompressed_len as usize {
        return Err(format!(
            "NEITEMS body length mismatch: got={} expected={}",
            body.len(),
            header.body_uncompressed_len
        ));
    }
    if header.body_raw_hash != *blake3::hash(&body).as_bytes() {
        return Err("NEITEMS body BLAKE3 hash mismatch".to_owned());
    }
    parse_authored_item_package_json(&body)
}

pub fn decode_authored_item_package(bytes: &[u8]) -> Result<AuthoredItemPackage, String> {
    if bytes.starts_with(&LIST_FILE_MAGIC_NEF8) {
        decode_authored_item_package_nef8(bytes)
    } else {
        parse_authored_item_package_json(bytes)
    }
}

fn compile_item_definition(authored: &AuthoredItemDefinition) -> Result<ItemDefinition, String> {
    let kind = parse_item_kind(&authored.kind)?;
    let display_name = if authored.display_name.trim().is_empty() {
        authored.id.trim()
    } else {
        authored.display_name.trim()
    };
    let mut definition = match kind {
        ItemKind::Weapon => {
            let weapon = authored
                .weapon
                .as_ref()
                .ok_or_else(|| format!("weapon '{}' has no weapon definition", authored.id))?;
            let ammo_item = ItemId::from_name(&weapon.ammo).ok_or_else(|| {
                format!(
                    "weapon '{}' has invalid ammo id '{}'",
                    authored.id, weapon.ammo
                )
            })?;
            ItemDefinition::weapon(
                &authored.id,
                display_name,
                parse_equipment_slot(&authored.equipment_slot)?,
                weapon.tuning(),
                ammo_item,
                authored.unit_weight,
            )?
        }
        ItemKind::Consumable => ItemDefinition::consumable(
            &authored.id,
            display_name,
            authored.max_stack,
            authored.unit_weight,
            parse_use_effect(authored.use_effect.as_ref())?,
        )?,
        other => ItemDefinition::stackable(
            &authored.id,
            display_name,
            other,
            authored.max_stack,
            authored.unit_weight,
        )?,
    };
    definition = definition
        .with_description(authored.description.trim())
        .with_tags(authored.tags.clone());
    if !authored.icon.trim().is_empty() {
        definition = definition.with_icon(authored.icon.trim());
    }
    if kind != ItemKind::Weapon && !authored.equipment_slot.trim().is_empty() {
        definition.equipment_slot = Some(parse_equipment_slot(&authored.equipment_slot)?);
    }
    if let Some(world) = authored.world.as_ref() {
        definition = definition.with_world_definition(world.compile(kind)?);
    }
    Ok(definition)
}

fn validate_package_header(package: &AuthoredItemPackage) -> Result<(), String> {
    if package.schema != AUTHORED_ITEM_PACKAGE_SCHEMA {
        return Err(format!(
            "item package schema mismatch: got='{}' expected='{}'",
            package.schema, AUTHORED_ITEM_PACKAGE_SCHEMA
        ));
    }
    if package.version != AUTHORED_ITEM_PACKAGE_VERSION {
        return Err(format!(
            "item package version mismatch: got={} expected={}",
            package.version, AUTHORED_ITEM_PACKAGE_VERSION
        ));
    }
    if package.items.is_empty() {
        return Err("item package must contain at least one item".to_owned());
    }
    Ok(())
}

fn parse_item_kind(value: &str) -> Result<ItemKind, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "generic" => Ok(ItemKind::Generic),
        "weapon" => Ok(ItemKind::Weapon),
        "ammo" | "ammunition" => Ok(ItemKind::Ammo),
        "consumable" => Ok(ItemKind::Consumable),
        "component" => Ok(ItemKind::Component),
        "quest" => Ok(ItemKind::Quest),
        "key" => Ok(ItemKind::Key),
        other => Err(format!("unsupported item kind '{other}'")),
    }
}

fn parse_equipment_slot(value: &str) -> Result<EquipmentSlot, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "primary" => Ok(EquipmentSlot::Primary),
        "secondary" => Ok(EquipmentSlot::Secondary),
        "sidearm" => Ok(EquipmentSlot::Sidearm),
        "melee" => Ok(EquipmentSlot::Melee),
        "throwable" => Ok(EquipmentSlot::Throwable),
        "gadget" => Ok(EquipmentSlot::Gadget),
        "utility1" | "utility_1" => Ok(EquipmentSlot::Utility1),
        "utility2" | "utility_2" => Ok(EquipmentSlot::Utility2),
        other => Err(format!("unsupported equipment slot '{other}'")),
    }
}

fn parse_use_effect(effect: Option<&AuthoredUseEffect>) -> Result<ItemUseEffect, String> {
    let Some(effect) = effect else {
        return Ok(ItemUseEffect::None);
    };
    match effect.kind.trim().to_ascii_lowercase().as_str() {
        "" | "none" => Ok(ItemUseEffect::None),
        "heal" => Ok(ItemUseEffect::Heal {
            amount: effect.amount.max(0.0),
        }),
        other => Err(format!("unsupported item use effect '{other}'")),
    }
}

fn write_u16(output: &mut [u8], offset: usize, value: u16) {
    output[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(output: &mut [u8], offset: usize, value: u64) {
    output[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_package() -> AuthoredItemPackage {
        AuthoredItemPackage {
            items: vec![
                AuthoredItemDefinition {
                    id: "ammo.test".to_owned(),
                    display_name: "Test Ammo".to_owned(),
                    kind: "ammo".to_owned(),
                    max_stack: 100,
                    ..AuthoredItemDefinition::default()
                },
                AuthoredItemDefinition {
                    id: "weapon.test".to_owned(),
                    display_name: "Test Rifle".to_owned(),
                    kind: "weapon".to_owned(),
                    equipment_slot: "primary".to_owned(),
                    weapon: Some(AuthoredWeaponDefinition {
                        ammo: "ammo.test".to_owned(),
                        ..AuthoredWeaponDefinition::default()
                    }),
                    ..AuthoredItemDefinition::default()
                },
            ],
            loadouts: vec![AuthoredLoadoutDefinition {
                id: "loadout.test".to_owned(),
                entries: vec![AuthoredLoadoutEntry {
                    item: "weapon.test".to_owned(),
                    quantity: 1,
                    equip: true,
                }],
                ..AuthoredLoadoutDefinition::default()
            }],
            ..AuthoredItemPackage::default()
        }
    }

    #[test]
    fn authored_package_compiles_and_round_trips_through_nef8() {
        let package = sample_package();
        let compiled = compile_authored_item_package(&package).expect("compile package");
        assert_eq!(compiled.catalog.len(), 2);
        assert!(compiled.catalog.find("weapon.test").is_some());

        let encoded = encode_authored_item_package_nef8(&package, "items/test.neitems")
            .expect("encode NEITEMS");
        assert!(encoded.starts_with(&LIST_FILE_MAGIC_NEF8));
        let decoded = decode_authored_item_package_nef8(&encoded).expect("decode NEITEMS");
        assert_eq!(decoded, package);
    }

    #[test]
    fn embedded_fps_package_installs_multi_weapon_primary_loadout() {
        let package = compiled_embedded_fps_item_package().expect("embedded package");
        assert!(package.catalog.find("weapon.rifle.standard").is_some());
        assert!(package.catalog.find("weapon.pistol.standard").is_some());
        let mut world = World::new();
        install_compiled_item_package(&mut world, package);
        let owner = world.spawn();
        crate::gameplay::give_default_fps_loadout(&mut world, owner).expect("default loadout");
        let inventory = world
            .get::<crate::gameplay::PlayerInventory>(owner)
            .expect("inventory");
        assert_eq!(inventory.active_slot, Some(EquipmentSlot::Primary));
        assert!(inventory
            .equipped_instance(EquipmentSlot::Sidearm)
            .is_some());
    }

    #[test]
    fn authored_package_rejects_weapon_with_missing_ammo_definition() {
        let mut package = sample_package();
        package.items.remove(0);
        let error = compile_authored_item_package(&package).expect_err("missing ammo must fail");
        assert!(error.contains("missing ammo"));
    }
}
