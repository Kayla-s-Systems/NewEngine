#![forbid(unsafe_op_in_unsafe_fn)]

//! Central data-only game configuration.
//!
//! Runtime/gameplay systems must consume these values through a snapshot/provider boundary
//! instead of embedding product constants in execution code. The current provider is Rust-owned;
//! a Lua provider can later populate the exact same [`GameData`] schema.

use serde::{Deserialize, Serialize};
use std::sync::{Arc, OnceLock};

pub const GAME_DATA_SCHEMA: &str = "newengine.game_data.v1";
pub const GAME_DATA_VERSION: u32 = 1;

pub const GAME_FIXED_DT_MS: u32 = 16;
pub const GAME_APP_ASSETS_DIR_ENV: &str = "NEWENGINE_GAME_ASSETS_DIR";
pub const GAME_READY_APP_DIR_NAME: &str = "game-ready-fps";
pub const GAME_READY_FPS_APP_NAME: &str = "game-ready-fps";
pub const GAME_READY_FPS_WINDOW_TITLE: &str = "North Star Game Ready FPS";
pub const GAME_READY_FPS_EARLY_LOG_FILE: &str = "game-ready-fps-early.log";
pub const GAME_READY_PROFILE_ENV: &str = "NEWENGINE_SCENE_PROFILE";
pub const GAME_READY_DEFAULT_PROFILE_ASSET: &str = "maps/white_platform.ymap";

pub const DEFAULT_RIFLE_ITEM_NAME: &str = "weapon.rifle.standard";
pub const DEFAULT_RIFLE_AMMO_NAME: &str = "ammo.rifle.standard";
pub const DEFAULT_MEDKIT_ITEM_NAME: &str = "consumable.medkit.standard";
pub const DEFAULT_FPS_LOADOUT_NAME: &str = "loadout.fps.default";
pub const DEFAULT_ITEM_PACKAGE_ASSET: &str = "items/fps_items.neitems";
pub const WORLD_MATERIAL_LIBRARY: &str = "materials/world_game_ready.nemat";
pub const MISSION_MATERIAL_LIBRARY: &str = "materials/world_game_ready.nemat";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GameData {
    pub schema: String,
    pub version: u32,
    pub runtime: RuntimeData,
    pub world: WorldData,
    pub player: PlayerData,
    pub gameplay: GameplayData,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RuntimeData {
    pub fixed_dt_ms: u32,
    pub app_name: String,
    pub app_dir_name: String,
    pub window_title: String,
    pub default_profile_asset: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldData {
    pub title: String,
    pub objective: String,
    pub terrain: TerrainData,
    pub sky: SkyData,
    pub palette: PaletteData,
    pub material: MaterialDefaultsData,
    pub lighting: LightingData,
    pub shadows: ShadowData,
    pub day_night: DayNightData,
    pub foliage: FoliageData,
    pub mission: MissionDefaultsData,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlayerData {
    pub spawn: [f32; 3],
    pub yaw: f32,
    pub move_speed: f32,
    pub look_sensitivity: f32,
    pub model: PlayerModelData,
    pub tuning: PlayerTuningData,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlayerModelData {
    pub enabled: bool,
    pub source: String,
    pub target_height: f32,
    pub eye_height_ratio: f32,
    pub local_offset: [f32; 3],
    pub yaw_offset: f32,
    pub hide_in_first_person: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlayerTuningData {
    pub body_radius: f32,
    pub body_half_height: f32,
    pub crouched_body_half_height: f32,
    pub visual_radius: f32,
    pub visual_half_height: f32,
    pub camera_eye_height: f32,
    pub crouched_camera_eye_height: f32,
    pub crouch_camera_speed: f32,
    pub sprint_multiplier: f32,
    pub jump_speed: f32,
    pub gravity: f32,
    pub contact_skin: f32,
    pub ground_probe_distance: f32,
    pub max_slope_degrees: f32,
    pub footstep_stride: f32,
    pub landing_speed_threshold: f32,
    pub locomotion_min_horizontal_speed: f32,
    pub ground_probe_max_upward_velocity: f32,
    pub landing_min_airborne_seconds: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GameplayData {
    pub status: GameplayStatusData,
    pub projectile: ProjectileData,
    pub inventory: InventoryDefaultsData,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GameplayStatusData {
    pub default_status: String,
    pub pickup_status: String,
    pub target_status: String,
    pub hazard_status: String,
    pub goal_locked_status: String,
    pub goal_complete_status: String,
    pub failed_progress_label: String,
    pub completed_progress_label: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectileData {
    pub radius: f32,
    pub speed: f32,
    pub lifetime_seconds: f32,
    pub spawn_clearance: f32,
    pub restitution: f32,
    pub friction: f32,
    pub density: f32,
    pub angular_velocity: [f32; 3],
    pub color: [f32; 4],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InventoryDefaultsData {
    pub rifle_item: String,
    pub rifle_ammo: String,
    pub medkit_item: String,
    pub loadout: String,
    pub package_asset: String,
    pub hud_slots: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TerrainData {
    pub enabled: bool,
    pub seed: u64,
    pub cells: u32,
    pub size: f32,
    pub base_height: f32,
    pub height_scale: f32,
    pub generator: TerrainGeneratorData,
    pub surface: TerrainSurfaceData,
    pub heightmap: TerrainHeightmapData,
    pub streaming: TerrainStreamingData,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TerrainGeneratorData {
    pub id: String,
    pub ridged_seed_xor: u64,
    pub ridged_frequency: f32,
    pub ridged_amplitude: f32,
    pub ridged_shape_edge0: f32,
    pub ridged_shape_edge1: f32,
    pub veins_seed_xor: u64,
    pub veins_frequency: f32,
    pub veins_amplitude: f32,
    pub smoothing_passes: u32,
    pub smoothing_strength: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TerrainSurfaceData {
    pub forest_texture: String,
    pub sand_texture: String,
    pub rock_texture: String,
    pub patch_scale: f32,
    pub blend_softness: f32,
    pub layer_weight: f32,
    pub layer_uv_scale: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TerrainHeightmapData {
    pub enabled: bool,
    pub source: String,
    pub mode: String,
    pub strength: f32,
    pub min_height: f32,
    pub max_height: f32,
    pub tile_scale: [f32; 2],
    pub tile_offset: [f32; 2],
    pub invert: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TerrainStreamingData {
    pub enabled: bool,
    pub chunk_radius: i32,
    pub unload_radius: i32,
    pub max_chunks_per_frame: usize,
    pub launch_warm_radius: i32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkyData {
    pub definition_ref: String,
    pub radius: f32,
    pub mesh: String,
    pub follow_camera: bool,
    pub cloud_dictionary: String,
    pub cloud_profile: String,
    pub sun_radius: f32,
    pub moon_radius: f32,
    pub moon_texture: String,
    pub atmosphere: SkyAtmosphereData,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkyAtmosphereData {
    pub day_zenith: [f32; 3],
    pub day_horizon: [f32; 3],
    pub dusk_zenith: [f32; 3],
    pub dusk_horizon: [f32; 3],
    pub night_zenith: [f32; 3],
    pub night_horizon: [f32; 3],
    pub cloud_day: [f32; 3],
    pub cloud_night: [f32; 3],
    pub night_sky_strength: f32,
    pub cloud_coverage: f32,
    pub cloud_softness: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PaletteData {
    pub terrain: [f32; 4],
    pub sky: [f32; 4],
    pub sky_emissive: [f32; 3],
    pub tree_bark: [f32; 4],
    pub tree_leaf: [f32; 4],
    pub tree_branch: [f32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MaterialDefaultsData {
    pub uv_scale: [f32; 2],
    pub uv_offset: [f32; 2],
    pub roughness: f32,
    pub normal_scale: f32,
    pub occlusion_strength: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LightingData {
    pub ambient_color: [f32; 3],
    pub ambient_intensity: f32,
    pub sun_direction: [f32; 3],
    pub sun_color: [f32; 3],
    pub sun_intensity: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShadowData {
    pub enabled: bool,
    pub resolution: u32,
    pub cascade_count: u32,
    pub max_distance: f32,
    pub softness: f32,
    pub bias: f32,
    pub normal_bias: f32,
    pub contact_strength: f32,
    pub filter: String,
    pub pcss_light_angular_radius_degrees: f32,
    pub pcss_blocker_search_radius_texels: f32,
    pub pcss_max_filter_radius_texels: f32,
    pub pcss_blocker_samples: u32,
    pub pcss_filter_samples: u32,
    pub pcss_min_filter_radius_texels: f32,
    pub pcss_stable_kernel_cell_texels: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DayNightData {
    pub enabled: bool,
    pub time_of_day_hours: f32,
    pub day_length_seconds: f32,
    pub day_of_year: u32,
    pub latitude_degrees: f32,
    pub axial_tilt_degrees: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FoliageData {
    pub enabled: bool,
    pub prefab: String,
    pub seed: u64,
    pub grid_min: i32,
    pub grid_max: i32,
    pub spacing: f32,
    pub jitter: f32,
    pub gate_threshold: f32,
    pub max_count: u32,
    pub min_scale: f32,
    pub max_scale: f32,
    pub min_player_distance: f32,
    pub edge_margin: f32,
    pub surface_offset: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MissionDefaultsData {
    pub pickup_radius: f32,
    pub pickup_scale: [f32; 3],
    pub target_health: f32,
    pub target_scale: [f32; 3],
    pub hazard_radius: f32,
    pub hazard_scale: [f32; 3],
    pub goal_radius: f32,
    pub goal_scale: [f32; 3],
}

impl GameData {
    /// Validates the stable data contract before a provider snapshot enters the runtime world.
    /// Lua/native providers must fail here instead of leaking invalid numbers into hot systems.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != GAME_DATA_SCHEMA {
            return Err(format!(
                "unsupported game-data schema '{}' expected '{}'",
                self.schema, GAME_DATA_SCHEMA
            ));
        }
        if self.version != GAME_DATA_VERSION {
            return Err(format!(
                "unsupported game-data version {} expected {}",
                self.version, GAME_DATA_VERSION
            ));
        }
        if self.runtime.fixed_dt_ms == 0 {
            return Err("runtime.fixed_dt_ms must be greater than zero".to_owned());
        }
        if self.runtime.app_name.trim().is_empty()
            || self.runtime.default_profile_asset.trim().is_empty()
        {
            return Err("runtime app_name/default_profile_asset must be non-empty".to_owned());
        }
        let tuning = self.player.tuning;
        let scalars = [
            self.player.yaw,
            self.player.move_speed,
            self.player.look_sensitivity,
            tuning.body_radius,
            tuning.body_half_height,
            tuning.crouched_body_half_height,
            tuning.camera_eye_height,
            tuning.crouched_camera_eye_height,
            tuning.sprint_multiplier,
            tuning.jump_speed,
            tuning.gravity,
            tuning.contact_skin,
            tuning.ground_probe_distance,
            tuning.max_slope_degrees,
            self.gameplay.projectile.radius,
            self.gameplay.projectile.speed,
            self.world.terrain.size,
            self.world.terrain.base_height,
            self.world.terrain.height_scale,
            self.world.sky.radius,
        ];
        if scalars.iter().any(|value| !value.is_finite())
            || self.player.spawn.iter().any(|value| !value.is_finite())
        {
            return Err("game-data contains non-finite runtime values".to_owned());
        }
        if tuning.body_radius <= 0.0
            || tuning.body_half_height <= 0.0
            || tuning.crouched_body_half_height <= 0.0
            || self.gameplay.projectile.radius <= 0.0
            || self.world.terrain.size <= 0.0
            || self.world.sky.radius <= 0.0
        {
            return Err("game-data contains non-positive physical dimensions".to_owned());
        }
        Ok(())
    }
}

impl Default for GameData {
    fn default() -> Self {
        Self {
            schema: GAME_DATA_SCHEMA.to_owned(),
            version: GAME_DATA_VERSION,
            runtime: RuntimeData {
                fixed_dt_ms: GAME_FIXED_DT_MS,
                app_name: GAME_READY_FPS_APP_NAME.to_owned(),
                app_dir_name: GAME_READY_APP_DIR_NAME.to_owned(),
                window_title: GAME_READY_FPS_WINDOW_TITLE.to_owned(),
                default_profile_asset: GAME_READY_DEFAULT_PROFILE_ASSET.to_owned(),
            },
            world: WorldData {
                title: "KAYLA FPS: Procedural Highlands".to_owned(),
                objective: "Walk a deterministic map assembled from .ymap -> .ytyp -> .ydd -> .nemat -> .ytd assets.".to_owned(),
                terrain: TerrainData {
                    enabled: true,
                    seed: 0x2026_0509_4b41_594c,
                    cells: 80,
                    size: 52.0,
                    base_height: -0.04,
                    height_scale: 1.35,
                    generator: TerrainGeneratorData {
                        id: "newengine.generator.lowland-biomes.v1".to_owned(),
                        ridged_seed_xor: 0x7e22_a11d,
                        ridged_frequency: 1.25,
                        ridged_amplitude: 0.11,
                        ridged_shape_edge0: 0.08,
                        ridged_shape_edge1: 1.0,
                        veins_seed_xor: 0x5317_1001,
                        veins_frequency: 0.52,
                        veins_amplitude: 0.10,
                        smoothing_passes: 2,
                        smoothing_strength: 0.42,
                    },
                    surface: TerrainSurfaceData {
                        forest_texture: String::new(),
                        sand_texture: String::new(),
                        rock_texture: String::new(),
                        patch_scale: 0.033,
                        blend_softness: 0.18,
                        layer_weight: 1.0,
                        layer_uv_scale: 1.0,
                    },
                    heightmap: TerrainHeightmapData {
                        enabled: false,
                        source: String::new(),
                        mode: "blend".to_owned(),
                        strength: 0.0,
                        min_height: -1.0,
                        max_height: 1.0,
                        tile_scale: [1.0, 1.0],
                        tile_offset: [0.0, 0.0],
                        invert: false,
                    },
                    streaming: TerrainStreamingData {
                        enabled: true,
                        chunk_radius: 2,
                        unload_radius: 4,
                        max_chunks_per_frame: 4,
                        launch_warm_radius: 1,
                    },
                },
                sky: SkyData {
                    definition_ref: "definitions/fps/sky_clear_morning.ytyp@sky_clear_morning".to_owned(),
                    radius: 220.0,
                    mesh: String::new(),
                    follow_camera: true,
                    cloud_dictionary: String::new(),
                    cloud_profile: "clear".to_owned(),
                    sun_radius: 18.0,
                    moon_radius: 13.5,
                    moon_texture: String::new(),
                    atmosphere: SkyAtmosphereData {
                        day_zenith: [0.23, 0.42, 0.82],
                        day_horizon: [0.64, 0.78, 0.96],
                        dusk_zenith: [0.16, 0.20, 0.40],
                        dusk_horizon: [1.00, 0.47, 0.20],
                        night_zenith: [0.006, 0.010, 0.030],
                        night_horizon: [0.020, 0.024, 0.052],
                        cloud_day: [0.96, 0.98, 1.00],
                        cloud_night: [0.040, 0.050, 0.085],
                        night_sky_strength: 0.35,
                        // Authored fallback is the neutral clear-sky baseline.
                        // Broken/overcast coverage belongs to the environment/weather provider.
                        cloud_coverage: 0.16,
                        cloud_softness: 0.68,
                    },
                },
                palette: PaletteData {
                    terrain: [0.78, 0.86, 0.68, 1.0],
                    sky: [0.08, 0.16, 0.34, 1.0],
                    sky_emissive: [0.07, 0.14, 0.34],
                    tree_bark: [0.38, 0.23, 0.12, 1.0],
                    tree_leaf: [0.18, 0.42, 0.16, 1.0],
                    tree_branch: [0.32, 0.20, 0.12, 1.0],
                },
                material: MaterialDefaultsData {
                    uv_scale: [1.0, 1.0],
                    uv_offset: [0.0, 0.0],
                    roughness: 0.86,
                    normal_scale: 1.0,
                    occlusion_strength: 1.0,
                },
                lighting: LightingData {
                    ambient_color: [0.42, 0.47, 0.56],
                    ambient_intensity: 0.52,
                    sun_direction: [-0.55, -0.82, -0.28],
                    sun_color: [1.0, 0.955, 0.86],
                    sun_intensity: 4.60,
                },
                shadows: ShadowData {
                    enabled: true,
                    resolution: 4096,
                    cascade_count: 4,
                    max_distance: 180.0,
                    softness: 1.0,
                    bias: 0.0025,
                    normal_bias: 0.015,
                    contact_strength: 0.58,
                    filter: "pcss".to_owned(),
                    pcss_light_angular_radius_degrees: 0.266,
                    pcss_blocker_search_radius_texels: 3.0,
                    pcss_max_filter_radius_texels: 5.0,
                    pcss_blocker_samples: 10,
                    pcss_filter_samples: 12,
                    pcss_min_filter_radius_texels: 0.18,
                    pcss_stable_kernel_cell_texels: 8.0,
                },
                day_night: DayNightData {
                    enabled: true,
                    time_of_day_hours: 9.35,
                    day_length_seconds: 720.0,
                    day_of_year: 172,
                    latitude_degrees: 45.0,
                    axial_tilt_degrees: 23.44,
                },
                foliage: FoliageData {
                    enabled: false,
                    prefab: String::new(),
                    seed: 0x5452_4545_2026,
                    grid_min: -5,
                    grid_max: 5,
                    spacing: 6.0,
                    jitter: 0.45,
                    gate_threshold: 0.62,
                    max_count: 0,
                    min_scale: 0.85,
                    max_scale: 1.35,
                    min_player_distance: 5.0,
                    edge_margin: 4.0,
                    surface_offset: 0.03,
                },
                mission: MissionDefaultsData {
                    pickup_radius: 0.8,
                    pickup_scale: [0.38, 0.38, 0.38],
                    target_health: 75.0,
                    target_scale: [0.55, 1.05, 0.55],
                    hazard_radius: 1.5,
                    hazard_scale: [1.45, 0.08, 1.45],
                    goal_radius: 2.0,
                    goal_scale: [1.8, 1.8, 1.8],
                },
            },
            player: PlayerData {
                spawn: [-17.5, 0.0, -17.5],
                yaw: -0.72,
                move_speed: 7.3,
                look_sensitivity: 0.0022,
                model: PlayerModelData {
                    enabled: false,
                    source: String::new(),
                    target_height: 1.78,
                    eye_height_ratio: 0.91,
                    local_offset: [0.0, 0.0, 0.0],
                    yaw_offset: 0.0,
                    hide_in_first_person: true,
                },
                tuning: PlayerTuningData {
                    body_radius: 0.45,
                    body_half_height: 0.45,
                    crouched_body_half_height: 0.15,
                    visual_radius: 0.45,
                    visual_half_height: 0.90,
                    camera_eye_height: 0.72,
                    crouched_camera_eye_height: 0.45,
                    crouch_camera_speed: 12.0,
                    sprint_multiplier: 1.75,
                    jump_speed: 5.5,
                    gravity: 9.81,
                    contact_skin: 0.035,
                    ground_probe_distance: 0.25,
                    max_slope_degrees: 50.0,
                    footstep_stride: 2.1,
                    landing_speed_threshold: 3.0,
                    locomotion_min_horizontal_speed: 0.15,
                    ground_probe_max_upward_velocity: 0.1,
                    landing_min_airborne_seconds: 0.05,
                },
            },
            gameplay: GameplayData {
                status: GameplayStatusData {
                    default_status: "Collect field cores, neutralize targets, avoid hazards, reach extraction.".to_owned(),
                    pickup_status: "Core acquired.".to_owned(),
                    target_status: "Target neutralized.".to_owned(),
                    hazard_status: "You touched a hazard. Relaunch the demo to retry.".to_owned(),
                    goal_locked_status: "Beacon locked: collect all cores first.".to_owned(),
                    goal_complete_status: "Extraction complete. Stable runtime loop is playable.".to_owned(),
                    failed_progress_label: "FAILED - touch a hazard to retry scene".to_owned(),
                    completed_progress_label: "EXTRACTED".to_owned(),
                },
                projectile: ProjectileData {
                    radius: 0.22,
                    speed: 26.0,
                    lifetime_seconds: 12.0,
                    spawn_clearance: 0.85,
                    restitution: 0.42,
                    friction: 0.36,
                    density: 1.0,
                    angular_velocity: [0.0, 2.5, 0.0],
                    color: [0.94, 0.97, 1.0, 1.0],
                },
                inventory: InventoryDefaultsData {
                    rifle_item: DEFAULT_RIFLE_ITEM_NAME.to_owned(),
                    rifle_ammo: DEFAULT_RIFLE_AMMO_NAME.to_owned(),
                    medkit_item: DEFAULT_MEDKIT_ITEM_NAME.to_owned(),
                    loadout: DEFAULT_FPS_LOADOUT_NAME.to_owned(),
                    package_asset: DEFAULT_ITEM_PACKAGE_ASSET.to_owned(),
                    hud_slots: 24,
                },
            },
        }
    }
}

static RUST_DEFAULT_GAME_DATA: OnceLock<GameData> = OnceLock::new();
static RUST_DEFAULT_GAME_DATA_SHARED: OnceLock<Arc<GameData>> = OnceLock::new();

/// Immutable process-wide Rust fallback snapshot.
///
/// Future Lua integration should replace the provider that creates the active snapshot, not the
/// systems that consume these fields.
#[inline]
pub fn default_game_data() -> &'static GameData {
    RUST_DEFAULT_GAME_DATA.get_or_init(GameData::default)
}

/// Immutable runtime snapshot installed once during scene bootstrap.
/// Gameplay systems consume this resource without invoking the source provider in hot loops.
#[derive(Clone, Debug)]
pub struct GameDataSnapshot {
    source_id: String,
    data: Arc<GameData>,
}

impl GameDataSnapshot {
    #[inline]
    pub fn new(source_id: impl Into<String>, data: GameData) -> Self {
        Self {
            source_id: source_id.into(),
            data: Arc::new(data),
        }
    }

    #[inline]
    pub fn rust_defaults() -> Self {
        let data =
            RUST_DEFAULT_GAME_DATA_SHARED.get_or_init(|| Arc::new(default_game_data().clone()));
        Self {
            source_id: "newengine.game_data.rust_defaults".to_owned(),
            data: Arc::clone(data),
        }
    }

    #[inline]
    pub fn source_id(&self) -> &str {
        &self.source_id
    }

    #[inline]
    pub fn data(&self) -> &GameData {
        self.data.as_ref()
    }

    #[inline]
    pub fn shared(&self) -> Arc<GameData> {
        Arc::clone(&self.data)
    }
}

pub trait GameDataProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn load(&self) -> Result<GameData, String>;

    #[inline]
    fn load_snapshot(&self) -> Result<GameDataSnapshot, String> {
        self.load()
            .map(|data| GameDataSnapshot::new(self.id(), data))
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RustGameDataProvider;

impl GameDataProvider for RustGameDataProvider {
    fn id(&self) -> &'static str {
        "newengine.game_data.rust_defaults"
    }

    fn load(&self) -> Result<GameData, String> {
        Ok(default_game_data().clone())
    }

    #[inline]
    fn load_snapshot(&self) -> Result<GameDataSnapshot, String> {
        Ok(GameDataSnapshot::rust_defaults())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_defaults_are_single_source_and_lua_serializable() {
        let data = default_game_data();
        assert_eq!(data.schema, GAME_DATA_SCHEMA);
        assert_eq!(data.version, GAME_DATA_VERSION);
        assert_eq!(data.runtime.fixed_dt_ms, GAME_FIXED_DT_MS);
        assert_eq!(data.player.tuning.gravity, 9.81);
        assert_eq!(data.gameplay.inventory.loadout, DEFAULT_FPS_LOADOUT_NAME);
    }

    #[test]
    fn contract_validation_rejects_non_finite_provider_data() {
        let mut data = GameData::default();
        data.player.move_speed = f32::NAN;
        assert!(data.validate().is_err());
    }

    #[test]
    fn snapshot_keeps_provider_identity_and_shared_immutable_data() {
        let snapshot = GameDataSnapshot::rust_defaults();
        assert_eq!(snapshot.source_id(), "newengine.game_data.rust_defaults");
        assert_eq!(snapshot.data().version, GAME_DATA_VERSION);
        let a = snapshot.shared();
        let b = snapshot.shared();
        assert!(Arc::ptr_eq(&a, &b));
    }
}
