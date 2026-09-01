#![forbid(unsafe_op_in_unsafe_fn)]

//! Project-authored runtime/game configuration contract.
//!
//! The crate defines schema/validation/snapshot mechanics only. It does not ship a playable game,
//! character, loadout, asset path, mission text, or tuning profile.

mod defaults;
mod provider;
mod schema;

pub use provider::*;
pub use schema::*;

pub const GAME_DATA_SCHEMA: &str = "newengine.game_data.v2";
pub const GAME_DATA_VERSION: u32 = 2;

// Product/runtime-launch compatibility constants. These are not gameplay/content defaults.
// They are scheduled for migration into the project launch descriptor; no GameData field derives
// a value from them.
pub const GAME_FIXED_DT_MS: u32 = 16;
pub const GAME_APP_ASSETS_DIR_ENV: &str = "NEWENGINE_GAME_ASSETS_DIR";
pub const GAME_READY_APP_DIR_NAME: &str = "game-ready-fps";
pub const GAME_READY_FPS_APP_NAME: &str = "game-ready-fps";
pub const GAME_READY_FPS_WINDOW_TITLE: &str = "North Star Game Ready FPS";
pub const GAME_READY_FPS_EARLY_LOG_FILE: &str = "game-ready-fps-early.log";
pub const GAME_READY_PROFILE_ENV: &str = "NEWENGINE_SCENE_PROFILE";
pub const GAME_READY_DEFAULT_PROFILE_ASSET: &str = "maps/white_platform.ymap";

#[cfg(test)]
mod tests {
    use super::*;

    fn project_fixture() -> GameData {
        let mut data = GameData::default();
        data.runtime.fixed_dt_ms = 16;
        data.runtime.app_name = "test-project".to_owned();
        data.runtime.app_dir_name = "test-project".to_owned();
        data.runtime.window_title = "Test Project".to_owned();
        data.runtime.default_profile_asset = "maps/test.ymap".to_owned();
        data.audio.mix_graph.buses.push(newengine_audio_api::AudioMixBusSpec {
            id: newengine_audio_api::AudioRouteId::new("test.output"),
            parent: None,
            gain_db: 0.0,
        });
        data.player.spawn = [0.0, 1.0, 0.0];
        data.player.yaw = 0.0;
        data.player.look_sensitivity = 0.002;
        data.player.character_ref = "definitions/test/player.ytyp@player".to_owned();
        data.world.sky.definition_ref = "definitions/test/sky.ytyp@sky".to_owned();
        data.world.sky.radius = 1.0;
        data.world.shadows.filter = "hard".to_owned();
        data.gameplay.projectile.radius = 0.1;
        data.gameplay.projectile.speed = 0.0;
        data.gameplay.projectile.lifetime_seconds = 1.0;
        data.gameplay.inventory.hud_slots = 1;
        data
    }

    #[test]
    fn default_is_unconfigured_and_never_a_shipping_game() {
        let data = GameData::default();
        assert_eq!(data.schema, GAME_DATA_SCHEMA);
        assert_eq!(data.version, GAME_DATA_VERSION);
        assert!(data.validate().is_err());
        assert!(data.player.character_ref.is_empty());
        assert!(data.player.model.source.is_empty());
        assert_eq!(data.player.tuning, PlayerTuningData::default());
    }

    #[test]
    fn project_fixture_validates_without_embedded_character_or_loadout_defaults() {
        let data = project_fixture();
        data.validate().unwrap();
        let value = serde_json::to_value(&data).unwrap();
        let player = value["player"].as_object().unwrap();
        assert!(!player.contains_key("move_speed"));
        assert!(!player.contains_key("model"));
        assert!(!player.contains_key("tuning"));
        let inventory = value["gameplay"]["inventory"].as_object().unwrap();
        assert_eq!(inventory.len(), 1);
        assert!(inventory.contains_key("hud_slots"));
    }

    #[test]
    fn v1_content_identity_fields_are_rejected_in_v2() {
        let mut value = serde_json::to_value(project_fixture()).unwrap();
        value["gameplay"]["inventory"]["rifle_item"] =
            serde_json::Value::String("weapon.anything".to_owned());
        assert!(serde_json::from_value::<GameData>(value).is_err());
    }

    #[test]
    fn character_definition_is_project_mandatory() {
        let mut data = project_fixture();
        data.player.character_ref.clear();
        assert!(data.validate().is_err());
    }

    #[test]
    fn runtime_resolved_character_fields_do_not_cross_json_boundary() {
        let mut data = project_fixture();
        data.player.move_speed = 99.0;
        data.player.model.enabled = true;
        data.player.model.source = "should/not/serialize.ydd@x".to_owned();
        data.player.tuning.gravity = 123.0;
        let payload = serde_json::to_vec(&data).unwrap();
        let decoded: GameData = serde_json::from_slice(&payload).unwrap();
        assert_eq!(decoded.player.move_speed, 0.0);
        assert_eq!(decoded.player.model, PlayerModelData::default());
        assert_eq!(decoded.player.tuning, PlayerTuningData::default());
        decoded.validate().unwrap();
    }
}
