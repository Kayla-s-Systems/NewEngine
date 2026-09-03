pub(crate) fn apply_weapon_ytyp_namespace(
    authored: &mut AuthoredItemDefinition,
    namespace: &serde_json::Value,
) -> Result<(), String> {
    if authored.kind.trim().eq_ignore_ascii_case("weapon") {
        let mut weapon = authored.weapon.take().unwrap_or_default();
        weapon.weapon_type =
            string(namespace, "weapon", "type").unwrap_or_else(|| weapon.weapon_type.clone());
        weapon.class = string(namespace, "weapon", "class").unwrap_or_else(|| weapon.class.clone());
        if let Some(value) = u32_value(namespace, "weapon", "rank") {
            weapon.rank = Some(value.min(u16::MAX as u32) as u16);
        }
        let weapon_type = weapon
            .weapon_type()
            .map_err(|error| format!("weapon YTYP '{}': {error}", authored.definition_ref))?;
        weapon.ammo = string(namespace, "weapon", "ammo").unwrap_or_default();
        if weapon_type == WeaponType::Firearm && weapon.ammo.trim().is_empty() {
            return Err(format!(
                "firearm YTYP '{}' has no ammo",
                authored.definition_ref
            ));
        }
        weapon.fire_mode =
            string(namespace, "weapon", "fire_mode").unwrap_or_else(|| "semi_auto".to_owned());
        weapon.firing_pattern_kind =
            string(namespace, "weapon", "firing_pattern_kind").unwrap_or_default();
        if let Some(value) = u32_value(namespace, "weapon", "bursts_min") {
            weapon.bursts_min = value.min(u8::MAX as u32) as u8;
        }
        if let Some(value) = u32_value(namespace, "weapon", "bursts_max") {
            weapon.bursts_max = value.min(u8::MAX as u32) as u8;
        }
        if let Some(value) = u32_value(namespace, "weapon", "shots_per_burst_min") {
            weapon.shots_per_burst_min = value.min(u8::MAX as u32) as u8;
        }
        if let Some(value) = u32_value(namespace, "weapon", "shots_per_burst_max") {
            weapon.shots_per_burst_max = value.min(u8::MAX as u32) as u8;
        }
        if let Some(value) = f32_value(namespace, "weapon", "time_between_shots") {
            weapon.time_between_shots = value;
        }
        if let Some(value) = f32_value(namespace, "weapon", "time_between_bursts") {
            weapon.time_between_bursts = value;
        }
        if let Some(value) = f32_value(namespace, "weapon", "delay_before_firing") {
            weapon.delay_before_firing = value;
        }
        if let Some(value) = f32_value(namespace, "weapon", "burst_cooldown") {
            weapon.burst_cooldown = value;
        }
        if let Some(value) = u32_value(namespace, "weapon", "magazine_capacity") {
            weapon.magazine_capacity = value;
        }
        if let Some(value) = u32_value(namespace, "weapon", "reserve_capacity") {
            weapon.reserve_capacity = value;
        }
        if let Some(value) = f32_value(namespace, "weapon", "fire_interval") {
            weapon.fire_interval = value;
        }
        if let Some(value) = f32_value(namespace, "weapon", "reload_duration") {
            weapon.reload_duration = value;
        }
        if let Some(value) = string(namespace, "weapon", "reload_topology") {
            weapon.reload_topology = value;
        }
        if let Some(value) = f32_value(namespace, "weapon", "damage") {
            weapon.damage = value;
        }
        if let Some(value) = f32_value(namespace, "weapon", "range") {
            weapon.range = value;
        }
        if let Some(value) = f32_value(namespace, "weapon", "melee_damage") {
            weapon.melee_damage = value;
        }
        if let Some(value) = f32_value(namespace, "weapon", "melee_range") {
            weapon.melee_range = value;
        }
        if let Some(value) = f32_value(namespace, "weapon", "melee_attack_interval") {
            weapon.melee_attack_interval = value;
        }
        if let Some(value) = f32_value(namespace, "weapon", "hip_spread_degrees") {
            weapon.hip_spread_degrees = value;
        }
        if let Some(value) = f32_value(namespace, "weapon", "aim_spread_degrees") {
            weapon.aim_spread_degrees = value;
        }
        if let Some(value) = f32_value(namespace, "weapon", "movement_spread_multiplier") {
            weapon.movement_spread_multiplier = value;
        }
        if let Some(value) = f32_value(namespace, "weapon", "crouch_spread_multiplier") {
            weapon.crouch_spread_multiplier = value;
        }
        if let Some(value) = f32_value(namespace, "weapon", "recoil_accuracy_per_shot_degrees") {
            weapon.recoil_accuracy_per_shot_degrees = value;
        }
        if let Some(value) = f32_value(namespace, "weapon", "recoil_accuracy_max_degrees") {
            weapon.recoil_accuracy_max_degrees = value;
        }
        if let Some(value) = f32_value(namespace, "weapon", "accuracy_recovery_hz") {
            weapon.accuracy_recovery_hz = value;
        }
        if let Some(value) = f32_value(namespace, "weapon", "accuracy_recovery_delay_seconds") {
            weapon.accuracy_recovery_delay_seconds = value;
        }
        if let Some(value) = f32_value(namespace, "weapon", "recoil_pitch_degrees") {
            weapon.recoil_pitch_degrees = value;
        }
        if let Some(value) = f32_value(namespace, "weapon", "recoil_yaw_degrees") {
            weapon.recoil_yaw_degrees = value;
        }
        if let Some(value) = f32_value(namespace, "weapon", "recoil_pitch_random_degrees") {
            weapon.recoil_pitch_random_degrees = value;
        }
        if let Some(value) = f32_value(namespace, "weapon", "recoil_yaw_bias_degrees") {
            weapon.recoil_yaw_bias_degrees = value;
        }
        if let Some(value) = f32_value(namespace, "weapon", "ads_recoil_multiplier") {
            weapon.ads_recoil_multiplier = value;
        }
        if let Some(value) = f32_value(namespace, "weapon", "recoil_recovery_hz") {
            weapon.recoil_recovery_hz = value;
        }
        if let Some(value) = f32_value(namespace, "weapon", "recoil_pitch_tracker_speed_scale") {
            weapon.recoil_pitch_tracker_speed_scale = value;
        }
        if let Some(value) = f32_value(namespace, "weapon", "recoil_yaw_tracker_speed_scale") {
            weapon.recoil_yaw_tracker_speed_scale = value;
        }
        if let Some(value) = bool_value(namespace, "weapon", "ricochet_enabled") {
            weapon.ricochet_enabled = value;
        }
        if let Some(value) = u32_value(namespace, "weapon", "ricochet_max_bounces") {
            weapon.ricochet_max_bounces = value.min(4) as u8;
        }
        if let Some(value) = f32_value(namespace, "weapon", "ricochet_grazing_dot") {
            weapon.ricochet_grazing_dot = value;
        }
        if let Some(value) = f32_value(namespace, "weapon", "ricochet_energy_retention") {
            weapon.ricochet_energy_retention = value;
        }
        if let Some(value) = f32_value(namespace, "weapon", "muzzle_forward_offset") {
            weapon.muzzle_forward_offset = value;
        }
        if weapon_type == WeaponType::Firearm {
            weapon
                .fire_mode()
                .map_err(|error| format!("weapon YTYP '{}': {error}", authored.definition_ref))?;
        }
        authored.weapon = Some(weapon);
        authored.weapon_components = parse_component_graph(namespace)?;

        let audio = AuthoredWeaponAudioDefinition {
            fire: string(namespace, "audio", "fire").unwrap_or_default(),
            reload_start: string(namespace, "audio", "reload_start").unwrap_or_default(),
            reload_complete: string(namespace, "audio", "reload_complete").unwrap_or_default(),
            equip: string(namespace, "audio", "equip").unwrap_or_default(),
            unequip: string(namespace, "audio", "unequip").unwrap_or_default(),
            empty: string(namespace, "audio", "empty").unwrap_or_default(),
            shell_eject: string(namespace, "audio", "shell_eject").unwrap_or_default(),
            shell_contact_small: string(namespace, "audio", "shell_contact_small")
                .unwrap_or_default(),
            shell_contact_medium: string(namespace, "audio", "shell_contact_medium")
                .unwrap_or_default(),
            shell_contact_hard: string(namespace, "audio", "shell_contact_hard")
                .unwrap_or_default(),
            shell_contact_soft: string(namespace, "audio", "shell_contact_soft")
                .unwrap_or_default(),
        };
        let has_audio = !audio.fire.trim().is_empty()
            || !audio.reload_start.trim().is_empty()
            || !audio.reload_complete.trim().is_empty()
            || !audio.equip.trim().is_empty()
            || !audio.unequip.trim().is_empty()
            || !audio.empty.trim().is_empty()
            || !audio.shell_eject.trim().is_empty()
            || !audio.shell_contact_small.trim().is_empty()
            || !audio.shell_contact_medium.trim().is_empty()
            || !audio.shell_contact_hard.trim().is_empty()
            || !audio.shell_contact_soft.trim().is_empty();
        if has_audio {
            authored.weapon_audio = Some(audio);
        }

        if namespace.get("vfx").is_some() {
            let vfx = AuthoredWeaponVfxDefinition {
                shot: string(namespace, "vfx", "shot").unwrap_or_default(),
                tracer: string(namespace, "vfx", "tracer").unwrap_or_default(),
                ricochet: string(namespace, "vfx", "ricochet").unwrap_or_default(),
                exit: string(namespace, "vfx", "exit").unwrap_or_default(),
                impact_default: string(namespace, "vfx", "impact_default").unwrap_or_default(),
                impact_by_surface: string_map(namespace, "vfx", "impact_by_surface")
                    .unwrap_or_default(),
            };
            if !vfx.shot.trim().is_empty()
                || !vfx.tracer.trim().is_empty()
                || !vfx.ricochet.trim().is_empty()
                || !vfx.exit.trim().is_empty()
                || !vfx.impact_default.trim().is_empty()
                || !vfx.impact_by_surface.is_empty()
            {
                authored.weapon_vfx = Some(vfx);
            }
        }

        if namespace.get("presentation").is_some() {
            let mut presentation = AuthoredWeaponPresentationDefinition {
                enabled: bool_value(namespace, "presentation", "enabled").unwrap_or(true),
                ..Default::default()
            };
            macro_rules! v3 {
                ($field:ident) => {
                    if let Some(value) = vec3(namespace, "presentation", stringify!($field)) {
                        presentation.$field = value;
                    }
                };
            }
            macro_rules! v4 {
                ($field:ident) => {
                    if let Some(value) = vec4(namespace, "presentation", stringify!($field)) {
                        presentation.$field = value;
                    }
                };
            }
            v3!(handle_from_root);
            v3!(muzzle_from_root);
            v3!(left_grip_from_handle);
            v3!(stock_contact_from_handle);
            v3!(ready_shoulder_pocket_offset);
            v3!(ads_shoulder_pocket_offset);
            v3!(ready_right_elbow_pole_offset);
            v3!(ready_left_elbow_pole_offset);
            v3!(ready_left_palm_to_left_grip);
            v3!(right_palm_to_handle);
            v3!(first_person_hip_handle_offset);
            if let Some(value) = vec3(
                namespace,
                "presentation",
                "first_person_full_body_hip_handle_offset",
            ) {
                presentation.first_person_full_body_hip_handle_offset = Some(value);
            }
            v3!(ads_rear_sight_from_handle);
            v3!(ads_front_sight_from_handle);
            v3!(ads_camera_to_rear_sight);
            v3!(ads_camera_translation_weight);
            v4!(handle_rotation_from_root);
            v4!(ready_body_to_root_rotation);
            v4!(ready_right_palm_to_weapon);
            v4!(ready_left_palm_to_weapon);
            v4!(right_palm_to_native_rig);
            v4!(native_rig_to_runtime_basis);
            v4!(authored_socket_to_weapon_handle_basis);
            if let Some(value) = f32_value(namespace, "presentation", "fire_kick_duration_seconds")
            {
                presentation.fire_kick_duration_seconds = value;
            }
            if let Some(value) = f32_value(namespace, "presentation", "fire_kick_pitch_radians") {
                presentation.fire_kick_pitch_radians = value;
            }
            if let Some(value) =
                f32_value(namespace, "presentation", "first_person_hip_convergence_m")
            {
                presentation.first_person_hip_convergence_m = value;
            }
            macro_rules! presentation_scalar {
                ($field:ident) => {
                    if let Some(value) = f32_value(namespace, "presentation", stringify!($field)) {
                        presentation.$field = value;
                    }
                };
            }
            presentation_scalar!(aim_response_hz);
            presentation_scalar!(secondary_hip_max_angle_radians);
            presentation_scalar!(secondary_ads_max_angle_radians);
            presentation_scalar!(secondary_angular_inertia_gain);
            presentation_scalar!(secondary_movement_inertia_gain);
            presentation_scalar!(secondary_natural_hz_hip);
            presentation_scalar!(secondary_natural_hz_ads);
            presentation_scalar!(secondary_obstruction_hz_boost);
            authored.weapon_presentation = Some(presentation);
        }

        if namespace.get("casing").is_some() {
            let mut casing = AuthoredWeaponCasingDefinition {
                model_dictionary: string(namespace, "casing", "model_dictionary")
                    .unwrap_or_default(),
                variants: string_list(namespace, "casing", "variants").unwrap_or_default(),
                material_ref: string(namespace, "casing", "material_ref").unwrap_or_default(),
                ..Default::default()
            };
            if let Some(value) = vec3(namespace, "casing", "half_extents") {
                casing.half_extents = value;
            }
            if let Some(value) = f32_value(namespace, "casing", "ejection_delay_seconds") {
                casing.ejection_delay_seconds = value;
            }
            casing.ejection_joint =
                string(namespace, "casing", "ejection_joint").unwrap_or_default();
            if let Some(value) = f32_value(namespace, "casing", "inherit_socket_linear_velocity") {
                casing.inherit_socket_linear_velocity = value;
            }
            if let Some(value) = f32_value(namespace, "casing", "inherit_socket_angular_velocity") {
                casing.inherit_socket_angular_velocity = value;
            }
            if let Some(value) = vec3(namespace, "casing", "origin_local") {
                casing.origin_local = value;
            }
            if let Some(value) = vec3(namespace, "casing", "velocity_local") {
                casing.velocity_local = value;
            }
            if let Some(value) = vec3(namespace, "casing", "velocity_jitter") {
                casing.velocity_jitter = value;
            }
            if let Some(value) = vec3(namespace, "casing", "axis_local") {
                casing.axis_local = value;
            }
            if let Some(value) = vec3(namespace, "casing", "angular_velocity") {
                casing.angular_velocity = value;
            }
            if let Some(value) = vec3(namespace, "casing", "angular_velocity_jitter") {
                casing.angular_velocity_jitter = value;
            }
            if let Some(value) = f32_value(namespace, "casing", "friction") {
                casing.friction = value;
            }
            if let Some(value) = f32_value(namespace, "casing", "restitution") {
                casing.restitution = value;
            }
            if let Some(value) = f32_value(namespace, "casing", "density") {
                casing.density = value;
            }
            if let Some(value) = f32_value(namespace, "casing", "linear_damping") {
                casing.linear_damping = value;
            }
            if let Some(value) = f32_value(namespace, "casing", "angular_damping") {
                casing.angular_damping = value;
            }
            if let Some(value) = f32_value(namespace, "casing", "contact_min_impulse") {
                casing.contact_min_impulse = value;
            }
            if let Some(value) = f32_value(namespace, "casing", "contact_medium_impulse") {
                casing.contact_medium_impulse = value;
            }
            if let Some(value) = f32_value(namespace, "casing", "contact_hard_impulse") {
                casing.contact_hard_impulse = value;
            }
            if let Some(value) = string_list(namespace, "casing", "soft_surface_contains") {
                casing.soft_surface_contains = value;
            }
            authored.weapon_casing = Some(casing);
        }

        let animation = AuthoredWeaponAnimationDefinition {
            skeleton: string(namespace, "world", "skeleton").unwrap_or_default(),
            animation_dictionary: string(namespace, "world", "animation_dictionary")
                .unwrap_or_default(),
            idle: string(namespace, "animations", "idle").unwrap_or_default(),
            fire: string(namespace, "animations", "fire").unwrap_or_default(),
            reload: string(namespace, "animations", "reload").unwrap_or_default(),
            spawn_pose: string(namespace, "animations", "spawn_pose").unwrap_or_default(),
        };
        let has_animation = !animation.skeleton.trim().is_empty()
            || !animation.animation_dictionary.trim().is_empty()
            || !animation.idle.trim().is_empty()
            || !animation.fire.trim().is_empty()
            || !animation.reload.trim().is_empty()
            || !animation.spawn_pose.trim().is_empty();
        if has_animation {
            authored.weapon_animation = Some(animation);
        }
    }

    let mut world = authored.world.take().unwrap_or_default();
    if let Some(value) = string(namespace, "world", "model") {
        world.model = value;
    }
    if let Some(value) = string(namespace, "world", "material_library") {
        world.material_library = value;
    }
    if let Some(value) = f32_value(namespace, "world", "scale") {
        world.scale = [value; 3];
    }
    if let Some(value) = vec3(namespace, "world", "pickup_half_extents") {
        world.pickup_half_extents = value;
    }
    authored.world = Some(world);
    Ok(())
}
