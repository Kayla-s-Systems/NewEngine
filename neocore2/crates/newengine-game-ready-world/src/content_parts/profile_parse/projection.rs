use super::*;

impl RawGameReadyPayload {
    pub(super) fn into_profile(self) -> GameReadyMapProfile {
        let terrain_chunk_radius = self.terrain.streaming.chunk_radius.clamp(
            0,
            newengine_scene::SceneStreamingBudget::MAX_RESIDENT_RADIUS,
        );
        let terrain_unload_radius = self.terrain.streaming.unload_radius.clamp(
            (terrain_chunk_radius + 1).max(1),
            newengine_scene::SceneStreamingBudget::MAX_UNLOAD_RADIUS,
        );

        let legacy_run_speed = self.player.move_speed.clamp(0.05, 50.0);
        let run_speed = self
            .player
            .run_speed
            .filter(|value| value.is_finite())
            .unwrap_or(legacy_run_speed)
            .clamp(0.05, 50.0);
        let walk_speed = self
            .player
            .walk_speed
            .filter(|value| value.is_finite())
            .unwrap_or(run_speed)
            .clamp(0.05, run_speed);
        let sprint_speed = self
            .player
            .sprint_speed
            .filter(|value| value.is_finite())
            .unwrap_or(run_speed * 1.75)
            .clamp(run_speed, 75.0);
        let crouch_speed = self
            .player
            .crouch_speed
            .filter(|value| value.is_finite())
            .unwrap_or(walk_speed)
            .clamp(0.05, run_speed);

        GameReadyMapProfile {
            title: self.title,
            objective: self.objective,
            authored_map_streaming: None,
            player: GameReadyPlayerSpec {
                start: arr3(self.player.start),
                yaw: self.player.yaw,
                move_speed: run_speed,
                walk_speed,
                run_speed,
                sprint_speed,
                crouch_speed,
                look_sens: self.player.look_sens,
                model: GameReadyPlayerModelSpec {
                    enabled: self.player.model.enabled
                        && !self.player.model.source.trim().is_empty(),
                    source: if self.player.model.enabled {
                        non_empty_or(self.player.model.source, default_player_model_source())
                    } else {
                        String::new()
                    },
                    properties_ref: sanitize_asset_path(self.player.model.properties_ref),
                    texture_dictionary: sanitize_texture_path(self.player.model.texture_dictionary),
                    skeleton: sanitize_asset_path(self.player.model.skeleton),
                    idle_animation: sanitize_asset_path(self.player.model.idle_animation),
                    walk_animation: sanitize_asset_path(self.player.model.walk_animation),
                    run_animation: sanitize_asset_path(self.player.model.run_animation),
                    sprint_animation: sanitize_asset_path(self.player.model.sprint_animation),
                    crouch_idle_animation: sanitize_asset_path(
                        self.player.model.crouch_idle_animation,
                    ),
                    crouch_walk_animation: sanitize_asset_path(
                        self.player.model.crouch_walk_animation,
                    ),
                    jump_animation: sanitize_asset_path(self.player.model.jump_animation),
                    fall_animation: sanitize_asset_path(self.player.model.fall_animation),
                    fall_low_animation: None,
                    fall_medium_animation: None,
                    fall_high_animation: None,
                    landing_soft_animation: None,
                    landing_medium_animation: None,
                    landing_hard_animation: None,
                    landing_hard_run_animation: None,
                    fall_medium_min_distance: 0.0,
                    fall_high_min_distance: 0.0,
                    // Character presentation is definition-owned metadata. Map projection starts empty;
                    // the selected .ytyp character definition hydrates all rig/layer/IK contracts.
                    detached_head_follow: false,
                    detached_head_follow_rule: None,
                    eye_parent_follow: false,
                    eye_parent_follow_rule: None,
                    helper_pose_copies: Vec::new(),
                    braid_secondary_motion: None,
                    equipment_ready_animation: None,
                    equipment_aim_animation: None,
                    equipment_reload_animation: None,
                    unarmed_ready_animation: None,
                    unarmed_attack_animation: None,
                    turn_45_left_animation: None,
                    turn_45_right_animation: None,
                    turn_90_left_animation: None,
                    turn_90_right_animation: None,
                    turn_135_left_animation: None,
                    turn_135_right_animation: None,
                    turn_180_left_animation: None,
                    turn_180_right_animation: None,
                    equipment_ready_sample_phase: 0.0,
                    equipment_ready_rotation_weights: Vec::new(),
                    equipment_aim_rotation_weights: Vec::new(),
                    equipment_reload_rotation_weights: Vec::new(),
                    equipment_arm_ik: false,
                    equipment_arm_ik_rig: None,
                    target_height: self.player.model.target_height.clamp(0.25, 3.0),
                    eye_height_ratio: self.player.model.eye_height_ratio.clamp(0.55, 0.98),
                    local_offset: arr3(self.player.model.local_offset),
                    yaw_offset: self.player.model.yaw_offset,
                    hide_in_first_person: self.player.model.hide_in_first_person,
                    render_options: newengine_model_domain_api::MeshRenderOptions::character_body(),
                },
            },
            terrain: GameReadyTerrainSpec {
                enabled: self.terrain.enabled,
                seed: self.terrain.seed,
                cells_x: self.terrain.cells_x.clamp(16, 80),
                cells_z: self.terrain.cells_z.clamp(16, 80),
                size_x: self.terrain.size_x.max(4.0),
                size_z: self.terrain.size_z.max(4.0),
                base_height: self.terrain.base_height,
                height_scale: self.terrain.height_scale.clamp(0.05, 1.45),
                render_options: newengine_model_domain_api::MeshRenderOptions::terrain_patch(),
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
                surface: sanitize_terrain_surface_spec(self.terrain.surface),
                heightmap: sanitize_terrain_heightmap_spec(self.terrain.heightmap),
                streaming: GameReadyTerrainStreamingSpec {
                    enabled: self.terrain.streaming.enabled,
                    chunk_radius: terrain_chunk_radius,
                    unload_radius: terrain_unload_radius,
                    max_chunks_per_frame: self.terrain.streaming.max_chunks_per_frame.clamp(
                        1,
                        newengine_scene::SceneStreamingBudget::MAX_COMMITS_PER_TICK,
                    ),
                },
            },
            sky: GameReadySkySpec {
                definition_ref: non_empty_or(self.sky.definition_ref, default_sky_definition_ref()),
                render_options: newengine_model_domain_api::MeshRenderOptions::sky_background(),
                radius: self.sky.radius.max(16.0),
                mesh: non_empty_or(self.sky.mesh, default_skydome_mesh()),
                follow_camera: self.sky.follow_camera,
                environment_profile: self.sky.environment_profile.trim().to_owned(),
                environment_region: self.sky.environment_region.trim().to_owned(),
                environment_biome: self.sky.environment_biome.trim().to_owned(),
                cloud_dictionary: non_empty_or(
                    self.sky.cloud_dictionary,
                    default_cloud_dictionary(),
                ),
                cloud_profile: non_empty_or(self.sky.cloud_profile, default_cloud_profile()),
                sun_radius: self.sky.sun_radius.clamp(1.0, 64.0),
                moon_radius: self.sky.moon_radius.clamp(1.0, 64.0),
                moon_texture: non_empty_or(self.sky.moon_texture, default_moon_texture()),
                atmosphere: sanitize_sky_atmosphere_spec(self.sky.atmosphere),
            },
            materials: GameReadyMaterialSetSpec {
                terrain: sanitize_material_spec_with_default_asset(
                    self.materials.terrain,
                    default_terrain_material(),
                ),
                sky: sanitize_material_spec_with_default_asset(
                    self.materials.sky,
                    default_sky_material(),
                ),
                sun: sanitize_material_spec_with_default_asset(
                    self.materials.sun,
                    default_sun_material(),
                ),
                moon: sanitize_material_spec_with_default_asset(
                    self.materials.moon,
                    default_moon_material(),
                ),
                tree_bark: sanitize_material_spec_with_default_asset(
                    self.materials.tree_bark,
                    default_tree_bark_material(),
                ),
                tree_leaf: sanitize_material_spec_with_default_asset(
                    self.materials.tree_leaf,
                    default_tree_leaf_material(),
                ),
                tree_branch: sanitize_material_spec_with_default_asset(
                    self.materials.tree_branch,
                    default_tree_branch_material(),
                ),
            },
            lighting: sanitize_lighting_spec(self.lighting),
            foliage: sanitize_foliage_spec(self.foliage),
            prefabs: self
                .prefabs
                .into_iter()
                .filter_map(sanitize_prefab_spec)
                .collect(),
            definitions: self
                .definitions
                .into_iter()
                .filter_map(sanitize_definition_instance_spec)
                .collect(),
            acoustic_materials: newengine_audio_api::AcousticMaterialLibrary::default(),
            gameplay: GameReadyGameplaySpec {
                default_status: non_empty_or(self.gameplay.default_status, default_status_text()),
                pickup_status: non_empty_or(self.gameplay.pickup_status, default_pickup_status()),
                target_status: non_empty_or(self.gameplay.target_status, default_target_status()),
                hazard_status: non_empty_or(self.gameplay.hazard_status, default_hazard_status()),
                goal_locked_status: non_empty_or(
                    self.gameplay.goal_locked_status,
                    default_goal_locked_status(),
                ),
                goal_complete_status: non_empty_or(
                    self.gameplay.goal_complete_status,
                    default_goal_complete_status(),
                ),
                failed_progress_label: non_empty_or(
                    self.gameplay.failed_progress_label,
                    default_failed_progress_label(),
                ),
                completed_progress_label: non_empty_or(
                    self.gameplay.completed_progress_label,
                    default_completed_progress_label(),
                ),
                player_collision: GameReadyPlayerCollisionSpec {
                    radius: self.gameplay.player_collision.radius.clamp(0.05, 5.0),
                    half_height: self.gameplay.player_collision.half_height.clamp(0.05, 8.0),
                },
                player_visual: GameReadyPlayerVisualSpec {
                    radius: self.gameplay.player_visual.radius.clamp(0.05, 8.0),
                    half_height: self.gameplay.player_visual.half_height.clamp(0.05, 12.0),
                    camera_eye_height: self
                        .gameplay
                        .player_visual
                        .camera_eye_height
                        .clamp(0.05, 12.0),
                    sprint_multiplier: self
                        .gameplay
                        .player_visual
                        .sprint_multiplier
                        .clamp(1.0, 8.0),
                },
                physics: GameReadyPhysicsSpec {
                    gravity: self.gameplay.physics.gravity.clamp(0.0, 80.0),
                    contact_skin: self.gameplay.physics.contact_skin.clamp(0.0, 0.50),
                },
                mission: GameReadyMissionSpec {
                    pickups: self
                        .gameplay
                        .mission
                        .pickups
                        .into_iter()
                        .filter_map(sanitize_mission_pickup_spec)
                        .collect(),
                    targets: self
                        .gameplay
                        .mission
                        .targets
                        .into_iter()
                        .filter_map(sanitize_mission_target_spec)
                        .collect(),
                    hazards: self
                        .gameplay
                        .mission
                        .hazards
                        .into_iter()
                        .filter_map(sanitize_mission_hazard_spec)
                        .collect(),
                    goals: self
                        .gameplay
                        .mission
                        .goals
                        .into_iter()
                        .filter_map(sanitize_mission_goal_spec)
                        .collect(),
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
