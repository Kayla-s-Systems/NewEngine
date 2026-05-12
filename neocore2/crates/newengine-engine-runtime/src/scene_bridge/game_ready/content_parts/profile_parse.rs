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


#[derive(Debug, Deserialize)]
struct RawFoliageSpec {
    #[serde(default)]
    enabled: bool,
    #[serde(default = "default_foliage_prefab")]
    prefab: String,
    #[serde(default = "default_foliage_seed")]
    seed: u64,
    #[serde(default = "default_foliage_grid_min")]
    grid_min: i32,
    #[serde(default = "default_foliage_grid_max")]
    grid_max: i32,
    #[serde(default = "default_foliage_spacing")]
    spacing: f32,
    #[serde(default = "default_foliage_jitter")]
    jitter: f32,
    #[serde(default = "default_foliage_gate_threshold")]
    gate_threshold: f32,
    #[serde(default)]
    max_count: u32,
    #[serde(default = "default_foliage_min_scale")]
    min_scale: f32,
    #[serde(default = "default_foliage_max_scale")]
    max_scale: f32,
    #[serde(default = "default_foliage_min_player_distance")]
    min_player_distance: f32,
    #[serde(default = "default_foliage_edge_margin")]
    edge_margin: f32,
    #[serde(default = "default_foliage_surface_offset")]
    surface_offset: f32,
}

impl Default for RawFoliageSpec {
    fn default() -> Self {
        Self {
            enabled: false,
            prefab: default_foliage_prefab(),
            seed: default_foliage_seed(),
            grid_min: default_foliage_grid_min(),
            grid_max: default_foliage_grid_max(),
            spacing: default_foliage_spacing(),
            jitter: default_foliage_jitter(),
            gate_threshold: default_foliage_gate_threshold(),
            max_count: 0,
            min_scale: default_foliage_min_scale(),
            max_scale: default_foliage_max_scale(),
            min_player_distance: default_foliage_min_player_distance(),
            edge_margin: default_foliage_edge_margin(),
            surface_offset: default_foliage_surface_offset(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawPrefabSpec {
    #[serde(default)]
    id: String,
    #[serde(default)]
    source: String,
    #[serde(default = "default_prefab_proxy")]
    proxy: String,
    #[serde(default = "default_prefab_enabled")]
    enabled: bool,
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
                    smoothing_passes: self.terrain.generator.smoothing_passes.min(16),
                    smoothing_strength: self.terrain.generator.smoothing_strength.clamp(0.0, 1.0),
                },
            },
            sky: GameReadySkySpec {
                radius: self.sky.radius.max(16.0),
                mesh: non_empty_or(self.sky.mesh, default_skydome_mesh()),
            },
            materials: GameReadyMaterialSetSpec {
                terrain: sanitize_material_spec(self.materials.terrain),
                sky: sanitize_material_spec(self.materials.sky),
                tree_bark: sanitize_material_spec(self.materials.tree_bark),
                tree_leaf: sanitize_material_spec(self.materials.tree_leaf),
                tree_branch: sanitize_material_spec(self.materials.tree_branch),
            },
            lighting: sanitize_lighting_spec(self.lighting),
            foliage: sanitize_foliage_spec(self.foliage),
            prefabs: self.prefabs.into_iter().filter_map(sanitize_prefab_spec).collect(),
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
                tree_bark: self.palette.tree_bark,
                tree_leaf: self.palette.tree_leaf,
                tree_branch: self.palette.tree_branch,
            },
        }
    }
}
