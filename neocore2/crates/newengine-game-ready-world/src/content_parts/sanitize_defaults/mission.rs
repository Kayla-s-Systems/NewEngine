use super::*;

#[inline]
fn sanitized_required_id(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

#[inline]
fn positive_clamped(value: f32, min: f32, max: f32) -> Option<f32> {
    (value.is_finite() && value > 0.0).then(|| value.clamp(min, max))
}

fn required_positive_scale(value: [f32; 3]) -> Option<[f32; 3]> {
    value
        .iter()
        .all(|component| component.is_finite() && component.abs() > 1.0e-6)
        .then_some(value)
}

#[inline]
pub(in super::super) fn sanitize_prefab_spec(raw: RawPrefabSpec) -> Option<AuthoredWorldPlacementSpec> {
    let id = sanitized_required_id(&raw.id)?;

    Some(AuthoredWorldPlacementSpec {
        authored_placement_id: id.clone(),
        id,
        authored_map_ref: String::new(),
        authored_cell: None,
        authored_discrete_placement: false,
        authored_primary: true,
        source: raw.source.trim().to_owned(),
        proxy: non_empty_or(raw.proxy, default_prefab_proxy()),
        material: raw.material.trim().replace('\\', "/"),
        surface_id: raw.surface_id.trim().to_owned(),
        surface_events: raw
            .surface_events
            .into_iter()
            .filter_map(|(signal, event_id)| {
                let signal = signal.trim().to_owned();
                let event_id = event_id.trim().to_owned();
                (!signal.is_empty() && !event_id.is_empty()).then_some((signal, event_id))
            })
            .collect(),
        ballistic_material: if raw.ballistic_penetration_resistance_j_per_m.is_some()
            || raw.ballistic_entry_energy_cost_j.is_some()
            || raw.ballistic_ricochet_allowed.is_some()
        {
            Some(
                newengine_engine_runtime::gameplay::BallisticMaterialResponse {
                    penetration_resistance_j_per_m: raw
                        .ballistic_penetration_resistance_j_per_m
                        .unwrap_or(f32::INFINITY),
                    entry_energy_cost_j: raw.ballistic_entry_energy_cost_j.unwrap_or(f32::INFINITY),
                    damage_transfer_multiplier: raw
                        .ballistic_damage_transfer_multiplier
                        .unwrap_or(1.0),
                    impulse_transfer_multiplier: raw
                        .ballistic_impulse_transfer_multiplier
                        .unwrap_or(1.0),
                    ricochet_allowed: raw.ballistic_ricochet_allowed.unwrap_or(false),
                    ricochet_max_incidence_dot: raw
                        .ballistic_ricochet_max_incidence_dot
                        .unwrap_or(0.0),
                    ricochet_energy_retention: raw
                        .ballistic_ricochet_energy_retention
                        .unwrap_or(0.0),
                }
                .sanitized(),
            )
        } else {
            None
        },
        ground_placement_surface: raw.ground_placement_surface,
        enabled: raw.enabled,
        position: arr3(sanitize_array3_finite(raw.position, [0.0, 0.0, 0.0])),
        rotation_ypr: arr3(sanitize_array3_finite(raw.rotation_ypr, [0.0, 0.0, 0.0])),
        scale: arr3(sanitize_array3_positive(raw.scale, [1.0, 1.0, 1.0])),
    })
}

#[inline]
pub(in super::super) fn sanitize_mission_pickup_spec(
    raw: RawMissionPickupSpec,
) -> Option<AuthoredMissionPickupSpec> {
    let id = sanitized_required_id(&raw.id)?;
    let item = raw.item.trim().replace('\\', "/");
    Some(AuthoredMissionPickupSpec {
        id,
        item: (!item.is_empty()).then_some(item),
        quantity: raw.quantity.clamp(1, 10_000),
        auto_equip: raw.auto_equip,
        position: arr3(sanitize_array3_finite(raw.position, [0.0, 0.0, 0.0])),
        rotation_ypr: arr3(sanitize_array3_finite(raw.rotation_ypr, [0.0, 0.0, 0.0])),
        radius: positive_clamped(raw.radius, 0.15, 8.0)?,
        scale: arr3(required_positive_scale(raw.scale)?),
    })
}

fn parse_patrol_route(raw: Option<&str>) -> Option<Vec<Vec3>> {
    let Some(raw) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Some(Vec::new());
    };
    let mut points = Vec::new();
    for point in raw
        .split(';')
        .map(str::trim)
        .filter(|point| !point.is_empty())
    {
        let values = point
            .split(',')
            .map(str::trim)
            .map(str::parse::<f32>)
            .collect::<Result<Vec<_>, _>>()
            .ok()?;
        if values.len() != 3 || values.iter().any(|value| !value.is_finite()) {
            return None;
        }
        points.push(Vec3::new(values[0], values[1], values[2]));
    }
    (points.len() >= 2).then_some(points)
}

pub(in super::super) fn sanitize_mission_target_spec(
    raw: RawMissionTargetSpec,
) -> Option<AuthoredMissionTargetSpec> {
    let id = sanitized_required_id(&raw.id)?;
    let character_ref = raw
        .character_ref
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.replace('\\', "/"));
    if character_ref
        .as_deref()
        .is_some_and(|value| !value.to_ascii_lowercase().contains(".ytyp@"))
    {
        return None;
    }
    let ai = if raw.ai_enabled {
        let combat_team = raw
            .combat_team
            .filter(|value| (1..=65_535).contains(value))?;
        let sight_range = raw
            .sight_range
            .filter(|value| value.is_finite() && *value > 0.0)?
            .clamp(0.1, 10_000.0);
        let field_of_view_degrees = raw
            .field_of_view_degrees
            .filter(|value| value.is_finite() && *value > 0.0)?
            .clamp(1.0, 360.0);
        let memory_seconds = raw
            .memory_seconds
            .filter(|value| value.is_finite() && *value >= 0.0)?
            .clamp(0.0, 300.0);
        let decision_interval_seconds = raw
            .decision_interval_seconds
            .filter(|value| value.is_finite() && *value > 0.0)?
            .clamp(0.016, 10.0);
        let move_speed = raw
            .move_speed
            .filter(|value| value.is_finite() && *value >= 0.0)?
            .clamp(0.0, 30.0);
        let patrol_route = parse_patrol_route(raw.patrol_route.as_deref())?;
        let patrol_looping = raw.patrol_looping.unwrap_or(true);
        let investigate_arrival_distance = raw
            .investigate_arrival_distance
            .filter(|value| value.is_finite() && *value > 0.0)?
            .clamp(0.05, 25.0);
        let engage_standoff_distance = raw
            .engage_standoff_distance
            .filter(|value| value.is_finite() && *value > 0.0)?
            .clamp(0.05, 250.0);
        let waypoint_arrival_distance = raw
            .waypoint_arrival_distance
            .filter(|value| value.is_finite() && *value > 0.0)?
            .clamp(0.02, 10.0);
        let repath_interval_seconds = raw
            .repath_interval_seconds
            .filter(|value| value.is_finite() && *value > 0.0)?
            .clamp(0.05, 30.0);
        let view_turn_speed_degrees_per_second = raw
            .view_turn_speed_degrees_per_second
            .filter(|value| value.is_finite() && *value > 0.0)?
            .clamp(1.0, 1440.0);
        let fire_distance = raw
            .fire_distance
            .filter(|value| value.is_finite() && *value > 0.0)?
            .clamp(0.1, 1_000.0);
        let aim_tolerance_degrees = raw
            .aim_tolerance_degrees
            .filter(|value| value.is_finite() && *value > 0.0)?
            .clamp(0.01, 180.0);
        let weapon_muzzle_offset = raw
            .weapon_muzzle_offset
            .filter(|value| value.iter().all(|component| component.is_finite()))?;
        let weapon_muzzle_forward = raw
            .weapon_muzzle_forward
            .filter(|value| value.iter().all(|component| component.is_finite()))?;
        let forward_len_sq = weapon_muzzle_forward
            .iter()
            .map(|value| value * value)
            .sum::<f32>();
        if forward_len_sq <= 1.0e-8 {
            return None;
        }
        let loadout = raw
            .loadout
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())?
            .to_owned();
        Some(GameReadyEnemyAiSpec {
            combat_team,
            sight_range,
            field_of_view_degrees,
            memory_seconds,
            decision_interval_seconds,
            navigation: newengine_engine_runtime::gameplay::AINavigationTuning {
                move_speed,
                investigate_arrival_distance,
                engage_standoff_distance,
                waypoint_arrival_distance,
                repath_interval_seconds,
                view_turn_speed_radians_per_second: view_turn_speed_degrees_per_second.to_radians(),
            }
            .sanitized(),
            patrol_route,
            patrol_looping,
            combat: newengine_gameplay_fps_api::FpsAiCombatTuning {
                fire_distance,
                aim_tolerance_radians: aim_tolerance_degrees.to_radians(),
            }
            .sanitized(),
            weapon_mount: newengine_gameplay_fps_api::FpsActorWeaponMountTuning {
                local_offset: weapon_muzzle_offset,
                local_forward: weapon_muzzle_forward,
            }
            .sanitized(),
            loadout,
        })
    } else {
        None
    };
    Some(AuthoredMissionTargetSpec {
        id,
        character_ref,
        position: arr3(sanitize_array3_finite(raw.position, [0.0, 0.0, 0.0])),
        health: positive_clamped(raw.health, 1.0, 100_000.0)?,
        scale: arr3(required_positive_scale(raw.scale)?),
        ai,
    })
}

pub(in super::super) fn sanitize_mission_hazard_spec(
    raw: RawMissionHazardSpec,
) -> Option<AuthoredMissionHazardSpec> {
    let id = sanitized_required_id(&raw.id)?;
    Some(AuthoredMissionHazardSpec {
        id,
        position: arr3(sanitize_array3_finite(raw.position, [0.0, 0.0, 0.0])),
        radius: positive_clamped(raw.radius, 0.2, 32.0)?,
        scale: arr3(required_positive_scale(raw.scale)?),
    })
}

pub(in super::super) fn sanitize_mission_goal_spec(
    raw: RawMissionGoalSpec,
) -> Option<AuthoredMissionGoalSpec> {
    let id = sanitized_required_id(&raw.id)?;
    Some(AuthoredMissionGoalSpec {
        id,
        position: arr3(sanitize_array3_finite(raw.position, [0.0, 0.0, 0.0])),
        radius: positive_clamped(raw.radius, 0.2, 32.0)?,
        scale: arr3(required_positive_scale(raw.scale)?),
    })
}

#[inline]
pub(in super::super) fn sanitize_definition_instance_spec(
    raw: RawDefinitionInstanceSpec,
) -> Option<GameReadyDefinitionInstanceSpec> {
    let definition_ref = raw.definition_ref.trim().replace('\\', "/");
    if definition_ref.is_empty() {
        return None;
    }
    if !definition_ref.to_ascii_lowercase().contains(".ytyp@") {
        newengine_ulog_api::ulog::warn!(
            "authored-world definitions: rejected definition_ref='{}' reason='expected .ytyp@entry selector'",
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

fn sanitize_array3_by(
    mut value: [f32; 3],
    fallback: [f32; 3],
    valid: impl Fn(f32) -> bool,
) -> [f32; 3] {
    for (component, fallback) in value.iter_mut().zip(fallback) {
        if !valid(*component) {
            *component = fallback;
        }
    }
    value
}

#[inline]
pub(in super::super) fn sanitize_array3_finite(value: [f32; 3], fallback: [f32; 3]) -> [f32; 3] {
    sanitize_array3_by(value, fallback, f32::is_finite)
}

#[inline]
pub(in super::super) fn sanitize_array3_positive(value: [f32; 3], fallback: [f32; 3]) -> [f32; 3] {
    sanitize_array3_by(value, fallback, |component| {
        component.is_finite() && component.abs() > 1.0e-6
    })
}

#[inline]
pub(in super::super) fn arr3(v: [f32; 3]) -> Vec3 {
    Vec3::new(v[0], v[1], v[2])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_id_trims_and_rejects_empty_values() {
        assert_eq!(
            sanitized_required_id("  relay-01  ").as_deref(),
            Some("relay-01")
        );
        assert_eq!(sanitized_required_id("   "), None);
    }

    #[test]
    fn positive_values_are_required_and_only_then_clamped() {
        assert_eq!(positive_clamped(f32::NAN, 0.2, 32.0), None);
        assert_eq!(positive_clamped(-1.0, 0.2, 32.0), None);
        assert_eq!(positive_clamped(100.0, 0.2, 32.0), Some(32.0));
    }

    #[test]
    fn array_sanitizers_share_finite_component_policy() {
        assert_eq!(
            sanitize_array3_finite([1.0, f32::NAN, 3.0], [4.0, 5.0, 6.0]),
            [1.0, 5.0, 3.0]
        );
        assert_eq!(
            sanitize_array3_positive([1.0, 0.0, -2.0], [4.0, 5.0, 6.0]),
            [1.0, 5.0, -2.0]
        );
    }
}
