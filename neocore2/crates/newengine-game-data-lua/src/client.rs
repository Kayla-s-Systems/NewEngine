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

    #[test]
    fn game_data_payload_round_trips_through_json_contract() {
        let expected = GameData::default();
        let payload = serde_json::to_vec(&expected).unwrap();
        let decoded: GameData = serde_json::from_slice(&payload).unwrap();
        decoded.validate().unwrap();
        assert_eq!(decoded, expected);
    }

    #[test]
    fn malformed_game_data_payload_is_rejected() {
        let decoded = serde_json::from_slice::<GameData>(br#"{\"schema\":\"wrong\"}"#);
        assert!(decoded.is_err() || decoded.unwrap().validate().is_err());
    }
}
