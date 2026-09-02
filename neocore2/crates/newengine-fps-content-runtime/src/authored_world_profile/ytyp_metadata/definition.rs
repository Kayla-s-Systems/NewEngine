pub fn load_game_ready_definition_entry(definition_ref: &str) -> Option<serde_json::Value> {
    let payload =
        serde_json::to_vec(&serde_json::json!({ "definition_ref": definition_ref })).ok()?;
    match call_service_v1_optional(
        newengine_assets::ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
        newengine_assets::definitions_method::ENTRY_JSON_V1,
        &payload,
    ) {
        Ok(Some(bytes)) => match serde_json::from_slice::<serde_json::Value>(&bytes) {
            Ok(value) => Some(value),
            Err(e) => {
                newengine_ulog_api::ulog::warn!("game-ready ytyp metadata: engine.assets.definitions returned invalid json ref='{}' err='{}'", definition_ref, e);
                None
            }
        },
        Ok(None) => {
            newengine_ulog_api::ulog::debug!("game-ready ytyp metadata: engine.assets.definitions route absent ref='{}'; metadata hydration skipped", definition_ref);
            None
        }
        Err(e) => {
            newengine_ulog_api::ulog::warn!(
                "game-ready ytyp metadata: engine.assets.definitions call failed ref='{}' err='{}'",
                definition_ref,
                e
            );
            None
        }
    }
}

fn metadata_namespace<'a>(
    entry: &'a serde_json::Value,
    namespace: &str,
) -> Option<&'a serde_json::Value> {
    entry
        .get("arbitrary_metadata")
        .and_then(|v| v.get("metadata"))
        .and_then(|v| v.get(namespace))
        .or_else(|| {
            entry
                .get("arbitrary_metadata")
                .and_then(|v| v.get("namespaces"))
                .and_then(|v| v.get(namespace))
        })
        .or_else(|| {
            entry
                .get("arbitrary_metadata")
                .and_then(|v| v.get(namespace))
        })
        .or_else(|| entry.get("metadata").and_then(|v| v.get(namespace)))
        .or_else(|| entry.get("namespaces").and_then(|v| v.get(namespace)))
}

pub fn load_character_model_assignment(
    definition_ref: &str,
) -> Result<newengine_engine_runtime::gameplay::PlayerModelAssignment, String> {
    let definition_ref = definition_ref.trim();
    if definition_ref.is_empty() || !definition_ref.to_ascii_lowercase().contains(".ytyp@") {
        return Err(
            "character definition_ref must be an authored .ytyp@entry reference".to_owned(),
        );
    }
    let entry = load_game_ready_definition_entry(definition_ref)
        .ok_or_else(|| format!("character definition unavailable ref='{definition_ref}'"))?;
    let metadata = metadata_namespace(&entry, "newengine.game_ready").ok_or_else(|| {
        format!("character definition has no newengine.game_ready namespace ref='{definition_ref}'")
    })?;
    player::character_model_assignment_from_ytyp_metadata(metadata, definition_ref).ok_or_else(
        || {
            format!(
                "character definition has no authored model/idle contract ref='{definition_ref}'"
            )
        },
    )
}
