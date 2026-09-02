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
    newengine_authored_world_runtime::load_authored_map_cell(map_ref, coord)
}

fn apply_discrete_placement(
    profile: &mut AuthoredWorldProfile,
    definition_cache: &newengine_authored_world_runtime::AuthoredMapDefinitionCache,
    logical_path: &str,
    coord: newengine_assets_api::MapCellCoordV1,
    include_render: bool,
    include_simulation: bool,
    placement: newengine_assets_api::MapPlacementV1,
) -> Result<(), String> {
    let authored_player_camera = placement.tags.iter().any(|tag| {
        matches!(
            tag.trim().to_ascii_lowercase().as_str(),
            "player_camera" | "camera.player" | "active_camera"
        )
    }) || matches!(
        placement.apply_mode.trim().to_ascii_lowercase().as_str(),
        "player_camera" | "camera_player" | "active_camera"
    );
    if authored_player_camera {
        if profile.gameplay.camera.declared {
            return Err(format!(
                "discrete YMAP declares more than one player camera previous='{}' duplicate='{}'",
                profile.gameplay.camera.instance_id, placement.id
            ));
        }
        profile.gameplay.camera.declared = true;
        profile.gameplay.camera.instance_id = placement.id.clone();
        profile.gameplay.camera.definition_ref = placement.definition_ref.clone();
        profile.gameplay.camera.position = Vec3::new(
            placement.transform.position[0],
            placement.transform.position[1],
            placement.transform.position[2],
        );
        profile.gameplay.camera.rotation_ypr = Vec3::new(
            placement.transform.rotation_ypr[0],
            placement.transform.rotation_ypr[1],
            placement.transform.rotation_ypr[2],
        );
        profile.definitions.push(GameReadyDefinitionInstanceSpec {
            definition_ref: placement.definition_ref.clone(),
            position: profile.gameplay.camera.position,
            rotation_ypr: placement.transform.rotation_ypr,
            scale: Vec3::ONE,
            apply_mode: GameReadyDefinitionApplyMode::MetadataOnly,
        });
        newengine_ulog_api::ulog::info!(
            "authored-world: authored player camera selected id='{}' definition_ref='{}' position={:?} rotation_ypr={:?} policy='YMAP declares camera instance; YTYP defines behavior'",
            profile.gameplay.camera.instance_id,
            profile.gameplay.camera.definition_ref,
            profile.gameplay.camera.position,
            profile.gameplay.camera.rotation_ypr,
        );
        return Ok(());
    }

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
            "authored-world: authored player spawn selected id='{}' position={:?} yaw={:.3} policy='YMAP spawn marker owns map start position'",
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

    let definition = definition_cache
        .resolve_definition_entry(&placement.definition_ref)
        .map_err(|e| {
            format!(
                "discrete YMAP placement '{}' definition_ref='{}' resolution failed: {e}",
                placement.id, placement.definition_ref
            )
        })?;

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
    let surface_binding =
        newengine_authored_world_runtime::project_authored_definition_surface(&definition);
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
        profile.prefabs.push(AuthoredWorldPlacementSpec {
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
            surface_id: surface_binding.id.clone(),
            surface_events: surface_binding.events.clone(),
            ballistic_material: surface_binding.ballistic_material,
            ground_placement_surface: surface_binding.ground_placement_surface,
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
        profile.prefabs.push(AuthoredWorldPlacementSpec {
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
            surface_id: surface_binding.id.clone(),
            surface_events: surface_binding.events.clone(),
            ballistic_material: surface_binding.ballistic_material,
            ground_placement_surface: surface_binding.ground_placement_surface,
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
