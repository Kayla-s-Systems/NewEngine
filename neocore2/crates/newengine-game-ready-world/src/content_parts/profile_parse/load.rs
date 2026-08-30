use super::*;

use super::super::super::paths::profile_asset_candidates;
use super::super::ymap_read_diagnostics::log_ymap_value_summary;
use super::xml::{parse_map_definition_payload, parse_payload, parse_ymap_xml_payload};
use newengine_assets::{AssetDecodeRequest, ASSET_LIST_FILE_BODY_OUTPUT};
use newengine_authored_xml as authored_xml;

pub(crate) fn load_game_ready_map_profile() -> Result<GameReadyMapProfile, Vec<String>> {
    load_profile_from_asset_manager()
}

fn load_profile_from_asset_manager() -> Result<GameReadyMapProfile, Vec<String>> {
    use newengine_assets::AssetService;

    if !newengine_core::has_engine_gateway_route(newengine_assets_api::ENGINE_ASSET_SERVICE_ID) {
        newengine_ulog_api::ulog::debug!(
            "game-ready: AssetManager service '{}' unavailable while resolving authored map",
            newengine_assets_api::ENGINE_ASSET_SERVICE_ID
        );
        return Err(vec![format!(
            "AssetManager service '{}' unavailable while resolving authored map",
            newengine_assets_api::ENGINE_ASSET_SERVICE_ID
        )]);
    }

    let assets =
        newengine_assets::AssetServiceClient::new(newengine_plugin_host::default_host_api());
    let candidates = profile_asset_candidates();
    newengine_ulog_api::ulog::info!(
        "game-ready ymap read: begin gateway='{}' candidates={} mount_policy='profile-owned VFS mounts already established' decode_policy='AssetManager decode_v1 only'",
        newengine_assets_api::ENGINE_ASSET_SERVICE_ID,
        candidates.len(),
    );

    let mut errors = Vec::new();
    for (index, logical_path) in candidates.into_iter().enumerate() {
        newengine_ulog_api::ulog::info!(
            "game-ready ymap read: candidate begin index={} path='{}'",
            index,
            logical_path,
        );
        match load_profile_asset(&assets, &logical_path) {
            Ok(profile) => {
                let trace = assets
                    .resolve_trace_json_v1(&logical_path)
                    .map(|v| v.to_string())
                    .unwrap_or_else(|te| format!("{{\"trace_error\":\"{te}\"}}"));
                newengine_ulog_api::ulog::info!(
                    "game-ready ymap read: candidate selected index={} path='{}' trace={}",
                    index,
                    logical_path,
                    trace,
                );
                newengine_ulog_api::ulog::info!(
                    "game-ready: loaded authored map asset='{}'",
                    logical_path,
                );
                return Ok(profile);
            }
            Err(e) => {
                let trace = assets
                    .resolve_trace_json_v1(&logical_path)
                    .map(|v| v.to_string())
                    .unwrap_or_else(|te| format!("{{\"trace_error\":\"{te}\"}}"));
                let message = format!("path='{logical_path}' err='{e}' trace={trace}");
                newengine_ulog_api::ulog::info!(
                    "game-ready ymap read: candidate rejected index={} {}",
                    index,
                    message
                );
                errors.push(message);
            }
        }
    }

    Err(errors)
}

fn load_profile_asset(
    assets: &newengine_assets::AssetServiceClient,
    logical_path: &str,
) -> Result<GameReadyMapProfile, String> {
    if !logical_path
        .to_ascii_lowercase()
        .split('@')
        .next()
        .unwrap_or(logical_path)
        .ends_with(&format!(
            ".{}",
            newengine_asset_format_nef8::ymap::EXTENSION
        ))
    {
        return Err(format!(
            "non-canonical authored map rejected path='{logical_path}' expected='.{}' policy='authored maps are NEF8/ListFile, not runtime plain JSON'", newengine_asset_format_nef8::ymap::EXTENSION
        ));
    }

    newengine_ulog_api::ulog::info!(
        "game-ready ymap read: canonical accepted path='{}' extension='{}'",
        logical_path,
        newengine_asset_format_nef8::ymap::EXTENSION,
    );

    let output_kind = ASSET_LIST_FILE_BODY_OUTPUT;
    let request = AssetDecodeRequest {
        logical_path: logical_path.to_owned(),
        output_kind: output_kind.to_owned(),
        selector: serde_json::Value::Null,
    };
    newengine_ulog_api::ulog::info!(
        "game-ready ymap read: decode start path='{}' output='{}' selector=null",
        logical_path,
        output_kind,
    );
    let payload = assets.decode_v1(&request).map_err(|e| {
        format!("asset.decode_v1 failed path='{logical_path}' output='{output_kind}' err='{e}'")
    })?;
    newengine_ulog_api::ulog::info!(
        "game-ready ymap read: decode complete path='{}' output='{}' payload_bytes={}",
        logical_path,
        output_kind,
        payload.len(),
    );
    if !authored_xml::body_is_xml(&payload) {
        return Err(format!(
            "ymap body must be XML path='{logical_path}' output='{output_kind}' payload_bytes={} policy='authored map metadata uses XML presentation inside NEF8; JSON runtime map bodies are forbidden'",
            payload.len()
        ));
    }
    if ymap_schema(&payload, logical_path)?.as_str() == "newengine.map.definition.v2" {
        return load_discrete_map_profile(logical_path);
    }
    let value = parse_ymap_xml_payload(&payload, logical_path)?;
    log_ymap_value_summary(logical_path, &value);
    newengine_ulog_api::ulog::info!(
        "game-ready: decoded authored .ymap path='{}' output='{}' policy='NEF8/ListFile body from engine.assets; XML map semantics stay outside AssetManager'",
        logical_path,
        output_kind
    );
    parse_map_definition_payload(value, logical_path)
}

fn ymap_schema(payload: &[u8], logical_path: &str) -> Result<String, String> {
    let text = std::str::from_utf8(payload)
        .map_err(|e| format!("ymap XML body is not UTF-8 path='{logical_path}' err='{e}'"))?;
    let doc = authored_xml::parse_xml_document(text, &format!("ymap path='{logical_path}'"))?;
    let root = doc.root_element();
    if !root.has_tag_name("YmapMapDefinition") && !root.has_tag_name("MapDefinition") {
        return Err(format!(
            "ymap XML root must be <YmapMapDefinition> path='{logical_path}' actual='{}'",
            root.tag_name().name()
        ));
    }
    Ok(root
        .attribute("schema")
        .unwrap_or_default()
        .trim()
        .to_owned())
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
struct ResolvedMapDefinitionRefs {
    drawable_refs: Vec<String>,
    material_refs: Vec<String>,
    collision_refs: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
struct ResolvedMapDefinitionModelExplanation {
    collision_policy: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
struct ResolvedMapDefinitionEntry {
    refs: ResolvedMapDefinitionRefs,
    semantic_tags: Vec<String>,
    model_explanation: ResolvedMapDefinitionModelExplanation,
}

fn load_resolved_map_definition(
    definition_ref: &str,
) -> Result<ResolvedMapDefinitionEntry, String> {
    let payload = serde_json::to_vec(&serde_json::json!({ "definition_ref": definition_ref }))
        .map_err(|e| {
            format!(
                "discrete YMAP definition request encode failed definition_ref='{definition_ref}' err='{e}'"
            )
        })?;
    let bytes = newengine_core::call_service_v1_optional(
        newengine_assets_api::ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
        newengine_assets_api::definitions_method::ENTRY_JSON_V1,
        &payload,
    )
    .map_err(|e| {
        format!(
            "engine.assets.definitions request failed definition_ref='{definition_ref}' err='{e}'"
        )
    })?
    .ok_or_else(|| {
        format!("engine.assets.definitions route unavailable definition_ref='{definition_ref}'")
    })?;
    serde_json::from_slice(&bytes).map_err(|e| {
        format!(
            "engine.assets.definitions returned invalid definition DTO definition_ref='{definition_ref}' err='{e}'"
        )
    })
}

fn cell_distance(
    a: newengine_assets_api::MapCellCoordV1,
    b: newengine_assets_api::MapCellCoordV1,
) -> i32 {
    (a.x - b.x).abs().max((a.z - b.z).abs())
}

fn existing_cells_within_radius(
    index: &newengine_assets_api::MapIndexV1,
    center: newengine_assets_api::MapCellCoordV1,
    radius: i32,
) -> Vec<newengine_assets_api::MapCellCoordV1> {
    let radius = radius.max(0);
    let mut cells = Vec::new();
    for dz in -radius..=radius {
        for dx in -radius..=radius {
            let coord = newengine_assets_api::MapCellCoordV1::new(
                center.x.saturating_add(dx),
                center.z.saturating_add(dz),
            );
            if index.cell(coord).is_some() {
                cells.push(coord);
            }
        }
    }
    cells.sort_by_key(|coord| (cell_distance(*coord, center), coord.x, coord.z));
    cells
}

fn metadata_i32(index: &newengine_assets_api::MapIndexV1, key: &str, default: i32) -> i32 {
    index
        .metadata
        .get(key)
        .and_then(|value| value.trim().parse::<i32>().ok())
        .unwrap_or(default)
}

fn metadata_usize(index: &newengine_assets_api::MapIndexV1, key: &str, default: usize) -> usize {
    index
        .metadata
        .get(key)
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(default)
}

fn load_discrete_cell(
    map_ref: &str,
    coord: newengine_assets_api::MapCellCoordV1,
) -> Result<newengine_assets_api::MapResolvedCellV2, String> {
    let cell_request = serde_json::to_vec(&newengine_assets_api::MapCellRequestV1 {
        map_ref: map_ref.to_owned(),
        coord,
    })
    .map_err(|e| format!("discrete YMAP cell request encode failed map='{map_ref}' err='{e}'"))?;
    let cell_bytes = newengine_core::call_service_v1_optional(
        newengine_assets_api::ENGINE_ASSETS_MAPS_SERVICE_ID,
        newengine_assets_api::maps_method::CELL_V2,
        &cell_request,
    )
    .map_err(|e| {
        format!(
            "engine.assets.maps cell request failed map='{map_ref}' cell={},{} err='{e}'",
            coord.x, coord.z
        )
    })?
    .ok_or_else(|| {
        format!(
            "engine.assets.maps route unavailable while loading map='{map_ref}' cell={},{}",
            coord.x, coord.z
        )
    })?;
    serde_json::from_slice(&cell_bytes).map_err(|e| {
        format!(
            "engine.assets.maps returned invalid MapResolvedCellV2 map='{map_ref}' cell={},{} err='{e}'",
            coord.x, coord.z
        )
    })
}

fn apply_discrete_placement(
    profile: &mut GameReadyMapProfile,
    definition_cache: &mut std::collections::BTreeMap<String, ResolvedMapDefinitionEntry>,
    logical_path: &str,
    coord: newengine_assets_api::MapCellCoordV1,
    include_render: bool,
    include_simulation: bool,
    placement: newengine_assets_api::MapPlacementV1,
) -> Result<(), String> {
    let authored_player_spawn = placement.tags.iter().any(|tag| {
        matches!(
            tag.trim().to_ascii_lowercase().as_str(),
            "player_spawn" | "info_player_start" | "spawn.player"
        )
    }) || matches!(
        placement.apply_mode.trim().to_ascii_lowercase().as_str(),
        "player_spawn" | "info_player_start"
    );
    if authored_player_spawn {
        profile.player.start = Vec3::new(
            placement.transform.position[0],
            placement.transform.position[1],
            placement.transform.position[2],
        );
        profile.player.yaw = placement.transform.rotation_ypr[0];
        newengine_ulog_api::ulog::info!(
            "game-ready: authored player spawn selected id='{}' position={:?} yaw={:.3} policy='YMAP spawn marker owns map start position'",
            placement.id,
            profile.player.start,
            profile.player.yaw,
        );
        return Ok(());
    }

    if placement
        .apply_mode
        .trim()
        .eq_ignore_ascii_case("metadata_only")
    {
        profile.definitions.push(GameReadyDefinitionInstanceSpec {
            definition_ref: placement.definition_ref,
            position: Vec3::new(
                placement.transform.position[0],
                placement.transform.position[1],
                placement.transform.position[2],
            ),
            rotation_ypr: placement.transform.rotation_ypr,
            scale: Vec3::new(
                placement.transform.scale[0],
                placement.transform.scale[1],
                placement.transform.scale[2],
            ),
            apply_mode: GameReadyDefinitionApplyMode::MetadataOnly,
        });
        return Ok(());
    }

    let definition = if let Some(existing) = definition_cache.get(&placement.definition_ref) {
        existing.clone()
    } else {
        let parsed = load_resolved_map_definition(&placement.definition_ref).map_err(|e| {
            format!(
                "discrete YMAP placement '{}' definition_ref='{}' resolution failed: {e}",
                placement.id, placement.definition_ref
            )
        })?;
        definition_cache.insert(placement.definition_ref.clone(), parsed.clone());
        parsed
    };

    let drawable_ref = definition
        .refs
        .drawable_refs
        .first()
        .cloned()
        .ok_or_else(|| {
            format!(
                "discrete YMAP placement '{}' definition_ref='{}' has no drawable_refs",
                placement.id, placement.definition_ref
            )
        })?;
    let material_ref = definition
        .refs
        .material_refs
        .first()
        .cloned()
        .unwrap_or_default();
    let position = Vec3::new(
        placement.transform.position[0],
        placement.transform.position[1],
        placement.transform.position[2],
    );
    let scale = Vec3::new(
        placement.transform.scale[0],
        placement.transform.scale[1],
        placement.transform.scale[2],
    );
    let rotation_ypr = Vec3::new(
        placement.transform.rotation_ypr[0],
        placement.transform.rotation_ypr[1],
        placement.transform.rotation_ypr[2],
    );
    let apply_mode = placement.apply_mode.trim();
    let dynamic_physics = apply_mode.eq_ignore_ascii_case("dynamic_physics");
    let collision_only = apply_mode.eq_ignore_ascii_case("collision_only")
        || placement
            .tags
            .iter()
            .any(|tag| tag.eq_ignore_ascii_case("collision_only"));

    if !collision_only
        && ((!dynamic_physics && include_render) || (dynamic_physics && include_simulation))
    {
        profile.prefabs.push(GameReadyPrefabSpec {
            id: placement.id.clone(),
            authored_map_ref: logical_path.to_owned(),
            authored_placement_id: placement.id.clone(),
            authored_cell: Some(coord),
            authored_discrete_placement: true,
            authored_primary: true,
            source: drawable_ref.clone(),
            proxy: if dynamic_physics {
                "world_dynamic_ydd".to_owned()
            } else {
                "world_static_ydd".to_owned()
            },
            material: material_ref,
            enabled: true,
            position,
            rotation_ypr,
            scale,
        });
    }

    let collision_policy = definition.model_explanation.collision_policy.trim();
    let has_collision = !definition.refs.collision_refs.is_empty()
        || definition
            .semantic_tags
            .iter()
            .any(|tag| tag.eq_ignore_ascii_case("collision"))
        || matches!(
            collision_policy.to_ascii_lowercase().as_str(),
            "static_mesh" | "triangle_mesh" | "mesh" | "box"
        );
    if has_collision && !dynamic_physics && include_simulation {
        let collision_source = definition
            .refs
            .collision_refs
            .first()
            .cloned()
            .unwrap_or(drawable_ref);
        profile.prefabs.push(GameReadyPrefabSpec {
            id: if collision_only {
                placement.id.clone()
            } else {
                format!("{}#collision", placement.id)
            },
            authored_map_ref: logical_path.to_owned(),
            authored_placement_id: placement.id.clone(),
            authored_cell: Some(coord),
            authored_discrete_placement: true,
            authored_primary: false,
            source: collision_source,
            proxy: if collision_policy.eq_ignore_ascii_case("box") {
                "world_collision_box".to_owned()
            } else {
                "world_collision_ydd".to_owned()
            },
            material: String::new(),
            enabled: true,
            position,
            rotation_ypr,
            scale,
        });
    } else if collision_only && include_simulation {
        return Err(format!(
            "discrete YMAP placement '{}' is collision_only but definition_ref='{}' declares no collision",
            placement.id, placement.definition_ref
        ));
    }

    // Ordinary render/collision placements are already fully resolved above. Do not append
    // 10k+ duplicate MetadataOnly definition instances: that turns a discrete map into an
    // eager startup graph and defeats cell streaming. Only authored metadata_only placements
    // belong in profile.definitions.
    Ok(())
}

fn load_discrete_map_profile(logical_path: &str) -> Result<GameReadyMapProfile, String> {
    let map_ref =
        newengine_assets_api::map_entry_ref(logical_path, newengine_assets_api::MAP_INDEX_ENTRY);
    let request = serde_json::to_vec(&newengine_assets_api::MapRefRequestV1 {
        map_ref: map_ref.clone(),
    })
    .map_err(|e| {
        format!("discrete YMAP index request encode failed path='{logical_path}' err='{e}'")
    })?;
    let index_bytes = newengine_core::call_service_v1_optional(
        newengine_assets_api::ENGINE_ASSETS_MAPS_SERVICE_ID,
        newengine_assets_api::maps_method::INDEX_V1,
        &request,
    )
    .map_err(|e| format!("engine.assets.maps index request failed map='{map_ref}' err='{e}'"))?
    .ok_or_else(|| format!("engine.assets.maps route unavailable while loading map='{map_ref}'"))?;
    let index: newengine_assets_api::MapIndexV1 =
        serde_json::from_slice(&index_bytes).map_err(|e| {
            format!("engine.assets.maps returned invalid MapIndexV1 map='{map_ref}' err='{e}'")
        })?;

    let mut profile = parse_payload(
        serde_json::json!({}),
        "game-ready.mode-defaults",
        logical_path,
    )?;
    profile.title = index.map_id.clone();
    profile.objective = format!("Explore {}", index.map_id);
    profile.terrain.enabled = false;
    profile.terrain.streaming.enabled = false;
    profile.foliage.enabled = false;
    profile.foliage.max_count = 0;
    profile.prefabs.clear();
    profile.definitions.clear();

    let mode_sky_definition = profile.sky.definition_ref.trim().to_owned();
    if !mode_sky_definition.is_empty() {
        profile.definitions.push(GameReadyDefinitionInstanceSpec {
            definition_ref: mode_sky_definition,
            position: Vec3::ZERO,
            rotation_ypr: [0.0, 0.0, 0.0],
            scale: Vec3::ONE,
            apply_mode: GameReadyDefinitionApplyMode::MetadataOnly,
        });
    }

    let fallback_spawn_cell = index
        .world_to_cell([
            profile.player.start.x,
            profile.player.start.y,
            profile.player.start.z,
        ])
        .or_else(|| index.cells.first().map(|cell| cell.coord))
        .unwrap_or_default();
    let spawn_cell = newengine_assets_api::MapCellCoordV1::new(
        metadata_i32(&index, "streaming.spawn_cell_x", fallback_spawn_cell.x),
        metadata_i32(&index, "streaming.spawn_cell_z", fallback_spawn_cell.z),
    );
    let legacy_resident_radius = metadata_i32(&index, "streaming.resident_radius", 1).clamp(0, 4);
    let render_radius =
        metadata_i32(&index, "streaming.render_radius", legacy_resident_radius).clamp(0, 6);
    // Preserve old maps by default while allowing larger render windows to keep physics tight.
    let simulation_default = legacy_resident_radius.min(1).min(render_radius);
    let simulation_radius = metadata_i32(&index, "streaming.simulation_radius", simulation_default)
        .clamp(0, render_radius.max(0));
    let render_unload_radius = metadata_i32(
        &index,
        "streaming.render_unload_radius",
        metadata_i32(&index, "streaming.unload_radius", render_radius + 1),
    )
    .clamp(render_radius + 1, 10);
    let simulation_unload_radius = metadata_i32(
        &index,
        "streaming.simulation_unload_radius",
        simulation_radius + 1,
    )
    .clamp(
        simulation_radius + 1,
        render_unload_radius.max(simulation_radius + 1),
    );
    let max_cells_per_tick = metadata_usize(&index, "streaming.max_cells_per_tick", 1).clamp(1, 8);

    let mut initial_render_cells = existing_cells_within_radius(&index, spawn_cell, render_radius);
    let mut initial_simulation_cells =
        existing_cells_within_radius(&index, spawn_cell, simulation_radius);
    if initial_render_cells.is_empty() && index.cell(spawn_cell).is_some() {
        initial_render_cells.push(spawn_cell);
    }
    if initial_simulation_cells.is_empty() && index.cell(spawn_cell).is_some() {
        initial_simulation_cells.push(spawn_cell);
    }
    let initial_render_set = initial_render_cells
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let initial_simulation_set = initial_simulation_cells
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let mut initial_cells = initial_render_set
        .union(&initial_simulation_set)
        .copied()
        .collect::<Vec<_>>();
    initial_cells.sort_by_key(|coord| (cell_distance(*coord, spawn_cell), coord.x, coord.z));

    let mut definition_cache =
        std::collections::BTreeMap::<String, ResolvedMapDefinitionEntry>::new();
    let mut initial_placement_ids = std::collections::BTreeMap::new();
    for coord in initial_cells.iter().copied() {
        let resolved = load_discrete_cell(&map_ref, coord)?;
        let ids = resolved
            .cell
            .placements
            .iter()
            .filter(|placement| placement.enabled)
            .map(|placement| placement.id.clone())
            .collect::<Vec<_>>();
        initial_placement_ids.insert(coord, ids);
        for placement in resolved
            .cell
            .placements
            .into_iter()
            .filter(|placement| placement.enabled)
        {
            apply_discrete_placement(
                &mut profile,
                &mut definition_cache,
                logical_path,
                coord,
                initial_render_set.contains(&coord),
                initial_simulation_set.contains(&coord),
                placement,
            )?;
        }
    }

    profile.authored_map_streaming = Some(GameReadyAuthoredMapStreamingSpec {
        map_ref: map_ref.clone(),
        index: index.clone(),
        initial_render_cells: initial_render_cells.clone(),
        initial_simulation_cells: initial_simulation_cells.clone(),
        initial_placement_ids,
        render_radius,
        simulation_radius,
        render_unload_radius,
        simulation_unload_radius,
        max_cells_per_tick,
    });

    newengine_ulog_api::ulog::info!(
        "game-ready: loaded discrete YMAP v2 map='{}' cells_total={} cells_initial={} prefabs_initial={} resolved_definitions={} spawn_cell={},{} render_radius={} simulation_radius={} render_unload_radius={} simulation_unload_radius={} policy='index-resident; dual-domain cell payloads stream by player position'",
        map_ref,
        index.cells.len(),
        initial_cells.len(),
        profile.prefabs.len(),
        definition_cache.len(),
        spawn_cell.x,
        spawn_cell.z,
        render_radius,
        simulation_radius,
        render_unload_radius,
        simulation_unload_radius,
    );
    Ok(profile)
}
