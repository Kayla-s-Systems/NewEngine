pub fn apply_game_ready_ytyp_metadata(
    profile: &mut AuthoredWorldProfile,
    game_data: &mut GameData,
) {
    let definitions = profile.definitions.clone();
    if definitions.is_empty() {
        newengine_ulog_api::ulog::warn!(
            "game-ready ytyp metadata: scene profile declares no .ytyp Definition Entries; runtime will use sanitized profile defaults"
        );
        return;
    }

    let mut applied_total = 0usize;
    for spec in definitions {
        let definition_ref = spec.definition_ref.trim();
        let Some(entry) = load_game_ready_definition_entry(definition_ref) else {
            continue;
        };
        let null_metadata = serde_json::Value::Null;
        let metadata = game_ready_metadata_namespace(&entry).unwrap_or(&null_metadata);
        let mut audio_applied = 0usize;
        if let Some(audio_metadata) = audio_metadata_namespace(&entry) {
            if let Some(library) = acoustic_material_library_from_ytyp(audio_metadata) {
                let count = library.rules.len();
                merge_acoustic_material_library(&mut profile.acoustic_materials, library);
                audio_applied = count;
                newengine_ulog_api::ulog::info!(
                    "game-ready ytyp audio metadata: definition_ref='{}' acoustic_material_rules={} total_rules={} policy='Shared baseline first; later definitions replace matching surface rules'",
                    definition_ref,
                    count,
                    profile.acoustic_materials.rules.len(),
                );
            }
        }
        if metadata.is_null() {
            newengine_ulog_api::ulog::debug!(
                "game-ready ytyp metadata: definition_ref='{}' has no newengine.game_ready namespace; graph-only definition",
                definition_ref
            );
        }
        let applied = audio_applied
            + apply_render_options_from_ytyp(profile, &entry, definition_ref)
            + apply_sky_drawable_from_ytyp(profile, &entry, definition_ref)
            + apply_material_refs_from_ytyp(profile, metadata, definition_ref)
            + apply_texture_refs_from_ytyp(profile, metadata, definition_ref)
            + apply_player_model_from_ytyp(profile, metadata, definition_ref)
            + apply_player_runtime_data_from_ytyp(profile, game_data, metadata)
            + apply_gameplay_constants_from_ytyp(profile, metadata)
            + apply_sky_constants_from_ytyp(profile, metadata)
            + apply_time_constants_from_ytyp(profile, metadata);
        applied_total += applied;
        newengine_ulog_api::ulog::info!(
            "game-ready ytyp metadata: consumed definition_ref='{}' applied_constants={} policy='metadata constants from engine.assets.definitions; no runtime json material source'",
            definition_ref,
            applied
        );
    }

    newengine_ulog_api::ulog::info!(
        "game-ready ytyp metadata: completed definitions={} applied_constants={} chain='.ytyp -> .ytyd -> .ydd -> .nemat -> .ytd'",
        profile.definitions.len(),
        applied_total
    );
}

pub fn resolve_game_ready_asset_graph(
    root_ref: &str,
) -> Option<newengine_model_domain_api::ResolvedAssetGraphV2> {
    let payload = serde_json::to_vec(&serde_json::json!({ "root_ref": root_ref })).ok()?;
    match call_service_v1_optional(
        newengine_model_domain_api::ENGINE_ASSETS_GRAPH_SERVICE_ID,
        newengine_model_domain_api::ASSET_GRAPH_METHOD_RESOLVE_V1,
        &payload,
    ) {
        Ok(Some(bytes)) => {
            match serde_json::from_slice::<newengine_model_domain_api::ResolvedAssetGraphV2>(&bytes)
            {
                Ok(graph) => Some(graph),
                Err(e) => {
                    newengine_ulog_api::ulog::warn!(
                        "assets.graph.resolve_v1: invalid json graph root_ref='{}' err='{}'",
                        root_ref,
                        e
                    );
                    None
                }
            }
        }
        Ok(None) => {
            newengine_ulog_api::ulog::debug!(
                "assets.graph.resolve_v1: route absent root_ref='{}'; graph hydration skipped",
                root_ref
            );
            None
        }
        Err(e) => {
            newengine_ulog_api::ulog::warn!(
                "assets.graph.resolve_v1: gateway call failed root_ref='{}' err='{}'",
                root_ref,
                e
            );
            None
        }
    }
}


/// Generic FPS authored-profile alias retained alongside the historical symbol.
pub use apply_game_ready_ytyp_metadata as apply_authored_fps_ytyp_metadata;
/// Generic FPS authored asset-graph alias retained alongside the historical symbol.
pub use resolve_game_ready_asset_graph as resolve_authored_fps_asset_graph;
