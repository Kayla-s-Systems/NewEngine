fn load_discrete_map_profile(
    logical_path: &str,
    authored_profile: Option<AuthoredWorldProfile>,
) -> Result<AuthoredWorldProfile, String> {
    let (map_ref, index) = newengine_authored_world_runtime::load_authored_map_index(logical_path)?;
    load_discrete_map_profile_from_index(logical_path, authored_profile, map_ref, index)
}

fn load_discrete_map_profile_from_index(
    logical_path: &str,
    authored_profile: Option<AuthoredWorldProfile>,
    map_ref: String,
    index: newengine_assets_api::MapIndexV1,
) -> Result<AuthoredWorldProfile, String> {
    let authored_profile_present = authored_profile.is_some();
    let mut profile = if let Some(profile) = authored_profile {
        profile
    } else {
        parse_payload(
            serde_json::json!({}),
            "authored-world.defaults",
            logical_path,
        )?
    };
    if profile.title.trim().is_empty() {
        profile.title = index.map_id.clone();
    }
    if profile.objective.trim().is_empty() {
        profile.objective = format!("Explore {}", index.map_id);
    }
    profile.terrain.enabled = false;
    profile.terrain.streaming.enabled = false;
    profile.foliage.enabled = false;
    profile.foliage.max_count = 0;
    if !authored_profile_present {
        profile.prefabs.clear();
        profile.definitions.clear();
    }

    // Camera identity is declared by an explicit YMAP player_camera placement.
    // No map-level camera scalar or hidden engine camera selection is accepted here.

    let mode_sky_definition = profile.sky.definition_ref.trim().to_owned();
    if !mode_sky_definition.is_empty()
        && !profile.definitions.iter().any(|definition| {
            definition
                .definition_ref
                .eq_ignore_ascii_case(&mode_sky_definition)
        })
    {
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
    const MAX_RENDER_RADIUS_CELLS: i32 = 48;
    const MAX_RENDER_UNLOAD_RADIUS_CELLS: i32 = 64;

    let legacy_resident_radius = metadata_i32(&index, "streaming.resident_radius", 1).clamp(0, 4);
    let authored_render_radius =
        metadata_i32(&index, "streaming.render_radius", legacy_resident_radius)
            .clamp(0, MAX_RENDER_RADIUS_CELLS);
    let configured_view_distance_meters = newengine_plugin_host::current_host_context()
        .environment_var(newengine_core::startup_window::ENV_VIEW_DISTANCE_METERS)
        .and_then(|raw| raw.trim().parse::<f32>().ok())
        .filter(|value| value.is_finite() && *value > 0.0);
    let configured_render_radius = configured_view_distance_meters.map(|meters| {
        ((meters / index.cell_size.max(1.0)).ceil() as i32).clamp(1, MAX_RENDER_RADIUS_CELLS)
    });
    let render_radius = configured_render_radius.unwrap_or(authored_render_radius);
    // Preserve old maps by default while allowing larger render windows to keep physics tight.
    let simulation_default = legacy_resident_radius.min(1).min(render_radius);
    let simulation_radius = metadata_i32(&index, "streaming.simulation_radius", simulation_default)
        .clamp(0, render_radius.max(0));
    let render_unload_radius = metadata_i32(
        &index,
        "streaming.render_unload_radius",
        metadata_i32(&index, "streaming.unload_radius", render_radius + 1),
    )
    .clamp(render_radius + 1, MAX_RENDER_UNLOAD_RADIUS_CELLS);
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
    // Launch residency is intentionally distinct from steady-state residency. Existing maps
    // retain their historical behavior unless they author streaming.launch_radius (or the
    // domain-specific variants). Dense worlds can launch on the spawn cell and expand to the
    // full render/simulation radii immediately after the public activation gate releases.
    let launch_radius = metadata_i32(&index, "streaming.launch_radius", legacy_resident_radius)
        .clamp(0, render_radius.max(simulation_radius));
    let launch_render_radius = metadata_i32(
        &index,
        "streaming.launch_render_radius",
        launch_radius.min(render_radius),
    )
    .clamp(0, render_radius);
    let launch_simulation_radius = metadata_i32(
        &index,
        "streaming.launch_simulation_radius",
        launch_radius.min(simulation_radius),
    )
    .clamp(0, simulation_radius);

    let mut initial_render_cells =
        existing_cells_within_radius(&index, spawn_cell, launch_render_radius);
    let mut initial_simulation_cells =
        existing_cells_within_radius(&index, spawn_cell, launch_simulation_radius);
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

    let definition_cache =
        newengine_authored_world_runtime::AuthoredMapDefinitionCache::for_map(&map_ref);
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
                &definition_cache,
                logical_path,
                coord,
                initial_render_set.contains(&coord),
                initial_simulation_set.contains(&coord),
                placement,
            )?;
        }
    }

    if !profile.gameplay.camera.declared {
        return Err(format!(
            "discrete YMAP v2 declares no player camera path='{}' expected Placement apply_mode='player_camera' definition_ref='.ytyp@entry'",
            logical_path
        ));
    }
    if !profile
        .gameplay
        .camera
        .definition_ref
        .to_ascii_lowercase()
        .contains(".ytyp@")
    {
        return Err(format!(
            "player camera '{}' has invalid definition_ref='{}' expected='.ytyp@entry'",
            profile.gameplay.camera.instance_id, profile.gameplay.camera.definition_ref
        ));
    }

    profile.authored_map_streaming = Some(AuthoredMapStreamingSpec {
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
        "authored-world: loaded discrete YMAP v2 map='{}' cells_total={} cells_initial={} prefabs_initial={} resolved_definitions={} spawn_cell={},{} launch_render_radius={} launch_simulation_radius={} view_distance_meters={:?} render_radius={} simulation_radius={} render_unload_radius={} simulation_unload_radius={} policy='launch ring resident before public Play; steady-state dual-domain cells stream after activation by player position'",
        map_ref,
        index.cells.len(),
        initial_cells.len(),
        profile.prefabs.len(),
        definition_cache.len(),
        spawn_cell.x,
        spawn_cell.z,
        launch_render_radius,
        launch_simulation_radius,
        configured_view_distance_meters,
        render_radius,
        simulation_radius,
        render_unload_radius,
        simulation_unload_radius,
    );
    Ok(profile)
}
