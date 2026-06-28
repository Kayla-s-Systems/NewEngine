use super::*;

// Strict data-driven mode: authored .ymap is required; no emergency runtime profile is generated.
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
            definitions: Vec::new(),
            gameplay: RawGameplaySpec::default(),
            palette: RawPaletteSpec::default(),
        }
    }
}

#[inline]
pub(super) fn non_empty_or(value: String, fallback: String) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback
    } else {
        trimmed.to_owned()
    }
}

#[inline]
pub(super) fn sanitize_texture_path(value: Option<String>) -> Option<String> {
    sanitize_asset_path(value)
}

#[inline]
pub(super) fn sanitize_asset_path(value: Option<String>) -> Option<String> {
    value.and_then(|s| {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.replace('\\', "/"))
        }
    })
}

#[inline]
pub(super) fn sanitize_vec2(mut v: [f32; 2], fallback: [f32; 2]) -> [f32; 2] {
    for i in 0..2 {
        if !v[i].is_finite() || v[i].abs() <= 1.0e-6 {
            v[i] = fallback[i];
        }
    }
    v
}

#[inline]
pub(super) fn sanitize_material_spec(raw: RawMaterialSpec) -> GameReadyMaterialSpec {
    GameReadyMaterialSpec {
        asset: sanitize_asset_path(raw.asset),
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
pub(super) fn sanitize_material_spec_with_default_asset(
    raw: RawMaterialSpec,
    fallback: RawMaterialSpec,
) -> GameReadyMaterialSpec {
    let fallback = sanitize_material_spec(fallback);
    let mut spec = sanitize_material_spec(raw);
    if spec.asset.is_none() {
        spec.asset = fallback.asset;
    }
    spec
}

#[inline]
pub(super) fn sanitize_color3(mut v: ColorRgb, fallback: ColorRgb) -> ColorRgb {
    for i in 0..3 {
        if !v[i].is_finite() {
            v[i] = fallback[i];
        }
        v[i] = v[i].clamp(0.0, 1.0);
    }
    v
}

#[inline]
pub(super) fn sanitize_direction3(v: ColorRgb, fallback: ColorRgb) -> ColorRgb {
    let d = Vec3::new(v[0], v[1], v[2]);
    let d = if d.length_squared() > 1.0e-6 && d.is_finite() {
        d.normalize_or_zero()
    } else {
        Vec3::new(fallback[0], fallback[1], fallback[2]).normalize_or_zero()
    };
    [d.x, d.y, d.z]
}

#[inline]
pub(super) fn sanitize_sky_atmosphere_spec(
    raw: RawSkyAtmosphereSpec,
) -> GameReadySkyAtmosphereSpec {
    GameReadySkyAtmosphereSpec {
        day_zenith: sanitize_color3(raw.day_zenith, default_sky_day_zenith()),
        day_horizon: sanitize_color3(raw.day_horizon, default_sky_day_horizon()),
        dusk_zenith: sanitize_color3(raw.dusk_zenith, default_sky_dusk_zenith()),
        dusk_horizon: sanitize_color3(raw.dusk_horizon, default_sky_dusk_horizon()),
        night_zenith: sanitize_color3(raw.night_zenith, default_sky_night_zenith()),
        night_horizon: sanitize_color3(raw.night_horizon, default_sky_night_horizon()),
        cloud_day: sanitize_color3(raw.cloud_day, default_sky_cloud_day()),
        cloud_night: sanitize_color3(raw.cloud_night, default_sky_cloud_night()),
        night_sky_strength: raw.night_sky_strength.clamp(0.0, 1.0),
        cloud_coverage: raw.cloud_coverage.clamp(0.0, 1.0),
        cloud_softness: raw.cloud_softness.clamp(0.01, 1.0),
    }
}

#[inline]
pub(super) fn sanitize_lighting_spec(raw: RawLightingSpec) -> GameReadyLightingSpec {
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
pub(super) fn sanitize_foliage_spec(raw: RawFoliageSpec) -> GameReadyFoliageSpec {
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
pub(super) fn sanitize_prefab_spec(raw: RawPrefabSpec) -> Option<GameReadyPrefabSpec> {
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
pub(super) fn sanitize_definition_instance_spec(
    raw: RawDefinitionInstanceSpec,
) -> Option<GameReadyDefinitionInstanceSpec> {
    let definition_ref = raw.definition_ref.trim().replace('\\', "/");
    if definition_ref.is_empty() {
        return None;
    }
    if !definition_ref.to_ascii_lowercase().contains(".ytyp@") {
        newengine_ulog_api::ulog::warn!(
            "game-ready definitions: rejected definition_ref='{}' reason='expected .ytyp@entry selector'",
            definition_ref
        );
        return None;
    }
    let apply_mode = GameReadyDefinitionApplyMode::from_str(&raw.apply_mode);
    Some(GameReadyDefinitionInstanceSpec {
        definition_ref,
        position: arr3(raw.position),
        rotation_ypr: sanitize_array3_finite(raw.rotation_ypr, [0.0, 0.0, 0.0]),
        scale: arr3(sanitize_array3_positive(
            raw.scale,
            default_definition_scale(),
        )),
        apply_mode,
    })
}

#[inline]
pub(super) fn sanitize_array3_finite(mut value: [f32; 3], fallback: [f32; 3]) -> [f32; 3] {
    for i in 0..3 {
        if !value[i].is_finite() {
            value[i] = fallback[i];
        }
    }
    value
}

#[inline]
pub(super) fn sanitize_array3_positive(mut value: [f32; 3], fallback: [f32; 3]) -> [f32; 3] {
    for i in 0..3 {
        if !value[i].is_finite() || value[i].abs() <= 1.0e-6 {
            value[i] = fallback[i];
        }
    }
    value
}

#[inline]
pub(super) fn arr3(v: [f32; 3]) -> Vec3 {
    Vec3::new(v[0], v[1], v[2])
}

pub(super) fn default_definition_scale() -> [f32; 3] {
    [1.0, 1.0, 1.0]
}
pub(super) fn default_definition_apply_mode() -> String {
    "metadata_only".to_owned()
}
pub(super) fn default_title() -> String {
    "KAYLA FPS: Procedural Highlands".to_owned()
}
pub(super) fn default_objective() -> String {
    "Walk a deterministic map assembled from .ymap -> .ytyp -> .ydd -> .nemat -> .ytd assets."
        .to_owned()
}
pub(super) fn default_player_start() -> [f32; 3] {
    [-17.5, 0.0, -17.5]
}
pub(super) fn default_player_yaw() -> f32 {
    -0.72
}
pub(super) fn default_move_speed() -> f32 {
    7.3
}
pub(super) fn default_look_sens() -> f32 {
    0.0022
}
pub(super) fn default_player_model_enabled() -> bool {
    false
}
pub(super) fn default_player_model_source() -> String {
    String::new()
}
pub(super) fn default_player_texture_dictionary() -> Option<String> {
    None
}
pub(super) fn default_player_skeleton() -> Option<String> {
    None
}
pub(super) fn default_player_model_height() -> f32 {
    1.78
}
pub(super) fn default_player_model_eye_height_ratio() -> f32 {
    0.91
}
pub(super) fn default_player_model_offset() -> [f32; 3] {
    [0.0, 0.0, 0.0]
}
pub(super) fn default_player_model_yaw_offset() -> f32 {
    0.0
}
pub(super) fn default_player_model_hide_in_first_person() -> bool {
    true
}
pub(super) fn default_terrain_seed() -> u64 {
    0x2026_0509_4b41_594c
}
pub(super) fn default_terrain_cells() -> u32 {
    80
}
pub(super) fn default_terrain_size() -> f32 {
    52.0
}
pub(super) fn default_base_height() -> f32 {
    -0.04
}
pub(super) fn default_height_scale() -> f32 {
    1.35
}
pub(super) fn default_terrain_generator_id() -> String {
    "newengine.generator.lowland-biomes.v1".to_owned()
}
pub(super) fn default_ridged_seed_xor() -> u64 {
    0x7e22_a11d
}
pub(super) fn default_ridged_frequency() -> f32 {
    1.25
}
pub(super) fn default_ridged_amplitude() -> f32 {
    0.11
}
pub(super) fn default_ridged_shape_edge0() -> f32 {
    0.08
}
pub(super) fn default_ridged_shape_edge1() -> f32 {
    1.0
}
pub(super) fn default_veins_seed_xor() -> u64 {
    0x5317_1001
}
pub(super) fn default_veins_frequency() -> f32 {
    0.52
}
pub(super) fn default_veins_amplitude() -> f32 {
    0.10
}
pub(super) fn default_smoothing_passes() -> u32 {
    2
}
pub(super) fn default_smoothing_strength() -> f32 {
    0.42
}
pub(super) fn default_terrain_surface_forest() -> String {
    String::new()
}
pub(super) fn default_terrain_surface_sand() -> String {
    String::new()
}
pub(super) fn default_terrain_surface_rock() -> String {
    String::new()
}
pub(super) fn default_terrain_patch_scale() -> f32 {
    0.033
}
pub(super) fn default_terrain_blend_softness() -> f32 {
    0.18
}
pub(super) fn default_terrain_streaming_enabled() -> bool {
    true
}
pub(super) fn default_terrain_chunk_radius() -> i32 {
    2
}
pub(super) fn default_terrain_unload_radius() -> i32 {
    4
}
pub(super) fn default_terrain_max_chunks_per_frame() -> usize {
    4
}
pub(super) fn default_sky_radius() -> f32 {
    220.0
}
pub(super) fn default_skydome_mesh() -> String {
    String::new()
}
pub(super) fn default_sky_follow_camera() -> bool {
    true
}
pub(super) fn default_cloud_dictionary() -> String {
    String::new()
}
pub(super) fn default_cloud_profile() -> String {
    "clear".to_owned()
}
pub(super) fn default_sky_sun_radius() -> f32 {
    18.0
}
pub(super) fn default_sky_moon_radius() -> f32 {
    13.5
}
pub(super) fn default_moon_texture() -> String {
    String::new()
}
pub(super) fn default_sky_day_zenith() -> ColorRgb {
    [0.23, 0.42, 0.82]
}
pub(super) fn default_sky_day_horizon() -> ColorRgb {
    [0.64, 0.78, 0.96]
}
pub(super) fn default_sky_dusk_zenith() -> ColorRgb {
    [0.16, 0.20, 0.40]
}
pub(super) fn default_sky_dusk_horizon() -> ColorRgb {
    [1.00, 0.47, 0.20]
}
pub(super) fn default_sky_night_zenith() -> ColorRgb {
    [0.006, 0.010, 0.030]
}
pub(super) fn default_sky_night_horizon() -> ColorRgb {
    [0.020, 0.024, 0.052]
}
pub(super) fn default_sky_cloud_day() -> ColorRgb {
    [0.98, 0.96, 0.88]
}
pub(super) fn default_sky_cloud_night() -> ColorRgb {
    [0.040, 0.050, 0.085]
}
pub(super) fn default_sky_night_strength() -> f32 {
    0.35
}
pub(super) fn default_sky_cloud_coverage() -> f32 {
    0.42
}
pub(super) fn default_sky_cloud_softness() -> f32 {
    0.72
}
pub(super) fn default_status_text() -> String {
    String::new()
}
pub(super) fn default_pickup_status() -> String {
    String::new()
}
pub(super) fn default_hazard_status() -> String {
    String::new()
}
pub(super) fn default_goal_locked_status() -> String {
    String::new()
}
pub(super) fn default_goal_complete_status() -> String {
    String::new()
}
pub(super) fn default_failed_progress_label() -> String {
    String::new()
}
pub(super) fn default_completed_progress_label() -> String {
    String::new()
}
pub(super) fn default_player_body_radius() -> f32 {
    0.45
}
pub(super) fn default_player_body_half_height() -> f32 {
    0.45
}
pub(super) fn default_player_visual_radius() -> f32 {
    0.45
}
pub(super) fn default_player_visual_half_height() -> f32 {
    0.90
}
pub(super) fn default_camera_eye_height() -> f32 {
    0.72
}
pub(super) fn default_sprint_multiplier() -> f32 {
    1.75
}
pub(super) fn default_gravity() -> f32 {
    9.81
}
pub(super) fn default_contact_skin() -> f32 {
    0.035
}
pub(super) fn default_terrain_color() -> ColorRgba {
    [0.78, 0.86, 0.68, 1.0]
}
pub(super) fn default_sky_color() -> ColorRgba {
    [0.08, 0.16, 0.34, 1.0]
}
pub(super) fn default_sky_emissive() -> ColorRgb {
    [0.07, 0.14, 0.34]
}
pub(super) fn default_tree_bark_color() -> ColorRgba {
    [0.38, 0.23, 0.12, 1.0]
}
pub(super) fn default_tree_leaf_color() -> ColorRgba {
    [0.18, 0.42, 0.16, 1.0]
}
pub(super) fn default_tree_branch_color() -> ColorRgba {
    [0.32, 0.20, 0.12, 1.0]
}
pub(super) fn default_uv_scale() -> [f32; 2] {
    [1.0, 1.0]
}
pub(super) fn default_uv_offset() -> [f32; 2] {
    [0.0, 0.0]
}
pub(super) fn default_material_roughness() -> f32 {
    0.86
}
pub(super) fn default_material_normal_scale() -> f32 {
    1.0
}
pub(super) fn default_material_occlusion_strength() -> f32 {
    1.0
}
pub(super) fn default_ambient_color() -> ColorRgb {
    [0.42, 0.47, 0.56]
}
pub(super) fn default_ambient_intensity() -> f32 {
    0.36
}
pub(super) fn default_sun_direction() -> ColorRgb {
    [-0.55, -0.82, -0.28]
}
pub(super) fn default_sun_color() -> ColorRgb {
    [1.0, 0.955, 0.86]
}
pub(super) fn default_sun_intensity() -> f32 {
    4.60
}
pub(super) fn default_day_night_enabled() -> bool {
    true
}
pub(super) fn default_time_of_day_hours() -> f32 {
    9.35
}
pub(super) fn default_day_length_seconds() -> f32 {
    720.0
}
pub(super) fn default_sun_latitude_degrees() -> f32 {
    45.0
}
pub(super) fn default_axial_tilt_degrees() -> f32 {
    23.44
}
pub(super) fn default_shadow_enabled() -> bool {
    true
}
pub(super) fn default_shadow_resolution() -> u32 {
    4096
}
pub(super) fn default_shadow_cascade_count() -> u32 {
    4
}
pub(super) fn default_shadow_max_distance() -> f32 {
    180.0
}
pub(super) fn default_shadow_softness() -> f32 {
    0.62
}
pub(super) fn default_shadow_bias() -> f32 {
    0.0025
}
pub(super) fn default_shadow_normal_bias() -> f32 {
    0.015
}
pub(super) fn default_shadow_contact_strength() -> f32 {
    0.58
}
pub(super) fn default_foliage_prefab() -> String {
    String::new()
}
pub(super) fn default_foliage_seed() -> u64 {
    0x5452_4545_2026
}
pub(super) fn default_foliage_grid_min() -> i32 {
    -5
}
pub(super) fn default_foliage_grid_max() -> i32 {
    5
}
pub(super) fn default_foliage_spacing() -> f32 {
    6.0
}
pub(super) fn default_foliage_jitter() -> f32 {
    0.45
}
pub(super) fn default_foliage_gate_threshold() -> f32 {
    0.62
}
pub(super) fn default_foliage_min_scale() -> f32 {
    0.85
}
pub(super) fn default_foliage_max_scale() -> f32 {
    1.35
}
pub(super) fn default_foliage_min_player_distance() -> f32 {
    5.0
}
pub(super) fn default_foliage_edge_margin() -> f32 {
    4.0
}
pub(super) fn default_foliage_surface_offset() -> f32 {
    0.03
}
pub(super) fn default_prefab_proxy() -> String {
    String::new()
}
pub(super) fn default_prefab_enabled() -> bool {
    false
}
pub(super) fn default_terrain_material() -> RawMaterialSpec {
    RawMaterialSpec {
        roughness: default_material_roughness(),
        normal_scale: default_material_normal_scale(),
        occlusion_strength: default_material_occlusion_strength(),
        ..RawMaterialSpec::default()
    }
}

pub(super) fn default_sky_material() -> RawMaterialSpec {
    RawMaterialSpec {
        roughness: default_material_roughness(),
        normal_scale: default_material_normal_scale(),
        occlusion_strength: default_material_occlusion_strength(),
        ..RawMaterialSpec::default()
    }
}

pub(super) fn default_sun_material() -> RawMaterialSpec {
    RawMaterialSpec {
        roughness: default_material_roughness(),
        normal_scale: default_material_normal_scale(),
        occlusion_strength: default_material_occlusion_strength(),
        ..RawMaterialSpec::default()
    }
}

pub(super) fn default_moon_material() -> RawMaterialSpec {
    RawMaterialSpec {
        roughness: default_material_roughness(),
        normal_scale: default_material_normal_scale(),
        occlusion_strength: default_material_occlusion_strength(),
        ..RawMaterialSpec::default()
    }
}

pub(super) fn default_tree_bark_material() -> RawMaterialSpec {
    RawMaterialSpec {
        roughness: default_material_roughness(),
        normal_scale: default_material_normal_scale(),
        occlusion_strength: default_material_occlusion_strength(),
        ..RawMaterialSpec::default()
    }
}

pub(super) fn default_tree_leaf_material() -> RawMaterialSpec {
    RawMaterialSpec {
        roughness: default_material_roughness(),
        normal_scale: default_material_normal_scale(),
        occlusion_strength: default_material_occlusion_strength(),
        ..RawMaterialSpec::default()
    }
}

pub(super) fn default_tree_branch_material() -> RawMaterialSpec {
    RawMaterialSpec {
        roughness: default_material_roughness(),
        normal_scale: default_material_normal_scale(),
        occlusion_strength: default_material_occlusion_strength(),
        ..RawMaterialSpec::default()
    }
}
