use super::*;

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
        if self.player.character_ref.trim().is_empty() {
            return Err(
                "player.character_ref must select a Shared/player character definition".to_owned(),
            );
        }
        if !self
            .player
            .character_ref
            .to_ascii_lowercase()
            .contains(".ytyp@")
        {
            return Err(format!(
                "player.character_ref '{}' must be a selector-qualified .ytyp reference",
                self.player.character_ref
            ));
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
        if let Some(response) = tuning.motion_response {
            let response_scalars = [
                response.velocity_spring_const,
                response.velocity_spring_const_decel,
                response.velocity_spring_dampen_ratio,
                response.speed_spring_const,
                response.max_accel,
                response.trans_clamp_dist,
            ];
            if response_scalars.iter().any(|value| !value.is_finite()) {
                return Err("player.tuning.motion_response contains non-finite values".to_owned());
            }
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

impl Default for PlayerModelData {
    fn default() -> Self {
        Self {
            enabled: false,
            source: String::new(),
            target_height: 1.80,
            eye_height_ratio: 0.90,
            local_offset: [0.0, 0.0, 0.0],
            yaw_offset: 0.0,
            hide_in_first_person: false,
        }
    }
}

impl Default for PlayerTuningData {
    fn default() -> Self {
        Self {
            motion_response: None,
            body_radius: 0.35,
            body_half_height: 0.55,
            crouched_body_half_height: 0.30,
            visual_radius: 0.35,
            visual_half_height: 0.90,
            camera_eye_height: 0.62,
            crouched_camera_eye_height: 0.35,
            crouch_camera_speed: 8.0,
            sprint_multiplier: 1.5,
            jump_speed: 5.0,
            gravity: 9.81,
            contact_skin: 0.03,
            ground_probe_distance: 0.12,
            max_slope_degrees: 45.0,
            footstep_stride: 1.4,
            landing_speed_threshold: 4.0,
            locomotion_min_horizontal_speed: 0.10,
            ground_probe_max_upward_velocity: 0.10,
            landing_min_airborne_seconds: 0.05,
        }
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
                    definition_ref: "shared/definitions/environment/default_sky.ytyp@default_sky".to_owned(),
                    radius: 220.0,
                    mesh: "models/environment/skydome.ydd@skydome_high".to_owned(),
                    follow_camera: true,
                    environment_profile: "environment.game_ready_forest_road".to_owned(),
                    environment_region: "game_ready.forest_road".to_owned(),
                    environment_biome: "temperate_forest".to_owned(),
                    cloud_dictionary: "textures/environment/sky_clouds_v2.ytd".to_owned(),
                    cloud_profile: "temperate_cumulus_dynamic".to_owned(),
                    sun_radius: 18.0,
                    moon_radius: 13.5,
                    moon_texture: "textures/environment/skydome.ytd@moon_new".to_owned(),
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
                    ambient_intensity: 0.30,
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
                    contact_strength: 0.72,
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
                character_ref: DEFAULT_PLAYER_CHARACTER_REF.to_owned(),
                model: PlayerModelData::default(),
                tuning: PlayerTuningData::default(),
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
