use std::collections::BTreeMap;

use newengine_game_data::GameData;
use newengine_scripting_client::AssetBackedScriptClient;

pub(crate) fn load_game_data_from_script(
    script_ref: &str,
    operation: &str,
) -> Result<GameData, String> {
    let client = AssetBackedScriptClient::new(script_ref, "game-data");
    client.load_module()?;
    let data: GameData = client.invoke_json_unit(
        "game-data.bootstrap.v1",
        operation,
        BTreeMap::from([
            (
                "expected_schema".to_owned(),
                newengine_game_data::GAME_DATA_SCHEMA.to_owned(),
            ),
            (
                "expected_version".to_owned(),
                newengine_game_data::GAME_DATA_VERSION.to_string(),
            ),
        ]),
    )?;
    data.validate()
        .map_err(|error| format!("Lua game-data contract validation failed: {error}"))?;
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn authored_fixture() -> GameData {
        let mut data = GameData::default();
        data.runtime.fixed_dt_ms = 16;
        data.runtime.app_name = "provider-test".to_owned();
        data.runtime.app_dir_name = "provider-test".to_owned();
        data.runtime.window_title = "Provider Test".to_owned();
        data.runtime.default_profile_asset = "maps/test.ymap".to_owned();
        data.player.spawn = [0.0, 1.0, 0.0];
        data.player.look_sensitivity = 0.002;
        data.player.character_ref = "definitions/test/player.ytyp@player".to_owned();
        data.world.sky.definition_ref = "definitions/test/sky.ytyp@sky".to_owned();
        data.world.sky.radius = 1.0;
        data.world.shadows.filter = "hard".to_owned();
        data.gameplay.projectile.radius = 0.1;
        data.gameplay.projectile.lifetime_seconds = 1.0;
        data.gameplay.inventory.hud_slots = 1;
        data
    }

    #[test]
    fn game_data_payload_round_trips_through_json_contract() {
        let expected = authored_fixture();
        expected.validate().unwrap();
        let payload = serde_json::to_vec(&expected).unwrap();
        let decoded: GameData = serde_json::from_slice(&payload).unwrap();
        decoded.validate().unwrap();
        assert_eq!(decoded, expected);
    }

    #[test]
    fn unconfigured_default_is_not_a_provider_payload() {
        assert!(GameData::default().validate().is_err());
    }

    #[test]
    fn malformed_game_data_payload_is_rejected() {
        let decoded = serde_json::from_slice::<GameData>(br#"{\"schema\":\"wrong\"}"#);
        assert!(decoded.is_err() || decoded.unwrap().validate().is_err());
    }
}
