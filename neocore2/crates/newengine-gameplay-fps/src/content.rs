use std::sync::Arc;

use newengine_engine_runtime::gameplay::{
    apply_loadout, GameplayContentProvider, GameplayWorld, InventoryLoadoutCatalog, ItemCatalog,
    ItemId, PlayerController, PlayerInventory,
};

use crate::item_assets::{compiled_embedded_fps_item_package, install_compiled_item_package};

pub const DEFAULT_RIFLE_ITEM_NAME: &str = "weapon.rifle.standard";
pub const DEFAULT_RIFLE_AMMO_NAME: &str = "ammo.rifle.standard";
pub const DEFAULT_MEDKIT_ITEM_NAME: &str = "consumable.medkit.standard";
pub const DEFAULT_FPS_LOADOUT_NAME: &str = "loadout.fps.default";

#[inline]
pub fn default_rifle_item_id() -> ItemId {
    ItemId::from_name(DEFAULT_RIFLE_ITEM_NAME).expect("valid FPS item name")
}

#[inline]
pub fn default_rifle_ammo_id() -> ItemId {
    ItemId::from_name(DEFAULT_RIFLE_AMMO_NAME).expect("valid FPS ammo name")
}

#[inline]
pub fn default_medkit_item_id() -> ItemId {
    ItemId::from_name(DEFAULT_MEDKIT_ITEM_NAME).expect("valid FPS item name")
}

#[inline]
pub fn default_fps_loadout_id() -> ItemId {
    ItemId::from_name(DEFAULT_FPS_LOADOUT_NAME).expect("valid FPS loadout name")
}

/// Installs authored FPS inventory content. There is deliberately no built-in Rust fallback:
/// a broken/missing authored package is a content error surfaced by the provider registry.
pub struct FpsContentProvider;

impl FpsContentProvider {
    #[inline]
    pub fn shared() -> Arc<Self> {
        Arc::new(Self)
    }
}

impl GameplayContentProvider for FpsContentProvider {
    #[inline]
    fn id(&self) -> &'static str {
        "newengine.gameplay.fps.content"
    }

    fn install(&self, world: &mut GameplayWorld) -> Result<(), String> {
        let package = compiled_embedded_fps_item_package()?;
        if package.catalog.find(DEFAULT_RIFLE_ITEM_NAME).is_none() {
            return Err(format!(
                "authored FPS item package is missing required item '{}'",
                DEFAULT_RIFLE_ITEM_NAME
            ));
        }
        if package.catalog.find(DEFAULT_RIFLE_AMMO_NAME).is_none() {
            return Err(format!(
                "authored FPS item package is missing required item '{}'",
                DEFAULT_RIFLE_AMMO_NAME
            ));
        }
        if package.catalog.find(DEFAULT_MEDKIT_ITEM_NAME).is_none() {
            return Err(format!(
                "authored FPS item package is missing required item '{}'",
                DEFAULT_MEDKIT_ITEM_NAME
            ));
        }
        if package.loadouts.get(default_fps_loadout_id()).is_none() {
            return Err(format!(
                "authored FPS item package is missing required loadout '{}'",
                DEFAULT_FPS_LOADOUT_NAME
            ));
        }
        install_compiled_item_package(world, package);
        Ok(())
    }

    fn content_is_present(&self, world: &GameplayWorld) -> bool {
        world.resource::<ItemCatalog>().is_some_and(|catalog| {
            catalog.find(DEFAULT_RIFLE_ITEM_NAME).is_some()
                && catalog.find(DEFAULT_RIFLE_AMMO_NAME).is_some()
                && catalog.find(DEFAULT_MEDKIT_ITEM_NAME).is_some()
        }) && world
            .resource::<InventoryLoadoutCatalog>()
            .is_some_and(|loadouts| loadouts.get(default_fps_loadout_id()).is_some())
    }
}

pub(crate) fn ensure_fps_player_loadouts(world: &mut GameplayWorld) {
    if world.resource::<ItemCatalog>().is_none()
        || world.resource::<InventoryLoadoutCatalog>().is_none()
    {
        return;
    }

    let players = world
        .query::<PlayerController>()
        .map(|(entity, _)| entity)
        .collect::<Vec<_>>();

    for player in players {
        let Some(inventory) = world.get::<PlayerInventory>(player) else {
            continue;
        };
        if inventory.loadout_initialized() {
            continue;
        }
        if !inventory.entries.is_empty() {
            if let Some(inventory) = world.get_mut::<PlayerInventory>(player) {
                inventory.mark_loadout_initialized();
            }
            continue;
        }
        let _ = apply_loadout(world, player, default_fps_loadout_id());
    }
}
