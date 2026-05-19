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
    if let Some(profile) = load_profile_from_asset_manager() {
        return profile;
    }

    log::warn!(
        "game-ready: using code default scene profile; AssetManager profile not found and direct plugin-content filesystem fallback is disabled"
    );
    fallback_game_ready_map_profile()
}

fn load_profile_from_asset_manager() -> Option<GameReadyMapProfile> {
    use newengine_assets::AssetService;

    if !newengine_plugin_host::has_service(newengine_assets::consts::ASSET_SERVICE_ID) {
        log::debug!(
            "game-ready: AssetManager service '{}' unavailable while resolving scene profile",
            newengine_assets::consts::ASSET_SERVICE_ID
        );
        return None;
    }

    let assets = newengine_assets::AssetServiceClient::new(newengine_plugin_host::default_host_api());
    let roots = newengine_runtime_host::asset_bootstrap::collect_app_asset_roots(
        GAME_READY_APP_DIR,
        "NEWENGINE_GAME_ASSETS_DIR",
    );
    newengine_runtime_host::asset_bootstrap::mount_asset_roots_best_effort(&assets, &roots);

    for logical_path in profile_asset_candidates() {
        match load_profile_asset(&assets, &logical_path) {
            Ok(profile) => {
                log::info!(
                    "game-ready: loaded standalone scene profile asset='{}'",
                    logical_path,
                );
                return Some(profile);
            }
            Err(e) => {
                let trace = assets
                    .resolve_trace_json_v1(&logical_path)
                    .map(|v| v.to_string())
                    .unwrap_or_else(|te| format!("{{\"trace_error\":\"{te}\"}}"));
                log::debug!(
                    "game-ready: scene profile asset unavailable path='{}' err='{}' trace={}",
                    logical_path,
                    e,
                    trace
                );
            }
        }
    }

    None
}

fn load_profile_asset(
    assets: &newengine_assets::AssetServiceClient,
    logical_path: &str,
) -> Result<GameReadyMapProfile, String> {
    let payload = assets
        .text_v1(logical_path)
        .map_err(|e| format!("asset.text_v1 failed path='{logical_path}' err='{e}'"))?;

    let value: serde_json::Value = serde_json::from_slice(&payload)
        .map_err(|e| format!("json parse failed path='{logical_path}' err='{e}'"))?;

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
        let terrain_chunk_radius = self.terrain.streaming.chunk_radius.clamp(0, newengine_scene::SceneStreamingBudget::MAX_RESIDENT_RADIUS);
        let terrain_unload_radius = self
            .terrain
            .streaming
            .unload_radius
            .clamp((terrain_chunk_radius + 1).max(1), newengine_scene::SceneStreamingBudget::MAX_UNLOAD_RADIUS);

        GameReadyMapProfile {
            title: self.title,
            objective: self.objective,
            player: GameReadyPlayerSpec {
                start: arr3(self.player.start),
                yaw: self.player.yaw,
                move_speed: self.player.move_speed,
                look_sens: self.player.look_sens,
                model: GameReadyPlayerModelSpec {
                    enabled: self.player.model.enabled && !self.player.model.source.trim().is_empty(),
                    source: non_empty_or(self.player.model.source, default_player_model_source()),
                    texture_dictionary: sanitize_texture_path(self.player.model.texture_dictionary),
                    skeleton: sanitize_asset_path(self.player.model.skeleton),
                    target_height: self.player.model.target_height.clamp(0.25, 3.0),
                    eye_height_ratio: self.player.model.eye_height_ratio.clamp(0.55, 0.98),
                    local_offset: arr3(self.player.model.local_offset),
                    yaw_offset: self.player.model.yaw_offset,
                    hide_in_first_person: self.player.model.hide_in_first_person,
                },
            },
            terrain: GameReadyTerrainSpec {
                seed: self.terrain.seed,
                cells_x: self.terrain.cells_x.clamp(16, 80),
                cells_z: self.terrain.cells_z.clamp(16, 80),
                size_x: self.terrain.size_x.max(4.0),
                size_z: self.terrain.size_z.max(4.0),
                base_height: self.terrain.base_height,
                height_scale: self.terrain.height_scale.clamp(0.05, 1.45),
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
                surface: GameReadyTerrainSurfaceSpec {
                    forest_base_texture: non_empty_or(self.terrain.surface.forest_base_texture, default_terrain_surface_forest()),
                    sand_base_texture: non_empty_or(self.terrain.surface.sand_base_texture, default_terrain_surface_sand()),
                    rock_base_texture: non_empty_or(self.terrain.surface.rock_base_texture, default_terrain_surface_rock()),
                    patch_scale: self.terrain.surface.patch_scale.clamp(0.0025, 0.25),
                    blend_softness: self.terrain.surface.blend_softness.clamp(0.01, 0.45),
                },
                streaming: GameReadyTerrainStreamingSpec {
                    enabled: self.terrain.streaming.enabled,
                    chunk_radius: terrain_chunk_radius,
                    unload_radius: terrain_unload_radius,
                    max_chunks_per_frame: self.terrain.streaming.max_chunks_per_frame.clamp(1, newengine_scene::SceneStreamingBudget::MAX_COMMITS_PER_TICK),
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
