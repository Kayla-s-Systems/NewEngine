use super::*;

fn weapon_namespace(entry: &serde_json::Value) -> Option<&serde_json::Value> {
    entry
        .get("arbitrary_metadata")
        .and_then(|value| value.get("metadata"))
        .and_then(|value| value.get("newengine.weapon"))
        .or_else(|| {
            entry
                .get("metadata")
                .and_then(|value| value.get("newengine.weapon"))
        })
}

fn get<'a>(value: &'a serde_json::Value, object: &str, key: &str) -> Option<&'a serde_json::Value> {
    value.get(object)?.get(key)
}

fn string(value: &serde_json::Value, object: &str, key: &str) -> Option<String> {
    get(value, object, key)?
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn string_list(value: &serde_json::Value, object: &str, key: &str) -> Option<Vec<String>> {
    let value = get(value, object, key)?;
    let values = if let Some(values) = value.as_array() {
        values
            .iter()
            .filter_map(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>()
    } else {
        value
            .as_str()?
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>()
    };
    (!values.is_empty()).then_some(values)
}

fn string_map(
    value: &serde_json::Value,
    object: &str,
    key: &str,
) -> Option<std::collections::BTreeMap<String, String>> {
    let value = get(value, object, key)?;
    let mut out = std::collections::BTreeMap::new();
    if let Some(object) = value.as_object() {
        for (key, value) in object {
            let Some(value) = value
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            let key = key.trim().to_ascii_lowercase();
            if !key.is_empty() {
                out.insert(key, value.to_owned());
            }
        }
    } else if let Some(raw) = value.as_str() {
        for entry in raw
            .split(';')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
        {
            let Some((key, mapped)) = entry.split_once('=') else {
                continue;
            };
            let key = key.trim().to_ascii_lowercase();
            let mapped = mapped.trim();
            if !key.is_empty() && !mapped.is_empty() {
                out.insert(key, mapped.to_owned());
            }
        }
    }
    (!out.is_empty()).then_some(out)
}

fn f32_value(value: &serde_json::Value, object: &str, key: &str) -> Option<f32> {
    let value = get(value, object, key)?;
    value
        .as_f64()
        .map(|value| value as f32)
        .or_else(|| value.as_str()?.trim().parse::<f32>().ok())
        .filter(|value| value.is_finite())
}

fn u32_value(value: &serde_json::Value, object: &str, key: &str) -> Option<u32> {
    let value = get(value, object, key)?;
    value
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .or_else(|| value.as_str()?.trim().parse::<u32>().ok())
}

fn vec3(value: &serde_json::Value, object: &str, key: &str) -> Option<[f32; 3]> {
    let value = get(value, object, key)?;
    if let Some(values) = value.as_array() {
        if values.len() != 3 {
            return None;
        }
        let mut out = [0.0; 3];
        for (index, value) in values.iter().enumerate() {
            out[index] = value.as_f64()? as f32;
            if !out[index].is_finite() {
                return None;
            }
        }
        return Some(out);
    }
    let raw = value.as_str()?;
    let values = raw
        .split(',')
        .map(str::trim)
        .map(str::parse::<f32>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    (values.len() == 3 && values.iter().all(|value| value.is_finite()))
        .then(|| [values[0], values[1], values[2]])
}

fn vec4(value: &serde_json::Value, object: &str, key: &str) -> Option<[f32; 4]> {
    let value = get(value, object, key)?;
    let values = if let Some(values) = value.as_array() {
        values
            .iter()
            .map(|value| value.as_f64().map(|v| v as f32))
            .collect::<Option<Vec<_>>>()?
    } else {
        value
            .as_str()?
            .split(',')
            .map(str::trim)
            .map(str::parse::<f32>)
            .collect::<Result<Vec<_>, _>>()
            .ok()?
    };
    (values.len() == 4 && values.iter().all(|value| value.is_finite()))
        .then(|| [values[0], values[1], values[2], values[3]])
}

fn bool_value(value: &serde_json::Value, object: &str, key: &str) -> Option<bool> {
    let value = get(value, object, key)?;
    value.as_bool().or_else(|| {
        let raw = value.as_str()?.trim().to_ascii_lowercase();
        match raw.as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        }
    })
}

fn parse_component_graph(
    namespace: &serde_json::Value,
) -> Result<Option<AuthoredWeaponComponentGraphDefinition>, String> {
    let points_raw = string(namespace, "component_points", "points");
    let components_raw = string(namespace, "components", "definitions");
    let installed = string_map(namespace, "components", "installed").unwrap_or_default();
    if points_raw.is_none() && components_raw.is_none() && installed.is_empty() {
        return Ok(None);
    }
    let mut graph = AuthoredWeaponComponentGraphDefinition::default();
    if let Some(raw) = points_raw {
        for record in raw
            .split(';')
            .map(str::trim)
            .filter(|record| !record.is_empty())
        {
            let fields = record.split('|').map(str::trim).collect::<Vec<_>>();
            if fields.len() < 2 {
                return Err(format!("invalid weapon component point record '{record}'"));
            }
            graph.points.push(AuthoredWeaponComponentPointDefinition {
                id: fields[0].to_owned(),
                attach_joint: fields[1].to_owned(),
                allowed_components: fields
                    .get(2)
                    .copied()
                    .unwrap_or("")
                    .split(',')
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
                    .collect(),
            });
        }
    }
    if let Some(raw) = components_raw {
        for record in raw
            .split(';')
            .map(str::trim)
            .filter(|record| !record.is_empty())
        {
            let fields = record.split('|').map(str::trim).collect::<Vec<_>>();
            if fields.len() < 2 {
                return Err(format!("invalid weapon component record '{record}'"));
            }
            let scalar = |index: usize| -> Result<f32, String> {
                let raw = fields.get(index).copied().unwrap_or("");
                if raw.is_empty() {
                    return Ok(1.0);
                }
                raw.parse::<f32>()
                    .map_err(|_| format!("invalid component modifier '{}' in '{record}'", raw))
            };
            graph.components.push(AuthoredWeaponComponentDefinition {
                id: fields[0].to_owned(),
                slot: fields[1].to_owned(),
                model_ref: fields.get(2).copied().unwrap_or("").to_owned(),
                audio_override: fields.get(3).copied().unwrap_or("").to_owned(),
                muzzle_vfx_override: fields.get(4).copied().unwrap_or("").to_owned(),
                tracer_vfx_override: fields.get(5).copied().unwrap_or("").to_owned(),
                stat_modifiers: Vec::new(),
                modifiers: AuthoredWeaponComponentModifiers {
                    accuracy_multiplier: scalar(6)?,
                    recoil_multiplier: scalar(7)?,
                    damage_multiplier: scalar(8)?,
                    falloff_multiplier: scalar(9)?,
                    muzzle_velocity_multiplier: scalar(10)?,
                    penetration_multiplier: scalar(11)?,
                    audio_gain_multiplier: scalar(12)?,
                    presentation_offset_local: [
                        fields
                            .get(13)
                            .copied()
                            .unwrap_or("0")
                            .parse::<f32>()
                            .unwrap_or(0.0),
                        fields
                            .get(14)
                            .copied()
                            .unwrap_or("0")
                            .parse::<f32>()
                            .unwrap_or(0.0),
                        fields
                            .get(15)
                            .copied()
                            .unwrap_or("0")
                            .parse::<f32>()
                            .unwrap_or(0.0),
                    ],
                },
            });
        }
    }
    graph.default_installed = installed;
    // Validate through the runtime graph compiler now so malformed YTYP never reaches gameplay.
    let _ = graph.compile()?;
    Ok(Some(graph))
}

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
            let mut presentation = AuthoredWeaponPresentationDefinition::default();
            presentation.enabled = bool_value(namespace, "presentation", "enabled").unwrap_or(true);
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
            v4!(ready_body_to_root_rotation);
            v4!(ready_right_palm_to_weapon);
            v4!(ready_left_palm_to_weapon);
            v4!(right_palm_to_native_rig);
            v4!(native_rig_to_runtime_basis);
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
            let mut casing = AuthoredWeaponCasingDefinition::default();
            casing.model_dictionary =
                string(namespace, "casing", "model_dictionary").unwrap_or_default();
            casing.variants = string_list(namespace, "casing", "variants").unwrap_or_default();
            casing.material_ref = string(namespace, "casing", "material_ref").unwrap_or_default();
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

pub fn hydrate_item_package_from_ytyp(package: &mut AuthoredItemPackage) -> Result<usize, String> {
    let mut hydrated = 0usize;
    for authored in &mut package.items {
        let definition_ref = authored.definition_ref.trim().replace('\\', "/");
        if definition_ref.is_empty() {
            continue;
        }
        let payload = serde_json::to_vec(&serde_json::json!({
            "definition_ref": definition_ref,
        }))
        .map_err(|error| format!("weapon YTYP request encode failed: {error}"))?;
        let bytes = newengine_core::call_service_v1_optional(
            newengine_assets_api::ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
            newengine_assets_api::definitions_method::ENTRY_JSON_V1,
            &payload,
        )
        .map_err(|error| {
            format!(
                "weapon YTYP lookup failed item='{}' ref='{}': {error}",
                authored.id, definition_ref
            )
        })?
        .ok_or_else(|| {
            format!(
                "weapon YTYP definitions service unavailable item='{}' ref='{}'",
                authored.id, definition_ref
            )
        })?;
        let entry: serde_json::Value = serde_json::from_slice(&bytes).map_err(|error| {
            format!(
                "weapon YTYP entry JSON invalid item='{}' ref='{}': {error}",
                authored.id, definition_ref
            )
        })?;
        let namespace = weapon_namespace(&entry).ok_or_else(|| {
            format!(
                "weapon YTYP has no newengine.weapon metadata item='{}' ref='{}'",
                authored.id, definition_ref
            )
        })?;
        apply_weapon_ytyp_namespace(authored, namespace)?;
        hydrated += 1;
    }
    Ok(hydrated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weapon_ytyp_namespace_hydrates_fire_reload_and_world_presentation() {
        let mut item = AuthoredItemDefinition {
            id: "weapon.test".to_owned(),
            definition_ref: "definitions/weapon/test.ytyp@test".to_owned(),
            kind: "weapon".to_owned(),
            ..AuthoredItemDefinition::default()
        };
        let metadata = serde_json::json!({
            "weapon": {
                "class": "pistol",
                "ammo": "ammo.test",
                "fire_mode": "automatic",
                "magazine_capacity": 32,
                "reserve_capacity": 160,
                "fire_interval": 0.075,
                "reload_duration": 1.42,
                "damage": 19.0,
                "range": 87.0,
                "hip_spread_degrees": 2.0,
                "aim_spread_degrees": 0.4,
                "recoil_pitch_degrees": 0.9,
                "recoil_yaw_degrees": 0.3,
                "muzzle_forward_offset": 0.51
            },
            "audio": {
                "fire": "shared/audio/weapon/test/fire.wav",
                "reload_start": "shared/audio/weapon/test/reload.wav",
                "equip": "shared/audio/weapon/test/equip.wav",
                "empty": "shared/audio/weapon/test/empty.wav",
                "shell_eject": "shared/audio/weapon/test/shell.wav"
            },
            "casing": {
                "model_dictionary": "models/weapon/test_shells.ydd",
                "variants": "a,b,c",
                "material_ref": "materials/test_shell.nemat@metal",
                "half_extents": "0.01,0.02,0.03",
                "ejection_delay_seconds": 0.04,
                "origin_local": "0.1,0.2,-0.3",
                "velocity_local": "1.0,2.0,-0.2",
                "velocity_jitter": "0.2,0.1,0.0",
                "axis_local": "0.9,0.1,0.0",
                "angular_velocity": "10,20,30",
                "angular_velocity_jitter": "1,2,3",
                "friction": 0.3,
                "restitution": 0.2,
                "density": 7.5,
                "linear_damping": 0.08,
                "angular_damping": 1.5,
                "contact_min_impulse": 0.002,
                "contact_medium_impulse": 0.011,
                "contact_hard_impulse": 0.031,
                "soft_surface_contains": "dirt,sand,grass",
                "ejection_joint": "shell_eject",
                "inherit_socket_linear_velocity": 0.9,
                "inherit_socket_angular_velocity": 0.25
            },
            "world": {
                "model": "models/weapon/test.ydd@test",
                "material_library": "materials/test.nemat",
                "scale": 1.0,
                "pickup_half_extents": [0.2, 0.4, 0.1]
            },
            "presentation": {
                "enabled": true,
                "handle_from_root": [0.0, 0.014, -0.030],
                "muzzle_from_root": [0.108, 0.041, 0.640],
                "left_grip_from_handle": [-0.021, 0.043, 0.306],
                "stock_contact_from_handle": [-0.020, 0.053, -0.341],
                "ready_body_to_root_rotation": [0.036, 0.608, -0.041, 0.792],
                "right_palm_to_handle": [0.019, 0.033, -0.083],
                "first_person_hip_handle_offset": [0.20, -0.20, -0.58],
                "first_person_full_body_hip_handle_offset": [0.20, -0.20, -0.08],
                "ads_camera_translation_weight": [1.0, 0.0, 0.75],
                "aim_response_hz": 14.0,
                "secondary_hip_max_angle_radians": 0.08,
                "secondary_ads_max_angle_radians": 0.03,
                "secondary_angular_inertia_gain": 0.31,
                "secondary_movement_inertia_gain": 0.77,
                "secondary_natural_hz_hip": 4.8,
                "secondary_natural_hz_ads": 8.2,
                "secondary_obstruction_hz_boost": 5.7
            },
            "vfx": {
                "shot": "effects/weapons/test.fxd@shot",
                "impact_default": "effects/weapons/test.fxd@impact.default",
                "impact_by_surface": "surface.metal=effects/weapons/test.fxd@impact.metal;surface.wood=effects/weapons/test.fxd@impact.wood"
            }
        });
        apply_weapon_ytyp_namespace(&mut item, &metadata).expect("hydrate");
        let weapon = item.weapon.expect("weapon");
        assert_eq!(weapon.class, "pistol");
        assert_eq!(weapon.fire_mode().expect("mode"), WeaponFireMode::Automatic);
        assert_eq!(weapon.magazine_capacity, 32);
        assert!((weapon.reload_duration - 1.42).abs() < 1.0e-6);
        assert!((weapon.damage - 19.0).abs() < 1.0e-6);
        let audio = item.weapon_audio.expect("weapon audio");
        assert_eq!(audio.fire, "shared/audio/weapon/test/fire.wav");
        assert_eq!(audio.reload_start, "shared/audio/weapon/test/reload.wav");
        assert_eq!(audio.shell_eject, "shared/audio/weapon/test/shell.wav");
        let casing = item.weapon_casing.expect("weapon casing");
        assert_eq!(casing.model_dictionary, "models/weapon/test_shells.ydd");
        assert_eq!(casing.variants, vec!["a", "b", "c"]);
        assert_eq!(casing.half_extents, [0.01, 0.02, 0.03]);
        assert!((casing.ejection_delay_seconds - 0.04).abs() < 1.0e-6);
        assert!((casing.linear_damping - 0.08).abs() < 1.0e-6);
        assert!((casing.angular_damping - 1.5).abs() < 1.0e-6);
        assert_eq!(casing.axis_local, [0.9, 0.1, 0.0]);
        assert_eq!(casing.ejection_joint, "shell_eject");
        assert!((casing.contact_min_impulse - 0.002).abs() < 1.0e-6);
        assert!((casing.contact_medium_impulse - 0.011).abs() < 1.0e-6);
        assert!((casing.contact_hard_impulse - 0.031).abs() < 1.0e-6);
        assert_eq!(casing.soft_surface_contains, vec!["dirt", "sand", "grass"]);
        assert!((casing.inherit_socket_linear_velocity - 0.9).abs() < 1.0e-6);
        assert!((casing.inherit_socket_angular_velocity - 0.25).abs() < 1.0e-6);
        let vfx = item.weapon_vfx.expect("weapon vfx");
        assert_eq!(vfx.shot, "effects/weapons/test.fxd@shot");
        assert_eq!(
            vfx.impact_by_surface
                .get("surface.metal")
                .map(String::as_str),
            Some("effects/weapons/test.fxd@impact.metal")
        );
        let world = item.world.expect("world");
        assert_eq!(world.model, "models/weapon/test.ydd@test");
        assert_eq!(world.material_library, "materials/test.nemat");
        assert_eq!(world.pickup_half_extents, [0.2, 0.4, 0.1]);
        let presentation = item.weapon_presentation.expect("weapon presentation");
        assert!(presentation.enabled);
        assert_eq!(presentation.handle_from_root, [0.0, 0.014, -0.030]);
        assert_eq!(presentation.left_grip_from_handle, [-0.021, 0.043, 0.306]);
        assert_eq!(
            presentation.ready_body_to_root_rotation,
            [0.036, 0.608, -0.041, 0.792]
        );
        assert_eq!(presentation.right_palm_to_handle, [0.019, 0.033, -0.083]);
        assert_eq!(
            presentation.first_person_hip_handle_offset,
            [0.20, -0.20, -0.58]
        );
        assert_eq!(
            presentation.first_person_full_body_hip_handle_offset,
            Some([0.20, -0.20, -0.08])
        );
        let runtime_presentation = presentation.compile();
        assert_eq!(
            runtime_presentation.first_person_full_body_hip_handle_offset,
            [0.20, -0.20, -0.08]
        );
        assert_eq!(
            runtime_presentation.ads_camera_translation_weight,
            [1.0, 0.0, 0.75]
        );
        assert!((presentation.aim_response_hz - 14.0).abs() < 1.0e-6);
        assert!((presentation.secondary_angular_inertia_gain - 0.31).abs() < 1.0e-6);
        assert!((presentation.secondary_movement_inertia_gain - 0.77).abs() < 1.0e-6);
        assert!((presentation.secondary_natural_hz_ads - 8.2).abs() < 1.0e-6);
    }

    #[test]
    fn legacy_weapon_presentation_inherits_authored_hip_offset_for_full_body_fpp() {
        let authored = AuthoredWeaponPresentationDefinition {
            enabled: true,
            first_person_hip_handle_offset: [0.18, -0.21, -0.31],
            first_person_full_body_hip_handle_offset: None,
            ..AuthoredWeaponPresentationDefinition::default()
        };
        let runtime = authored.compile();
        assert_eq!(
            runtime.first_person_full_body_hip_handle_offset,
            authored.first_person_hip_handle_offset
        );
    }
    #[test]
    fn pistol_magazine_ytyp_hydrates_component_graph_and_default_install() {
        let mut item = AuthoredItemDefinition {
            id: "weapon.pistol.military".to_owned(),
            definition_ref: "shared/definitions/weapon/pistol_military.ytyp@pistol_military"
                .to_owned(),
            kind: "weapon".to_owned(),
            ..AuthoredItemDefinition::default()
        };
        let metadata = serde_json::json!({
            "weapon": {
                "type": "firearm",
                "ammo": "ammo.sidearm.standard",
                "fire_mode": "semi_auto",
                "firing_pattern_kind": "semi",
                "magazine_capacity": 15,
                "fire_interval": 0.22
            },
            "component_points": {
                "points": "magazine|magazine|pistol_magazine,pistol_magazine_upgrade;mag_mod|mag_mod|pistol_mag_extend"
            },
            "components": {
                "installed": "magazine=pistol_magazine",
                "definitions": "pistol_magazine|magazine|shared/models/weapon/pistol/pistol_magazine.ydd@pistol_magazine||||1|1|1|1|1|1|1|0|0|0;pistol_magazine_upgrade|magazine|shared/models/weapon/pistol/pistol_magazine_upgrade.ydd@pistol_magazine_upgrade||||0.96|0.96|1|1|1|1|1|0|0|0;pistol_mag_extend|mag_mod|shared/models/weapon/pistol/pistol_mag_extend.ydd@pistol_mag_extend||||1|1|1|1|1|1|1|0|0|0"
            }
        });

        apply_weapon_ytyp_namespace(&mut item, &metadata).expect("hydrate pistol magazine graph");
        let authored_graph = item.weapon_components.expect("component graph");
        let runtime_graph = authored_graph.compile().expect("compile component graph");

        assert_eq!(runtime_graph.points.len(), 2);
        assert_eq!(
            runtime_graph
                .default_installed
                .get("magazine")
                .map(String::as_str),
            Some("pistol_magazine")
        );
        let magazine = runtime_graph
            .components
            .get("pistol_magazine")
            .expect("standard magazine");
        assert_eq!(magazine.slot, "magazine");
        assert_eq!(
            magazine.model_ref.as_deref(),
            Some("shared/models/weapon/pistol/pistol_magazine.ydd@pistol_magazine")
        );
        let upgrade = runtime_graph
            .components
            .get("pistol_magazine_upgrade")
            .expect("magazine upgrade");
        assert!((upgrade.modifiers.accuracy_multiplier - 0.96).abs() < 1.0e-6);
        assert!((upgrade.modifiers.recoil_multiplier - 0.96).abs() < 1.0e-6);
        assert_eq!(
            runtime_graph
                .points
                .iter()
                .find(|point| point.id == "mag_mod")
                .expect("mag mod slot")
                .allowed_components,
            vec!["pistol_mag_extend".to_owned()]
        );
    }
}
