#![forbid(unsafe_op_in_unsafe_fn)]

//! Central data-only game configuration.
//!
//! Runtime/gameplay systems consume values through a snapshot/provider boundary.

mod defaults;
mod provider;
mod schema;

pub use provider::*;
pub use schema::*;

pub const GAME_DATA_SCHEMA: &str = "newengine.game_data.v1";
pub const GAME_DATA_VERSION: u32 = 1;

pub const GAME_FIXED_DT_MS: u32 = 16;
pub const GAME_APP_ASSETS_DIR_ENV: &str = "NEWENGINE_GAME_ASSETS_DIR";
pub const GAME_READY_APP_DIR_NAME: &str = "game-ready-fps";
pub const GAME_READY_FPS_APP_NAME: &str = "game-ready-fps";
pub const GAME_READY_FPS_WINDOW_TITLE: &str = "North Star Game Ready FPS";
pub const GAME_READY_FPS_EARLY_LOG_FILE: &str = "game-ready-fps-early.log";
pub const GAME_READY_PROFILE_ENV: &str = "NEWENGINE_SCENE_PROFILE";
pub const GAME_READY_DEFAULT_PROFILE_ASSET: &str = "maps/white_platform.ymap";

pub const DEFAULT_RIFLE_ITEM_NAME: &str = "weapon.rifle.standard";
pub const DEFAULT_RIFLE_AMMO_NAME: &str = "ammo.rifle.standard";
pub const DEFAULT_MEDKIT_ITEM_NAME: &str = "consumable.medkit.standard";
pub const DEFAULT_FPS_LOADOUT_NAME: &str = "loadout.fps.default";
pub const DEFAULT_ITEM_PACKAGE_ASSET: &str = "items/fps_items.neitems";
pub const WORLD_MATERIAL_LIBRARY: &str = "materials/world_game_ready.nemat";
pub const MISSION_MATERIAL_LIBRARY: &str = "materials/world_game_ready.nemat";

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn rust_defaults_are_single_source_and_lua_serializable() {
        let data = default_game_data();
        assert_eq!(data.schema, GAME_DATA_SCHEMA);
        assert_eq!(data.version, GAME_DATA_VERSION);
        assert_eq!(data.runtime.fixed_dt_ms, GAME_FIXED_DT_MS);
        assert_eq!(data.player.tuning.gravity, 9.81);
        assert_eq!(data.gameplay.inventory.loadout, DEFAULT_FPS_LOADOUT_NAME);
    }

    #[test]
    fn contract_validation_rejects_non_finite_provider_data() {
        let mut data = GameData::default();
        data.player.move_speed = f32::NAN;
        assert!(data.validate().is_err());
    }

    #[test]
    fn snapshot_keeps_provider_identity_and_shared_immutable_data() {
        let snapshot = GameDataSnapshot::rust_defaults();
        assert_eq!(snapshot.source_id(), "newengine.game_data.rust_defaults");
        assert_eq!(snapshot.data().version, GAME_DATA_VERSION);
        let a = snapshot.shared();
        let b = snapshot.shared();
        assert!(Arc::ptr_eq(&a, &b));
    }
}
