use super::*;

impl Default for RawPlayerSpec {
    fn default() -> Self {
        Self {
            start: default_player_start(),
            yaw: default_player_yaw(),
            move_speed: default_move_speed(),
            walk_speed: None,
            run_speed: None,
            sprint_speed: None,
            crouch_speed: None,
            combat_team: None,
            health_maximum: None,
            stamina_maximum: None,
            stamina_sprint_drain_per_second: None,
            stamina_regen_per_second: None,
            stamina_regen_delay_seconds: None,
            stamina_exhausted_resume_fraction: None,
            damage_stagger_damage_fraction: None,
            damage_stagger_impulse_threshold: None,
            damage_flinch_duration_seconds: None,
            damage_stagger_duration_seconds: None,
            injured_health_fraction: None,
            drop_active_weapon_on_death: None,
            death_presentation: None,
            look_sens: default_look_sens(),
            model: RawPlayerModelSpec::default(),
        }
    }
}

impl Default for RawPlayerModelSpec {
    fn default() -> Self {
        Self {
            enabled: default_player_model_enabled(),
            source: String::new(),
            properties_ref: default_player_model_properties_ref(),
            texture_dictionary: default_player_texture_dictionary(),
            skeleton: default_player_skeleton(),
            animation_slots: std::collections::BTreeMap::new(),
            idle_animation: None,
            walk_animation: None,
            run_animation: None,
            sprint_animation: None,
            crouch_idle_animation: None,
            crouch_walk_animation: None,
            jump_animation: None,
            fall_animation: None,
            target_height: default_player_model_height(),
            eye_height_ratio: default_player_model_eye_height_ratio(),
            local_offset: default_player_model_offset(),
            yaw_offset: default_player_model_yaw_offset(),
            hide_in_first_person: default_player_model_hide_in_first_person(),
        }
    }
}

impl Default for RawTerrainSurfaceSpec {
    fn default() -> Self {
        Self {
            forest_base_texture: default_terrain_surface_forest(),
            sand_base_texture: default_terrain_surface_sand(),
            rock_base_texture: default_terrain_surface_rock(),
            patch_scale: default_terrain_patch_scale(),
            blend_softness: default_terrain_blend_softness(),
            layers: Vec::new(),
        }
    }
}

impl Default for RawTerrainHeightmapSpec {
    fn default() -> Self {
        Self {
            enabled: false,
            source: String::new(),
            mode: default_terrain_heightmap_mode(),
            strength: default_terrain_heightmap_strength(),
            min_height: default_terrain_heightmap_min_height(),
            max_height: default_terrain_heightmap_max_height(),
            tile_scale: default_terrain_heightmap_tile_scale(),
            tile_offset: default_terrain_heightmap_tile_offset(),
            invert: false,
        }
    }
}

impl Default for RawTerrainStreamingSpec {
    fn default() -> Self {
        Self {
            enabled: default_terrain_streaming_enabled(),
            chunk_radius: default_terrain_chunk_radius(),
            unload_radius: default_terrain_unload_radius(),
            max_chunks_per_frame: default_terrain_max_chunks_per_frame(),
        }
    }
}

impl Default for RawTerrainSpec {
    fn default() -> Self {
        Self {
            enabled: default_terrain_enabled(),
            seed: default_terrain_seed(),
            cells_x: default_terrain_cells(),
            cells_z: default_terrain_cells(),
            size_x: default_terrain_size(),
            size_z: default_terrain_size(),
            base_height: default_base_height(),
            height_scale: default_height_scale(),
            generator: RawTerrainGeneratorSpec::default(),
            surface: RawTerrainSurfaceSpec::default(),
            heightmap: RawTerrainHeightmapSpec::default(),
            streaming: RawTerrainStreamingSpec::default(),
        }
    }
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
            smoothing_passes: default_smoothing_passes(),
            smoothing_strength: default_smoothing_strength(),
        }
    }
}

impl Default for RawSkySpec {
    fn default() -> Self {
        Self {
            definition_ref: default_sky_definition_ref(),
            radius: default_sky_radius(),
            mesh: default_skydome_mesh(),
            follow_camera: default_sky_follow_camera(),
            environment_profile: String::new(),
            environment_region: String::new(),
            environment_biome: String::new(),
            cloud_dictionary: default_cloud_dictionary(),
            cloud_profile: default_cloud_profile(),
            sun_radius: default_sky_sun_radius(),
            moon_radius: default_sky_moon_radius(),
            moon_texture: default_moon_texture(),
            atmosphere: RawSkyAtmosphereSpec::default(),
        }
    }
}

impl Default for RawSkyAtmosphereSpec {
    fn default() -> Self {
        Self {
            day_zenith: default_sky_day_zenith(),
            day_horizon: default_sky_day_horizon(),
            dusk_zenith: default_sky_dusk_zenith(),
            dusk_horizon: default_sky_dusk_horizon(),
            night_zenith: default_sky_night_zenith(),
            night_horizon: default_sky_night_horizon(),
            cloud_day: default_sky_cloud_day(),
            cloud_night: default_sky_cloud_night(),
            night_sky_strength: default_sky_night_strength(),
            cloud_coverage: default_sky_cloud_coverage(),
            cloud_softness: default_sky_cloud_softness(),
        }
    }
}

impl Default for RawGameplaySpec {
    fn default() -> Self {
        Self {
            default_status: default_status_text(),
            pickup_status: default_pickup_status(),
            target_status: default_target_status(),
            hazard_status: default_hazard_status(),
            goal_locked_status: default_goal_locked_status(),
            goal_complete_status: default_goal_complete_status(),
            failed_progress_label: default_failed_progress_label(),
            completed_progress_label: default_completed_progress_label(),
            player_collision: RawPlayerCollisionSpec::default(),
            player_visual: RawPlayerVisualSpec::default(),
            camera: RawCameraSpec::default(),
            physics: RawPhysicsSpec::default(),
            mission: RawMissionSpec::default(),
        }
    }
}

impl Default for RawPlayerCollisionSpec {
    fn default() -> Self {
        Self {
            radius: default_player_body_radius(),
            half_height: default_player_body_half_height(),
        }
    }
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

impl Default for RawCameraSpec {
    fn default() -> Self {
        Self {
            first_person_fov_y_degrees: default_camera_first_person_fov_y_degrees(),
            first_person_ads_fov_y_degrees: default_camera_first_person_ads_fov_y_degrees(),
            first_person_near: default_camera_first_person_near(),
            first_person_forward_clearance: default_camera_first_person_forward_clearance(),
            first_person_body_yaw_limit_degrees: default_camera_first_person_body_yaw_limit_degrees(
            ),
            first_person_down_pitch_limit_degrees:
                default_camera_first_person_down_pitch_limit_degrees(),
            third_person_follow_fov_y_degrees: default_camera_third_person_follow_fov_y_degrees(),
            third_person_aim_fov_y_degrees: default_camera_third_person_aim_fov_y_degrees(),
            third_person_orbit_fov_y_degrees: default_camera_third_person_orbit_fov_y_degrees(),
            hide_local_model_in_first_person: false,
        }
    }
}

impl Default for RawPhysicsSpec {
    fn default() -> Self {
        Self {
            gravity: default_gravity(),
            contact_skin: default_contact_skin(),
        }
    }
}

impl Default for RawPaletteSpec {
    fn default() -> Self {
        Self {
            terrain: default_terrain_color(),
            sky: default_sky_color(),
            sky_emissive: default_sky_emissive(),
            tree_bark: default_tree_bark_color(),
            tree_leaf: default_tree_leaf_color(),
            tree_branch: default_tree_branch_color(),
        }
    }
}

impl Default for RawMaterialSetSpec {
    fn default() -> Self {
        Self {
            terrain: default_terrain_material(),
            sky: default_sky_material(),
            sun: default_sun_material(),
            moon: default_moon_material(),
            tree_bark: default_tree_bark_material(),
            tree_leaf: default_tree_leaf_material(),
            tree_branch: default_tree_branch_material(),
        }
    }
}

impl Default for RawMaterialSpec {
    fn default() -> Self {
        Self {
            asset: None,
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

impl Default for RawLightingSpec {
    fn default() -> Self {
        Self {
            ambient_color: default_ambient_color(),
            ambient_intensity: default_ambient_intensity(),
            sun_direction: default_sun_direction(),
            sun_color: default_sun_color(),
            sun_intensity: default_sun_intensity(),
            shadows: RawShadowSpec::default(),
            day_night: RawDayNightSpec::default(),
        }
    }
}

impl Default for RawDayNightSpec {
    fn default() -> Self {
        Self {
            enabled: default_day_night_enabled(),
            time_of_day_hours: default_time_of_day_hours(),
            day_length_seconds: default_day_length_seconds(),
            day_of_year: default_day_of_year(),
            latitude_degrees: default_sun_latitude_degrees(),
            axial_tilt_degrees: default_axial_tilt_degrees(),
        }
    }
}
