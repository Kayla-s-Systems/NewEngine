pub(crate) fn prepare_player_hair_from_assignment_v1(
    player: EntityId,
    assignment: &newengine_engine_runtime::gameplay::PlayerModelAssignment,
    skeleton: Option<&newengine_model_skeleton_api::ModelSkeletonMetadata>,
) -> Result<Option<PreparedPlayerHairV1>, String> {
    let Some(definition_ref) = assignment.properties_ref.as_deref() else {
        return Ok(None);
    };
    let Some(entry) = newengine_definitions_runtime::load_definition_entry_v1(definition_ref).ok()
    else {
        return Ok(None);
    };
    let metadata = newengine_definitions_runtime::definition_metadata_namespace(
        &entry,
        "newengine.game_ready",
    )
    .unwrap_or(&serde_json::Value::Null);
    let Some(player_metadata) = metadata.get("player") else {
        return Ok(None);
    };
    let Some(groom_path) = hair_string(player_metadata, "groom") else {
        return Ok(None);
    };
    if hair_bool(player_metadata, "enabled") == Some(false) {
        return Ok(None);
    }
    let skeleton = skeleton.ok_or_else(|| {
        format!("player hair groom '{groom_path}' requires authored skeleton metadata")
    })?;
    let groom = load_nehair_groom_v1(&groom_path)?;
    validate_groom_against_skeleton(&groom, skeleton)?;

    let mut instance = HairInstanceDescV1 {
        instance_id: runtime_player_hair_instance_id(player, &groom_path),
        quality: parse_quality(hair_string(player_metadata, "quality"))?,
        casts_shadows: hair_bool(player_metadata, "casts_shadows").unwrap_or(true),
        receives_shadows: hair_bool(player_metadata, "receives_shadows").unwrap_or(true),
        ..HairInstanceDescV1::default()
    };
    instance.wind_velocity = hair_vec3(player_metadata, "wind_velocity").unwrap_or([0.0; 3]);
    instance.simulation.mode = parse_simulation_mode(hair_string(player_metadata, "simulation"))?;
    instance.simulation.collision =
        parse_collision_mode(hair_string(player_metadata, "collision"))?;
    if let Some(value) = hair_f32(player_metadata, "gravity_scale") {
        instance.simulation.gravity_scale = value;
    }
    if let Some(value) = hair_f32(player_metadata, "damping") {
        instance.simulation.damping = value;
    }
    if let Some(value) = hair_f32(player_metadata, "stretch_stiffness") {
        instance.simulation.stretch_stiffness = value;
    }
    if let Some(value) = hair_f32(player_metadata, "bend_stiffness") {
        instance.simulation.bend_stiffness = value;
    }
    if let Some(value) = hair_f32(player_metadata, "root_stiffness") {
        instance.simulation.root_stiffness = value;
    }
    if let Some(value) = hair_f32(player_metadata, "wind_response") {
        instance.simulation.wind_response = value;
    }
    if let Some(value) = hair_u8(player_metadata, "solver_iterations") {
        instance.simulation.solver_iterations = value;
    }
    if let Some(value) = hair_f32(player_metadata, "max_delta_seconds") {
        instance.simulation.max_delta_seconds = value;
    }

    if let Some(value) = hair_vec3(player_metadata, "base_color") {
        instance.material.base_color = value;
    }
    if let Some(value) = hair_f32(player_metadata, "roughness") {
        instance.material.roughness = value;
    }
    if let Some(value) = hair_f32(player_metadata, "secondary_specular") {
        instance.material.secondary_specular = value;
    }
    if let Some(value) = hair_f32(player_metadata, "melanin") {
        instance.material.melanin = value;
    }
    if let Some(value) = hair_f32(player_metadata, "redness") {
        instance.material.redness = value;
    }
    if let Some(value) = hair_f32(player_metadata, "opacity") {
        instance.material.opacity = value;
    }
    if let Some(value) = hair_f32(player_metadata, "strand_width_mm") {
        instance.material.strand_width_mm = value;
    }
    if let Some(value) = hair_f32(player_metadata, "tip_scale") {
        instance.material.tip_scale = value;
    }
    instance.material.transparency =
        parse_transparency(hair_string(player_metadata, "transparency"))?;

    if let Some(value) = hair_f32(player_metadata, "lod_density_start") {
        instance.lod.density_start_distance = value;
    }
    if let Some(value) = hair_f32(player_metadata, "lod_density_end") {
        instance.lod.density_end_distance = value;
    }
    if let Some(value) = hair_f32(player_metadata, "lod_minimum_density") {
        instance.lod.minimum_density = value;
    }
    if let Some(value) = hair_f32(player_metadata, "lod_simulation_distance") {
        instance.lod.simulation_distance = value;
    }
    instance = instance.normalized()?;

    let simulation_shader = hair_string(player_metadata, "simulation_shader")
        .ok_or_else(|| "player hair requires authored hair_simulation_shader".to_owned())?;
    let strands_vertex_shader = hair_string(player_metadata, "strands_vertex_shader")
        .ok_or_else(|| "player hair requires authored hair_strands_vertex_shader".to_owned())?;
    let strands_fragment_shader = hair_string(player_metadata, "strands_fragment_shader")
        .ok_or_else(|| "player hair requires authored hair_strands_fragment_shader".to_owned())?;
    let mut shaders = HairShaderSetV1::new(
        simulation_shader,
        strands_vertex_shader,
        strands_fragment_shader,
    );
    match (
        hair_string(player_metadata, "shadow_vertex_shader"),
        hair_string(player_metadata, "shadow_fragment_shader"),
    ) {
        (Some(vs), Some(fs)) => shaders = shaders.with_shadows(vs, fs),
        (None, None) => {}
        _ => return Err("player hair shadow shader pair must be authored atomically".to_owned()),
    }
    shaders = shaders.normalized()?;

    let source_mesh_prefixes = hair_string_list(player_metadata, "source_mesh_prefixes");
    Ok(Some(PreparedPlayerHairV1 {
        groom,
        instance,
        shaders,
        source_mesh_prefixes,
        hide_in_first_person: hair_bool(player_metadata, "hide_in_first_person")
            .unwrap_or(assignment.hide_in_first_person),
    }))
}

