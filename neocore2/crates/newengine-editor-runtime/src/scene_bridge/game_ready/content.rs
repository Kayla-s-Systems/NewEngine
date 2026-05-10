#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::PathBuf;

use newengine_math::Vec3;
use serde::Deserialize;

const GAME_READY_SCENE_ID: &str = "newengine.scene.game_ready.highlands.v1";

pub(super) type ColorRgba = [f32; 4];
pub(super) type ColorRgb = [f32; 3];

#[derive(Clone, Debug)]
pub(super) struct GameReadyMapProfile {
    pub(super) title: String,
    pub(super) objective: String,
    pub(super) player: GameReadyPlayerSpec,
    pub(super) terrain: GameReadyTerrainSpec,
    pub(super) sky: GameReadySkySpec,
    pub(super) materials: GameReadyMaterialSetSpec,
    pub(super) lighting: GameReadyLightingSpec,
    pub(super) gameplay: GameReadyGameplaySpec,
    pub(super) palette: GameReadyPaletteSpec,
}

#[derive(Clone, Debug)]
pub(super) struct GameReadyPlayerSpec {
    pub(super) start: Vec3,
    pub(super) yaw: f32,
    pub(super) move_speed: f32,
    pub(super) look_sens: f32,
}

#[derive(Clone, Debug)]
pub(super) struct GameReadyTerrainSpec {
    pub(super) seed: u64,
    pub(super) cells_x: u32,
    pub(super) cells_z: u32,
    pub(super) size_x: f32,
    pub(super) size_z: f32,
    pub(super) base_height: f32,
    pub(super) height_scale: f32,
    pub(super) collision_tile_cells: u32,
    pub(super) collision_floor_depth: f32,
    pub(super) collision_horizontal_skin: f32,
    pub(super) generator: GameReadyTerrainGeneratorSpec,
}

#[derive(Clone, Debug)]
pub(super) struct GameReadyTerrainGeneratorSpec {
    pub(super) id: String,
    pub(super) ridged_seed_xor: u64,
    pub(super) ridged_frequency: f32,
    pub(super) ridged_amplitude: f32,
    pub(super) ridged_shape_edge0: f32,
    pub(super) ridged_shape_edge1: f32,
    pub(super) veins_seed_xor: u64,
    pub(super) veins_frequency: f32,
    pub(super) veins_amplitude: f32,
}

#[derive(Clone, Debug)]
pub(super) struct GameReadySkySpec {
    pub(super) radius: f32,
    pub(super) mesh: String,
}

#[derive(Clone, Debug)]
pub(super) struct GameReadyPaletteSpec {
    pub(super) terrain: ColorRgba,
    pub(super) sky: ColorRgba,
    pub(super) sky_emissive: ColorRgb,
}

#[derive(Clone, Debug)]
pub(super) struct GameReadyMaterialSetSpec {
    pub(super) terrain: GameReadyMaterialSpec,
    pub(super) sky: GameReadyMaterialSpec,
}

#[derive(Clone, Debug)]
pub(super) struct GameReadyMaterialSpec {
    pub(super) base_color_texture: Option<String>,
    pub(super) normal_texture: Option<String>,
    pub(super) roughness_texture: Option<String>,
    pub(super) uv_scale: [f32; 2],
    pub(super) uv_offset: [f32; 2],
    pub(super) roughness: f32,
    pub(super) normal_scale: f32,
    pub(super) occlusion_strength: f32,
}

#[derive(Clone, Debug)]
pub(super) struct GameReadyLightingSpec {
    pub(super) ambient_color: ColorRgb,
    pub(super) ambient_intensity: f32,
    pub(super) sun_direction: ColorRgb,
    pub(super) sun_color: ColorRgb,
    pub(super) sun_intensity: f32,
    pub(super) shadows: GameReadyShadowSpec,
}

#[derive(Clone, Debug)]
pub(super) struct GameReadyShadowSpec {
    pub(super) enabled: bool,
    pub(super) resolution: u32,
    pub(super) cascade_count: u32,
    pub(super) max_distance: f32,
    pub(super) softness: f32,
    pub(super) bias: f32,
    pub(super) normal_bias: f32,
    pub(super) contact_strength: f32,
}

#[derive(Clone, Debug)]
pub(super) struct GameReadyGameplaySpec {
    pub(super) default_status: String,
    pub(super) pickup_status: String,
    pub(super) hazard_status: String,
    pub(super) goal_locked_status: String,
    pub(super) goal_complete_status: String,
    pub(super) failed_progress_label: String,
    pub(super) completed_progress_label: String,
    pub(super) player_collision: GameReadyPlayerCollisionSpec,
    pub(super) player_visual: GameReadyPlayerVisualSpec,
    pub(super) physics: GameReadyPhysicsSpec,
}

#[derive(Clone, Debug)]
pub(super) struct GameReadyPlayerCollisionSpec {
    pub(super) radius: f32,
    pub(super) half_height: f32,
}

#[derive(Clone, Debug)]
pub(super) struct GameReadyPlayerVisualSpec {
    pub(super) radius: f32,
    pub(super) half_height: f32,
    pub(super) camera_eye_height: f32,
    pub(super) sprint_multiplier: f32,
}

#[derive(Clone, Debug)]
pub(super) struct GameReadyPhysicsSpec {
    pub(super) gravity: f32,
    pub(super) contact_skin: f32,
}

#[derive(Debug, Deserialize)]
struct RawGameReadyPayload {
    #[serde(default = "default_title")]
    title: String,
    #[serde(default = "default_objective")]
    objective: String,
    #[serde(default)]
    player: RawPlayerSpec,
    #[serde(default)]
    terrain: RawTerrainSpec,
    #[serde(default)]
    sky: RawSkySpec,
    #[serde(default)]
    materials: RawMaterialSetSpec,
    #[serde(default)]
    lighting: RawLightingSpec,
    #[serde(default)]
    gameplay: RawGameplaySpec,
    #[serde(default)]
    palette: RawPaletteSpec,
}

#[derive(Debug, Deserialize)]
struct RawPlayerSpec {
    #[serde(default = "default_player_start")]
    start: [f32; 3],
    #[serde(default = "default_player_yaw")]
    yaw: f32,
    #[serde(default = "default_move_speed")]
    move_speed: f32,
    #[serde(default = "default_look_sens")]
    look_sens: f32,
}

impl Default for RawPlayerSpec {
    fn default() -> Self {
        Self {
            start: default_player_start(),
            yaw: default_player_yaw(),
            move_speed: default_move_speed(),
            look_sens: default_look_sens(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawTerrainSpec {
    #[serde(default = "default_terrain_seed")]
    seed: u64,
    #[serde(default = "default_terrain_cells")]
    cells_x: u32,
    #[serde(default = "default_terrain_cells")]
    cells_z: u32,
    #[serde(default = "default_terrain_size")]
    size_x: f32,
    #[serde(default = "default_terrain_size")]
    size_z: f32,
    #[serde(default = "default_base_height")]
    base_height: f32,
    #[serde(default = "default_height_scale")]
    height_scale: f32,
    #[serde(default = "default_collision_tile_cells")]
    collision_tile_cells: u32,
    #[serde(default = "default_collision_floor_depth")]
    collision_floor_depth: f32,
    #[serde(default = "default_collision_horizontal_skin")]
    collision_horizontal_skin: f32,
    #[serde(default)]
    generator: RawTerrainGeneratorSpec,
}

impl Default for RawTerrainSpec {
    fn default() -> Self {
        Self {
            seed: default_terrain_seed(),
            cells_x: default_terrain_cells(),
            cells_z: default_terrain_cells(),
            size_x: default_terrain_size(),
            size_z: default_terrain_size(),
            base_height: default_base_height(),
            height_scale: default_height_scale(),
            collision_tile_cells: default_collision_tile_cells(),
            collision_floor_depth: default_collision_floor_depth(),
            collision_horizontal_skin: default_collision_horizontal_skin(),
            generator: RawTerrainGeneratorSpec::default(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawTerrainGeneratorSpec {
    #[serde(default = "default_terrain_generator_id")]
    id: String,
    #[serde(default = "default_ridged_seed_xor")]
    ridged_seed_xor: u64,
    #[serde(default = "default_ridged_frequency")]
    ridged_frequency: f32,
    #[serde(default = "default_ridged_amplitude")]
    ridged_amplitude: f32,
    #[serde(default = "default_ridged_shape_edge0")]
    ridged_shape_edge0: f32,
    #[serde(default = "default_ridged_shape_edge1")]
    ridged_shape_edge1: f32,
    #[serde(default = "default_veins_seed_xor")]
    veins_seed_xor: u64,
    #[serde(default = "default_veins_frequency")]
    veins_frequency: f32,
    #[serde(default = "default_veins_amplitude")]
    veins_amplitude: f32,
}

impl Default for RawTerrainGeneratorSpec {
    fn default() -> Self {
        Self {
            id: default_terrain_generator_id(),
            ridged_seed_xor: default_ridged_seed_xor(),
            ridged_frequency: default_ridged_frequency(),
            ridged_amplitude: default_ridged_amplitude(),
            ridged_shape_edge0: default_ridged_shape_edge0(),
            ridged_shape_edge1: default_ridged_shape_edge1(),
            veins_seed_xor: default_veins_seed_xor(),
            veins_frequency: default_veins_frequency(),
            veins_amplitude: default_veins_amplitude(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawSkySpec {
    #[serde(default = "default_sky_radius")]
    radius: f32,
    #[serde(default = "default_skydome_mesh")]
    mesh: String,
}

impl Default for RawSkySpec {
    fn default() -> Self {
        Self {
            radius: default_sky_radius(),
            mesh: default_skydome_mesh(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawGameplaySpec {
    #[serde(default = "default_status_text")]
    default_status: String,
    #[serde(default = "default_pickup_status")]
    pickup_status: String,
    #[serde(default = "default_hazard_status")]
    hazard_status: String,
    #[serde(default = "default_goal_locked_status")]
    goal_locked_status: String,
    #[serde(default = "default_goal_complete_status")]
    goal_complete_status: String,
    #[serde(default = "default_failed_progress_label")]
    failed_progress_label: String,
    #[serde(default = "default_completed_progress_label")]
    completed_progress_label: String,
    #[serde(default)]
    player_collision: RawPlayerCollisionSpec,
    #[serde(default)]
    player_visual: RawPlayerVisualSpec,
    #[serde(default)]
    physics: RawPhysicsSpec,
}

impl Default for RawGameplaySpec {
    fn default() -> Self {
        Self {
            default_status: default_status_text(),
            pickup_status: default_pickup_status(),
            hazard_status: default_hazard_status(),
            goal_locked_status: default_goal_locked_status(),
            goal_complete_status: default_goal_complete_status(),
            failed_progress_label: default_failed_progress_label(),
            completed_progress_label: default_completed_progress_label(),
            player_collision: RawPlayerCollisionSpec::default(),
            player_visual: RawPlayerVisualSpec::default(),
            physics: RawPhysicsSpec::default(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawPlayerCollisionSpec {
    #[serde(default = "default_player_body_radius")]
    radius: f32,
    #[serde(default = "default_player_body_half_height")]
    half_height: f32,
}

impl Default for RawPlayerCollisionSpec {
    fn default() -> Self {
        Self {
            radius: default_player_body_radius(),
            half_height: default_player_body_half_height(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawPlayerVisualSpec {
    #[serde(default = "default_player_visual_radius")]
    radius: f32,
    #[serde(default = "default_player_visual_half_height")]
    half_height: f32,
    #[serde(default = "default_camera_eye_height")]
    camera_eye_height: f32,
    #[serde(default = "default_sprint_multiplier")]
    sprint_multiplier: f32,
}

impl Default for RawPlayerVisualSpec {
    fn default() -> Self {
        Self {
            radius: default_player_visual_radius(),
            half_height: default_player_visual_half_height(),
            camera_eye_height: default_camera_eye_height(),
            sprint_multiplier: default_sprint_multiplier(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawPhysicsSpec {
    #[serde(default = "default_gravity")]
    gravity: f32,
    #[serde(default = "default_contact_skin")]
    contact_skin: f32,
}

impl Default for RawPhysicsSpec {
    fn default() -> Self {
        Self {
            gravity: default_gravity(),
            contact_skin: default_contact_skin(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawPaletteSpec {
    #[serde(default = "default_terrain_color")]
    terrain: ColorRgba,
    #[serde(default = "default_sky_color")]
    sky: ColorRgba,
    #[serde(default = "default_sky_emissive")]
    sky_emissive: ColorRgb,
}

impl Default for RawPaletteSpec {
    fn default() -> Self {
        Self {
            terrain: default_terrain_color(),
            sky: default_sky_color(),
            sky_emissive: default_sky_emissive(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawMaterialSetSpec {
    #[serde(default = "default_terrain_material")]
    terrain: RawMaterialSpec,
    #[serde(default = "default_sky_material")]
    sky: RawMaterialSpec,
}

impl Default for RawMaterialSetSpec {
    fn default() -> Self {
        Self {
            terrain: default_terrain_material(),
            sky: default_sky_material(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct RawMaterialSpec {
    #[serde(default)]
    base_color_texture: Option<String>,
    #[serde(default)]
    normal_texture: Option<String>,
    #[serde(default)]
    roughness_texture: Option<String>,
    #[serde(default = "default_uv_scale")]
    uv_scale: [f32; 2],
    #[serde(default = "default_uv_offset")]
    uv_offset: [f32; 2],
    #[serde(default = "default_material_roughness")]
    roughness: f32,
    #[serde(default = "default_material_normal_scale")]
    normal_scale: f32,
    #[serde(default = "default_material_occlusion_strength")]
    occlusion_strength: f32,
}

impl Default for RawMaterialSpec {
    fn default() -> Self {
        Self {
            base_color_texture: None,
            normal_texture: None,
            roughness_texture: None,
            uv_scale: default_uv_scale(),
            uv_offset: default_uv_offset(),
            roughness: default_material_roughness(),
            normal_scale: default_material_normal_scale(),
            occlusion_strength: default_material_occlusion_strength(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawLightingSpec {
    #[serde(default = "default_ambient_color")]
    ambient_color: ColorRgb,
    #[serde(default = "default_ambient_intensity")]
    ambient_intensity: f32,
    #[serde(default = "default_sun_direction")]
    sun_direction: ColorRgb,
    #[serde(default = "default_sun_color")]
    sun_color: ColorRgb,
    #[serde(default = "default_sun_intensity")]
    sun_intensity: f32,
    #[serde(default)]
    shadows: RawShadowSpec,
}

impl Default for RawLightingSpec {
    fn default() -> Self {
        Self {
            ambient_color: default_ambient_color(),
            ambient_intensity: default_ambient_intensity(),
            sun_direction: default_sun_direction(),
            sun_color: default_sun_color(),
            sun_intensity: default_sun_intensity(),
            shadows: RawShadowSpec::default(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawShadowSpec {
    #[serde(default = "default_shadow_enabled")]
    enabled: bool,
    #[serde(default = "default_shadow_resolution")]
    resolution: u32,
    #[serde(default = "default_shadow_cascade_count")]
    cascade_count: u32,
    #[serde(default = "default_shadow_max_distance")]
    max_distance: f32,
    #[serde(default = "default_shadow_softness")]
    softness: f32,
    #[serde(default = "default_shadow_bias")]
    bias: f32,
    #[serde(default = "default_shadow_normal_bias")]
    normal_bias: f32,
    #[serde(default = "default_shadow_contact_strength")]
    contact_strength: f32,
}

impl Default for RawShadowSpec {
    fn default() -> Self {
        Self {
            enabled: default_shadow_enabled(),
            resolution: default_shadow_resolution(),
            cascade_count: default_shadow_cascade_count(),
            max_distance: default_shadow_max_distance(),
            softness: default_shadow_softness(),
            bias: default_shadow_bias(),
            normal_bias: default_shadow_normal_bias(),
            contact_strength: default_shadow_contact_strength(),
        }
    }
}

pub(super) fn load_game_ready_map_profile() -> GameReadyMapProfile {
    for path in profile_file_candidates() {
        match load_profile_file(&path) {
            Ok(profile) => {
                log::info!(
                    "game-ready: loaded standalone scene profile path='{}'",
                    path.display(),
                );
                return profile;
            }
            Err(e) => log::debug!(
                "game-ready: standalone scene profile unavailable path='{}' err='{}'",
                path.display(),
                e,
            ),
        }
    }

    for dir in plugin_dir_candidates() {
        match newengine_plugin_host::load_plugin_content_catalog_from_dir(&dir) {
            Ok(report) => {
                let Some(blob) = report.catalog.find_scene(GAME_READY_SCENE_ID) else {
                    continue;
                };
                match parse_payload(blob.payload.clone()) {
                    Ok(profile) => {
                        log::info!(
                            "game-ready: loaded scene profile id='{}' provider='{}' path='{}'",
                            blob.id,
                            blob.provider_plugin,
                            report.path.display(),
                        );
                        return profile;
                    }
                    Err(e) => log::warn!(
                        "game-ready: plugin scene profile ignored id='{}' path='{}' err='{}'",
                        blob.id,
                        report.path.display(),
                        e,
                    ),
                }
            }
            Err(e) => log::debug!(
                "game-ready: plugin content catalog unavailable dir='{}' err='{}'",
                dir.display(),
                e,
            ),
        }
    }

    log::warn!("game-ready: using built-in fallback scene profile; plugin content catalog not found");
    fallback_game_ready_map_profile()
}

fn load_profile_file(path: &std::path::Path) -> Result<GameReadyMapProfile, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read failed: {e}"))?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| format!("json parse failed: {e}"))?;

    if let Some(scene) = value.get("scene").cloned() {
        parse_payload(scene)
    } else if let Some(payload) = value.get("payload").cloned() {
        parse_payload(payload)
    } else {
        parse_payload(value)
    }
}

fn parse_payload(value: serde_json::Value) -> Result<GameReadyMapProfile, String> {
    let raw: RawGameReadyPayload = serde_json::from_value(value)
        .map_err(|e| format!("scene payload parse failed: {e}"))?;
    Ok(raw.into_profile())
}

impl RawGameReadyPayload {
    fn into_profile(self) -> GameReadyMapProfile {
        GameReadyMapProfile {
            title: self.title,
            objective: self.objective,
            player: GameReadyPlayerSpec {
                start: arr3(self.player.start),
                yaw: self.player.yaw,
                move_speed: self.player.move_speed,
                look_sens: self.player.look_sens,
            },
            terrain: GameReadyTerrainSpec {
                seed: self.terrain.seed,
                cells_x: self.terrain.cells_x.max(8),
                cells_z: self.terrain.cells_z.max(8),
                size_x: self.terrain.size_x.max(4.0),
                size_z: self.terrain.size_z.max(4.0),
                base_height: self.terrain.base_height,
                height_scale: self.terrain.height_scale.max(0.1),
                collision_tile_cells: self.terrain.collision_tile_cells.max(1),
                collision_floor_depth: self.terrain.collision_floor_depth.max(0.1),
                collision_horizontal_skin: self.terrain.collision_horizontal_skin.max(0.0),
                generator: GameReadyTerrainGeneratorSpec {
                    id: self.terrain.generator.id,
                    ridged_seed_xor: self.terrain.generator.ridged_seed_xor,
                    ridged_frequency: self.terrain.generator.ridged_frequency.max(0.001),
                    ridged_amplitude: self.terrain.generator.ridged_amplitude,
                    ridged_shape_edge0: self.terrain.generator.ridged_shape_edge0,
                    ridged_shape_edge1: self.terrain.generator.ridged_shape_edge1,
                    veins_seed_xor: self.terrain.generator.veins_seed_xor,
                    veins_frequency: self.terrain.generator.veins_frequency.max(0.001),
                    veins_amplitude: self.terrain.generator.veins_amplitude,
                },
            },
            sky: GameReadySkySpec {
                radius: self.sky.radius.max(16.0),
                mesh: non_empty_or(self.sky.mesh, default_skydome_mesh()),
            },
            materials: GameReadyMaterialSetSpec {
                terrain: sanitize_material_spec(self.materials.terrain),
                sky: sanitize_material_spec(self.materials.sky),
            },
            lighting: sanitize_lighting_spec(self.lighting),
            gameplay: GameReadyGameplaySpec {
                default_status: non_empty_or(self.gameplay.default_status, default_status_text()),
                pickup_status: non_empty_or(self.gameplay.pickup_status, default_pickup_status()),
                hazard_status: non_empty_or(self.gameplay.hazard_status, default_hazard_status()),
                goal_locked_status: non_empty_or(self.gameplay.goal_locked_status, default_goal_locked_status()),
                goal_complete_status: non_empty_or(self.gameplay.goal_complete_status, default_goal_complete_status()),
                failed_progress_label: non_empty_or(self.gameplay.failed_progress_label, default_failed_progress_label()),
                completed_progress_label: non_empty_or(self.gameplay.completed_progress_label, default_completed_progress_label()),
                player_collision: GameReadyPlayerCollisionSpec {
                    radius: self.gameplay.player_collision.radius.clamp(0.05, 5.0),
                    half_height: self.gameplay.player_collision.half_height.clamp(0.05, 8.0),
                },
                player_visual: GameReadyPlayerVisualSpec {
                    radius: self.gameplay.player_visual.radius.clamp(0.05, 8.0),
                    half_height: self.gameplay.player_visual.half_height.clamp(0.05, 12.0),
                    camera_eye_height: self.gameplay.player_visual.camera_eye_height.clamp(0.05, 12.0),
                    sprint_multiplier: self.gameplay.player_visual.sprint_multiplier.clamp(1.0, 8.0),
                },
                physics: GameReadyPhysicsSpec {
                    gravity: self.gameplay.physics.gravity.clamp(0.0, 80.0),
                    contact_skin: self.gameplay.physics.contact_skin.clamp(0.0, 0.50),
                },
            },
            palette: GameReadyPaletteSpec {
                terrain: self.palette.terrain,
                sky: self.palette.sky,
                sky_emissive: self.palette.sky_emissive,
            },
        }
    }
}

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
            gameplay: RawGameplaySpec::default(),
            palette: RawPaletteSpec::default(),
        }
    }
}

fn profile_file_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();

    for key in ["NEWENGINE_GAME_READY_PROFILE", "NEWENGINE_GAME_READY_SCENE"] {
        if let Ok(raw) = std::env::var(key) {
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                out.push(PathBuf::from(trimmed));
            }
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        out.push(cwd.join("assets").join("game_ready_highlands.scene.json"));
        out.push(cwd.join("apps").join("game-ready-fps").join("assets").join("game_ready_highlands.scene.json"));
        out.push(cwd.join("NewEngine").join("neocore2").join("apps").join("game-ready-fps").join("assets").join("game_ready_highlands.scene.json"));
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(debug_dir) = exe.parent() {
            out.push(debug_dir.join("assets").join("game_ready_highlands.scene.json"));
            if let Some(profile_dir) = debug_dir.parent() {
                if let Some(target_dir) = profile_dir.parent() {
                    if let Some(workspace_dir) = target_dir.parent() {
                        out.push(workspace_dir.join("apps").join("game-ready-fps").join("assets").join("game_ready_highlands.scene.json"));
                    }
                }
            }
        }
    }

    dedup_paths(out)
}

fn plugin_dir_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for key in ["NEWENGINE_PLUGIN_DIR", "NEWENGINE_PLUGINS_DIR", "NEWENGINE_MODULES_DIR"] {
        if let Ok(raw) = std::env::var(key) {
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                out.push(PathBuf::from(trimmed));
            }
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        out.push(cwd.join("plugins"));
        out.push(cwd.join("NewEngine").join("neocore2").join("plugins"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            out.push(parent.to_path_buf());
            out.push(parent.join("plugins"));
            if let Some(profile) = parent.parent() {
                out.push(profile.join("plugins"));
                if let Some(target) = profile.parent() {
                    out.push(target.join("plugins"));
                }
            }
        }
    }
    dedup_paths(out)
}

fn dedup_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for path in paths {
        if !out.iter().any(|existing| existing == &path) {
            out.push(path);
        }
    }
    out
}

#[inline]
fn non_empty_or(value: String, fallback: String) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() { fallback } else { trimmed.to_owned() }
}

#[inline]
fn sanitize_texture_path(value: Option<String>) -> Option<String> {
    value.and_then(|s| {
        let trimmed = s.trim();
        if trimmed.is_empty() { None } else { Some(trimmed.to_owned()) }
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
fn arr3(v: [f32; 3]) -> Vec3 { Vec3::new(v[0], v[1], v[2]) }

fn default_title() -> String { "KAYLA FPS: Procedural Highlands".to_owned() }
fn default_objective() -> String { "Walk a deterministic terrain sandbox with an imported sky dome.".to_owned() }
fn default_player_start() -> [f32; 3] { [-17.5, 0.0, -17.5] }
fn default_player_yaw() -> f32 { -0.72 }
fn default_move_speed() -> f32 { 7.3 }
fn default_look_sens() -> f32 { 0.0022 }
fn default_terrain_seed() -> u64 { 0x2026_0509_4b41_594c }
fn default_terrain_cells() -> u32 { 128 }
fn default_terrain_size() -> f32 { 52.0 }
fn default_base_height() -> f32 { -0.08 }
fn default_height_scale() -> f32 { 3.15 }
fn default_collision_tile_cells() -> u32 { 8 }
fn default_collision_floor_depth() -> f32 { 3.0 }
fn default_collision_horizontal_skin() -> f32 { 0.08 }
fn default_terrain_generator_id() -> String { "newengine.generator.heightfield.soft-cells.v1".to_owned() }
fn default_ridged_seed_xor() -> u64 { 0x7e22_a11d }
fn default_ridged_frequency() -> f32 { 1.85 }
fn default_ridged_amplitude() -> f32 { 0.42 }
fn default_ridged_shape_edge0() -> f32 { -0.35 }
fn default_ridged_shape_edge1() -> f32 { 1.0 }
fn default_veins_seed_xor() -> u64 { 0x5317_1001 }
fn default_veins_frequency() -> f32 { 0.68 }
fn default_veins_amplitude() -> f32 { 0.18 }
fn default_sky_radius() -> f32 { 220.0 }
fn default_skydome_mesh() -> String { "skydome/skydome_high.obj".to_owned() }
fn default_status_text() -> String { "Terrain sandbox: procedural heightfield + imported sky dome.".to_owned() }
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
fn default_camera_eye_height() -> f32 { 0.85 }
fn default_sprint_multiplier() -> f32 { 1.75 }
fn default_gravity() -> f32 { 9.81 }
fn default_contact_skin() -> f32 { 0.035 }
fn default_terrain_color() -> ColorRgba { [0.78, 0.86, 0.68, 1.0] }
fn default_sky_color() -> ColorRgba { [0.08, 0.16, 0.34, 1.0] }
fn default_sky_emissive() -> ColorRgb { [0.07, 0.14, 0.34] }
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
fn default_shadow_enabled() -> bool { true }
fn default_shadow_resolution() -> u32 { 2048 }
fn default_shadow_cascade_count() -> u32 { 1 }
fn default_shadow_max_distance() -> f32 { 80.0 }
fn default_shadow_softness() -> f32 { 1.35 }
fn default_shadow_bias() -> f32 { 0.0025 }
fn default_shadow_normal_bias() -> f32 { 0.015 }
fn default_shadow_contact_strength() -> f32 { 0.35 }
fn default_terrain_material() -> RawMaterialSpec {
    RawMaterialSpec {
        base_color_texture: Some("textures/fps/terrain_forest_floor.jpg".to_owned()),
        normal_texture: Some("textures/fps/terrain_forest_floor_normal.jpg".to_owned()),
        roughness_texture: Some("textures/fps/terrain_forest_floor_roughness.jpg".to_owned()),
        uv_scale: [10.0, 10.0],
        uv_offset: [0.0, 0.0],
        roughness: 0.92,
        normal_scale: 1.15,
        occlusion_strength: 0.95,
    }
}
fn default_sky_material() -> RawMaterialSpec {
    RawMaterialSpec {
        roughness: 1.0,
        normal_scale: 0.0,
        occlusion_strength: 0.0,
        ..RawMaterialSpec::default()
    }
}
