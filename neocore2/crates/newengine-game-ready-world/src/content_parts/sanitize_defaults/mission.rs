use super::*;
use newengine_game_data::default_game_data;

#[inline]
fn sanitized_required_id(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

#[inline]
fn positive_clamped_or(value: f32, min: f32, max: f32, fallback: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value.clamp(min, max)
    } else {
        fallback
    }
}

#[inline]
pub(in super::super) fn sanitize_prefab_spec(raw: RawPrefabSpec) -> Option<GameReadyPrefabSpec> {
    let id = sanitized_required_id(&raw.id)?;

    Some(GameReadyPrefabSpec {
        authored_placement_id: id.clone(),
        id,
        authored_map_ref: String::new(),
        authored_discrete_placement: false,
        authored_primary: true,
        source: raw.source.trim().to_owned(),
        proxy: non_empty_or(raw.proxy, default_prefab_proxy()),
        material: raw.material.trim().replace('\\', "/"),
        enabled: raw.enabled,
        position: arr3(sanitize_array3_finite(raw.position, [0.0, 0.0, 0.0])),
        rotation_ypr: arr3(sanitize_array3_finite(raw.rotation_ypr, [0.0, 0.0, 0.0])),
        scale: arr3(sanitize_array3_positive(raw.scale, [1.0, 1.0, 1.0])),
    })
}

#[inline]
pub(in super::super) fn sanitize_mission_pickup_spec(
    raw: RawMissionPickupSpec,
) -> Option<GameReadyMissionPickupSpec> {
    let id = sanitized_required_id(&raw.id)?;
    let item = raw.item.trim().replace('\\', "/");
    Some(GameReadyMissionPickupSpec {
        id,
        item: (!item.is_empty()).then_some(item),
        quantity: raw.quantity.max(1).min(10_000),
        auto_equip: raw.auto_equip,
        position: arr3(sanitize_array3_finite(raw.position, [0.0, 0.0, 0.0])),
        rotation_ypr: arr3(sanitize_array3_finite(raw.rotation_ypr, [0.0, 0.0, 0.0])),
        radius: positive_clamped_or(
            raw.radius,
            0.15,
            8.0,
            default_game_data().world.mission.pickup_radius,
        ),
        scale: arr3(sanitize_array3_positive(
            raw.scale,
            default_game_data().world.mission.pickup_scale,
        )),
    })
}

pub(in super::super) fn sanitize_mission_target_spec(
    raw: RawMissionTargetSpec,
) -> Option<GameReadyMissionTargetSpec> {
    let id = sanitized_required_id(&raw.id)?;
    Some(GameReadyMissionTargetSpec {
        id,
        position: arr3(sanitize_array3_finite(raw.position, [0.0, 0.0, 0.0])),
        health: positive_clamped_or(
            raw.health,
            1.0,
            100_000.0,
            default_game_data().world.mission.target_health,
        ),
        scale: arr3(sanitize_array3_positive(
            raw.scale,
            default_game_data().world.mission.target_scale,
        )),
    })
}

pub(in super::super) fn sanitize_mission_hazard_spec(
    raw: RawMissionHazardSpec,
) -> Option<GameReadyMissionHazardSpec> {
    let id = sanitized_required_id(&raw.id)?;
    Some(GameReadyMissionHazardSpec {
        id,
        position: arr3(sanitize_array3_finite(raw.position, [0.0, 0.0, 0.0])),
        radius: positive_clamped_or(
            raw.radius,
            0.2,
            32.0,
            default_game_data().world.mission.hazard_radius,
        ),
        scale: arr3(sanitize_array3_positive(
            raw.scale,
            default_game_data().world.mission.hazard_scale,
        )),
    })
}

pub(in super::super) fn sanitize_mission_goal_spec(
    raw: RawMissionGoalSpec,
) -> Option<GameReadyMissionGoalSpec> {
    let id = sanitized_required_id(&raw.id)?;
    Some(GameReadyMissionGoalSpec {
        id,
        position: arr3(sanitize_array3_finite(raw.position, [0.0, 0.0, 0.0])),
        radius: positive_clamped_or(
            raw.radius,
            0.2,
            32.0,
            default_game_data().world.mission.goal_radius,
        ),
        scale: arr3(sanitize_array3_positive(
            raw.scale,
            default_game_data().world.mission.goal_scale,
        )),
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
            "game-ready definitions: rejected definition_ref='{}' reason='expected .ytyp@entry selector'",
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
    fn positive_values_use_fallback_before_clamping() {
        assert_eq!(positive_clamped_or(f32::NAN, 0.2, 32.0, 1.5), 1.5);
        assert_eq!(positive_clamped_or(-1.0, 0.2, 32.0, 1.5), 1.5);
        assert_eq!(positive_clamped_or(100.0, 0.2, 32.0, 1.5), 32.0);
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
