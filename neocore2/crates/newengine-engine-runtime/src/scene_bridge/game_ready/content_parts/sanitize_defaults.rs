fn fallback_game_ready_map_profile() -> GameReadyMapProfile {
    RawGameReadyPayload::default().into_profile()
}

impl Default for RawGameReadyPayload {
    fn default() -> Self {
        Self {
            title: default_title(),
            objective: default_objective(),
            player: RawPlayerSpec::default(),
            terrain: RawTerrainSpec::default(),
            sky: RawSkySpec::default(),
            materials: RawMaterialSetSpec::default(),
            lighting: RawLightingSpec::default(),
            foliage: RawFoliageSpec::default(),
            prefabs: Vec::new(),
            gameplay: RawGameplaySpec::default(),
            palette: RawPaletteSpec::default(),
        }
    }
}


#[inline]
fn non_empty_or(value: String, fallback: String) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() { fallback } else { trimmed.to_owned() }
}

#[inline]
fn sanitize_texture_path(value: Option<String>) -> Option<String> {
    sanitize_asset_path(value)
}

#[inline]
fn sanitize_asset_path(value: Option<String>) -> Option<String> {
    value.and_then(|s| {
        let trimmed = s.trim();
        if trimmed.is_empty() { None } else { Some(trimmed.replace('\\', "/")) }
    })
}

#[inline]
fn sanitize_vec2(mut v: [f32; 2], fallback: [f32; 2]) -> [f32; 2] {
    for i in 0..2 {
        if !v[i].is_finite() || v[i].abs() <= 1.0e-6 {
            v[i] = fallback[i];
        }
    }
    v
}

#[inline]
fn sanitize_material_spec(raw: RawMaterialSpec) -> GameReadyMaterialSpec {
    GameReadyMaterialSpec {
        asset: sanitize_texture_path(raw.asset),
        base_color_texture: sanitize_texture_path(raw.base_color_texture),
        normal_texture: sanitize_texture_path(raw.normal_texture),
        roughness_texture: sanitize_texture_path(raw.roughness_texture),
        uv_scale: sanitize_vec2(raw.uv_scale, default_uv_scale()),
        uv_offset: sanitize_vec2(raw.uv_offset, default_uv_offset()),
        roughness: raw.roughness.clamp(0.02, 1.0),
        normal_scale: raw.normal_scale.clamp(0.0, 8.0),
        occlusion_strength: raw.occlusion_strength.clamp(0.0, 1.0),
    }
}

#[inline]
fn sanitize_color3(mut v: ColorRgb, fallback: ColorRgb) -> ColorRgb {
    for i in 0..3 {
        if !v[i].is_finite() {
            v[i] = fallback[i];
        }
        v[i] = v[i].clamp(0.0, 1.0);
    }
    v
}

#[inline]
fn sanitize_direction3(v: ColorRgb, fallback: ColorRgb) -> ColorRgb {
    let d = Vec3::new(v[0], v[1], v[2]);
    let d = if d.length_squared() > 1.0e-6 && d.is_finite() {
        d.normalize_or_zero()
    } else {
        Vec3::new(fallback[0], fallback[1], fallback[2]).normalize_or_zero()
    };
    [d.x, d.y, d.z]
}

#[inline]
fn sanitize_lighting_spec(raw: RawLightingSpec) -> GameReadyLightingSpec {
    GameReadyLightingSpec {
        ambient_color: sanitize_color3(raw.ambient_color, default_ambient_color()),
        ambient_intensity: raw.ambient_intensity.clamp(0.0, 8.0),
        sun_direction: sanitize_direction3(raw.sun_direction, default_sun_direction()),
        sun_color: sanitize_color3(raw.sun_color, default_sun_color()),
        sun_intensity: raw.sun_intensity.clamp(0.0, 32.0),
        day_night: GameReadyDayNightSpec {
            enabled: raw.day_night.enabled,
            time_of_day_hours: raw.day_night.time_of_day_hours.rem_euclid(24.0),
            day_length_seconds: raw.day_night.day_length_seconds.clamp(30.0, 86_400.0),
            latitude_degrees: raw.day_night.latitude_degrees.clamp(-89.0, 89.0),
            axial_tilt_degrees: raw.day_night.axial_tilt_degrees.clamp(-45.0, 45.0),
        },
        shadows: GameReadyShadowSpec {
            enabled: raw.shadows.enabled,
            resolution: raw.shadows.resolution.clamp(256, 8192),
            cascade_count: raw.shadows.cascade_count.clamp(1, 4),
            max_distance: raw.shadows.max_distance.clamp(1.0, 1000.0),
            softness: raw.shadows.softness.clamp(0.0, 16.0),
            bias: raw.shadows.bias.clamp(0.0, 0.1),
            normal_bias: raw.shadows.normal_bias.clamp(0.0, 0.5),
            contact_strength: raw.shadows.contact_strength.clamp(0.0, 1.0),
        },
    }
}


#[inline]
fn sanitize_foliage_spec(raw: RawFoliageSpec) -> GameReadyFoliageSpec {
    let min_scale = raw.min_scale.clamp(0.05, 32.0);
    let max_scale = raw.max_scale.clamp(min_scale, 32.0);
    let (grid_min, grid_max) = if raw.grid_min <= raw.grid_max {
        (raw.grid_min, raw.grid_max)
    } else {
        (raw.grid_max, raw.grid_min)
    };

    GameReadyFoliageSpec {
        enabled: raw.enabled && raw.max_count > 0,
        prefab: non_empty_or(raw.prefab, default_foliage_prefab()),
        seed: raw.seed,
        grid_min: grid_min.clamp(-512, 512),
        grid_max: grid_max.clamp(-512, 512),
        spacing: raw.spacing.clamp(0.5, 128.0),
        jitter: raw.jitter.clamp(0.0, 0.95),
        gate_threshold: raw.gate_threshold.clamp(0.0, 1.0),
        max_count: raw.max_count.min(8192),
        min_scale,
        max_scale,
        min_player_distance: raw.min_player_distance.clamp(0.0, 256.0),
        edge_margin: raw.edge_margin.clamp(0.0, 512.0),
        surface_offset: raw.surface_offset.clamp(-4.0, 8.0),
    }
}

#[inline]
fn sanitize_prefab_spec(raw: RawPrefabSpec) -> Option<GameReadyPrefabSpec> {
    let id = raw.id.trim();
    if id.is_empty() {
        return None;
    }

    Some(GameReadyPrefabSpec {
        id: id.to_owned(),
        source: raw.source.trim().to_owned(),
        proxy: non_empty_or(raw.proxy, default_prefab_proxy()),
        enabled: raw.enabled,
    })
}

#[inline]
fn arr3(v: [f32; 3]) -> Vec3 { Vec3::new(v[0], v[1], v[2]) }

fn default_title() -> String { "KAYLA FPS: Procedural Highlands".to_owned() }
fn default_objective() -> String { "Walk a deterministic terrain sandbox with an imported sky dome.".to_owned() }
fn default_player_start() -> [f32; 3] { [-17.5, 0.0, -17.5] }
fn default_player_yaw() -> f32 { -0.72 }
fn default_move_speed() -> f32 { 7.3 }
fn default_look_sens() -> f32 { 0.0022 }
fn default_player_model_enabled() -> bool { true }
fn default_player_model_source() -> String { "player/abigail/csb_abigail_static_y_up.obj".to_owned() }
fn default_player_texture_dictionary() -> Option<String> { Some("player/abigail/textures/abigail.neytd".to_owned()) }
fn default_player_skeleton() -> Option<String> { Some("player/abigail/csb_abigail.ymt".to_owned()) }
fn default_player_model_height() -> f32 { 1.78 }
fn default_player_model_eye_height_ratio() -> f32 { 0.91 }
fn default_player_model_offset() -> [f32; 3] { [0.0, 0.0, 0.0] }
fn default_player_model_yaw_offset() -> f32 { 0.0 }
fn default_player_model_hide_in_first_person() -> bool { true }
fn default_terrain_seed() -> u64 { 0x2026_0509_4b41_594c }
fn default_terrain_cells() -> u32 { 80 }
fn default_terrain_size() -> f32 { 52.0 }
fn default_base_height() -> f32 { -0.04 }
fn default_height_scale() -> f32 { 1.35 }
fn default_terrain_generator_id() -> String { "newengine.generator.lowland-biomes.v1".to_owned() }
fn default_ridged_seed_xor() -> u64 { 0x7e22_a11d }
fn default_ridged_frequency() -> f32 { 1.25 }
fn default_ridged_amplitude() -> f32 { 0.11 }
fn default_ridged_shape_edge0() -> f32 { 0.08 }
fn default_ridged_shape_edge1() -> f32 { 1.0 }
fn default_veins_seed_xor() -> u64 { 0x5317_1001 }
fn default_veins_frequency() -> f32 { 0.52 }
fn default_veins_amplitude() -> f32 { 0.10 }
fn default_smoothing_passes() -> u32 { 2 }
fn default_smoothing_strength() -> f32 { 0.42 }
fn default_terrain_surface_forest() -> String { "textures/fps/world_surfaces.neytd@terrain_forest_floor".to_owned() }
fn default_terrain_surface_sand() -> String { "textures/fps/world_surfaces.neytd@ground_sand".to_owned() }
fn default_terrain_surface_rock() -> String { "textures/fps/world_surfaces.neytd@rock_moss".to_owned() }
fn default_terrain_patch_scale() -> f32 { 0.033 }
fn default_terrain_blend_softness() -> f32 { 0.18 }
fn default_terrain_streaming_enabled() -> bool { true }
fn default_terrain_chunk_radius() -> i32 { 2 }
fn default_terrain_unload_radius() -> i32 { 4 }
fn default_terrain_max_chunks_per_frame() -> usize { 4 }
fn default_sky_radius() -> f32 { 220.0 }
fn default_skydome_mesh() -> String { "procedural:skydome".to_owned() }
fn default_sky_follow_camera() -> bool { true }
fn default_cloud_dictionary() -> String { "textures/fps/clouds_runtime.neytd".to_owned() }
fn default_cloud_profile() -> String { "clear".to_owned() }
fn default_status_text() -> String { "GameFirst world: streamed lowland terrain, runtime textures and sun shadows.".to_owned() }
fn default_pickup_status() -> String { "Core acquired.".to_owned() }
fn default_hazard_status() -> String { "Hazard touched.".to_owned() }
fn default_goal_locked_status() -> String { "Beacon locked.".to_owned() }
fn default_goal_complete_status() -> String { "Extraction complete. Runtime loop is stable and playable.".to_owned() }
fn default_failed_progress_label() -> String { "FAILED".to_owned() }
fn default_completed_progress_label() -> String { "EXTRACTED".to_owned() }
fn default_player_body_radius() -> f32 { 0.45 }
fn default_player_body_half_height() -> f32 { 0.45 }
fn default_player_visual_radius() -> f32 { 0.45 }
fn default_player_visual_half_height() -> f32 { 0.90 }
fn default_camera_eye_height() -> f32 { 0.72 }
fn default_sprint_multiplier() -> f32 { 1.75 }
fn default_gravity() -> f32 { 9.81 }
fn default_contact_skin() -> f32 { 0.035 }
fn default_terrain_color() -> ColorRgba { [0.78, 0.86, 0.68, 1.0] }
fn default_sky_color() -> ColorRgba { [0.08, 0.16, 0.34, 1.0] }
fn default_sky_emissive() -> ColorRgb { [0.07, 0.14, 0.34] }
fn default_tree_bark_color() -> ColorRgba { [0.38, 0.23, 0.12, 1.0] }
fn default_tree_leaf_color() -> ColorRgba { [0.18, 0.42, 0.16, 1.0] }
fn default_tree_branch_color() -> ColorRgba { [0.32, 0.20, 0.12, 1.0] }
fn default_uv_scale() -> [f32; 2] { [1.0, 1.0] }
fn default_uv_offset() -> [f32; 2] { [0.0, 0.0] }
fn default_material_roughness() -> f32 { 0.86 }
fn default_material_normal_scale() -> f32 { 1.0 }
fn default_material_occlusion_strength() -> f32 { 1.0 }
fn default_ambient_color() -> ColorRgb { [0.38, 0.42, 0.50] }
fn default_ambient_intensity() -> f32 { 0.28 }
fn default_sun_direction() -> ColorRgb { [-0.55, -0.82, -0.28] }
fn default_sun_color() -> ColorRgb { [1.0, 0.94, 0.82] }
fn default_sun_intensity() -> f32 { 3.20 }
fn default_day_night_enabled() -> bool { true }
fn default_time_of_day_hours() -> f32 { 9.35 }
fn default_day_length_seconds() -> f32 { 720.0 }
fn default_sun_latitude_degrees() -> f32 { 45.0 }
fn default_axial_tilt_degrees() -> f32 { 23.44 }
fn default_shadow_enabled() -> bool { true }
fn default_shadow_resolution() -> u32 { 2048 }
fn default_shadow_cascade_count() -> u32 { 1 }
fn default_shadow_max_distance() -> f32 { 80.0 }
fn default_shadow_softness() -> f32 { 0.75 }
fn default_shadow_bias() -> f32 { 0.0025 }
fn default_shadow_normal_bias() -> f32 { 0.015 }
fn default_shadow_contact_strength() -> f32 { 0.35 }
fn default_foliage_prefab() -> String { "tree_animate".to_owned() }
fn default_foliage_seed() -> u64 { 0x5452_4545_2026 }
fn default_foliage_grid_min() -> i32 { -5 }
fn default_foliage_grid_max() -> i32 { 5 }
fn default_foliage_spacing() -> f32 { 6.0 }
fn default_foliage_jitter() -> f32 { 0.45 }
fn default_foliage_gate_threshold() -> f32 { 0.62 }
fn default_foliage_min_scale() -> f32 { 0.85 }
fn default_foliage_max_scale() -> f32 { 1.35 }
fn default_foliage_min_player_distance() -> f32 { 5.0 }
fn default_foliage_edge_margin() -> f32 { 4.0 }
fn default_foliage_surface_offset() -> f32 { 0.03 }
fn default_prefab_proxy() -> String { "runtime_gltf_mesh".to_owned() }
fn default_prefab_enabled() -> bool { true }
fn default_terrain_material() -> RawMaterialSpec {
    RawMaterialSpec {
        asset: Some("materials/fps/terrain_forest_floor.material.json".to_owned()),
        base_color_texture: Some("textures/fps/world_surfaces.neytd@terrain_forest_floor".to_owned()),
        normal_texture: Some("textures/fps/world_surfaces.neytd@terrain_forest_floor_normal".to_owned()),
        roughness_texture: Some("textures/fps/world_surfaces.neytd@terrain_forest_floor_roughness".to_owned()),
        uv_scale: [4.0, 4.0],
        uv_offset: [0.0, 0.0],
        roughness: 0.92,
        normal_scale: 0.0,
        occlusion_strength: 0.88,
    }
}
fn default_sky_material() -> RawMaterialSpec {
    RawMaterialSpec {
        base_color_texture: Some("textures/fps/clouds_runtime.neytd@cloud_clear__new_skyhat_clear01_bot_ap".to_owned()),
        normal_texture: Some("textures/fps/clouds_runtime.neytd@cloud_clear__new_skyhat_clear01_bot_nrm".to_owned()),
        roughness: 1.0,
        normal_scale: 0.18,
        occlusion_strength: 1.0,
        ..RawMaterialSpec::default()
    }
}

fn default_tree_bark_material() -> RawMaterialSpec {
    RawMaterialSpec {
        base_color_texture: Some("prefabs/tree_animate/textures/tree_animate.neytd@bark_diffuse".to_owned()),
        normal_texture: Some("prefabs/tree_animate/textures/tree_animate.neytd@bark_normal".to_owned()),
        roughness: 0.88,
        normal_scale: 0.35,
        occlusion_strength: 1.0,
        ..RawMaterialSpec::default()
    }
}

fn default_tree_leaf_material() -> RawMaterialSpec {
    RawMaterialSpec {
        base_color_texture: Some("prefabs/tree_animate/textures/tree_animate.neytd@leaf_diffuse".to_owned()),
        normal_texture: Some("prefabs/tree_animate/textures/tree_animate.neytd@leaf_normal".to_owned()),
        roughness: 0.72,
        normal_scale: 0.25,
        occlusion_strength: 1.0,
        ..RawMaterialSpec::default()
    }
}

fn default_tree_branch_material() -> RawMaterialSpec {
    RawMaterialSpec {
        base_color_texture: Some("prefabs/tree_animate/textures/tree_animate.neytd@branch_diffuse".to_owned()),
        normal_texture: Some("prefabs/tree_animate/textures/tree_animate.neytd@branch_normal".to_owned()),
        roughness: 0.86,
        normal_scale: 0.30,
        occlusion_strength: 1.0,
        ..RawMaterialSpec::default()
    }
}
