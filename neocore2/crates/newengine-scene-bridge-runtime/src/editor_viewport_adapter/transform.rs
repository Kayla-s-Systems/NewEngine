use newengine_bounds::Bounds;
use newengine_ecs::{EntityId, World};
use newengine_math::Vec3;
use newengine_transform::Transform;
use newengine_world_authoring_api::{
    AuthoredMapPlacement, AuthoredMapPlacementReplicaScaleState, AuthoredMapPlacementSource,
};

use newengine_gameplay_world_runtime::gameplay::StaticMeshCollider;

pub struct EngineEditorTransformEffects;

impl newengine_editor_viewport_runtime::EditorTransformEffects for EngineEditorTransformEffects {
    #[inline]
    fn sync_authored_replicas(&mut self, world: &mut World, primary: EntityId) {
        sync_editor_transform_side_effects(world, primary);
    }
}

/// Captures manual calibration for runtime-driven transforms, then preserves the existing authored
/// map replica synchronization path. Gizmo, numeric inspector, undo/redo and cancel all converge here.
pub fn sync_editor_transform_side_effects(world: &mut World, primary: EntityId) {
    if let Some(edited) = world.get::<Transform>(primary).copied() {
        if let Some(runtime_override) =
            world.get_mut::<newengine_transform::RuntimeTransformEditOverride>(primary)
        {
            runtime_override.capture_edited_transform(edited);
        }
    }
    sync_authored_map_placement_replicas(world, primary);
}

#[inline]
fn min_vec3_component(value: Vec3) -> f32 {
    value.x.min(value.y).min(value.z)
}
#[inline]
fn max_abs_vec3_component(value: Vec3) -> f32 {
    value.x.abs().max(value.y.abs()).max(value.z.abs())
}

pub fn sync_authored_map_placement_replicas(world: &mut World, primary: EntityId) {
    let Some(authored) = world.get::<AuthoredMapPlacement>(primary).cloned() else {
        return;
    };
    if !authored.primary || authored.source != AuthoredMapPlacementSource::DiscretePlacement {
        return;
    }
    let Some(primary_transform) = world.get::<Transform>(primary).copied() else {
        return;
    };
    if !primary_transform.position.is_finite()
        || !primary_transform.rotation.is_finite()
        || !primary_transform.scale.is_finite()
        || min_vec3_component(primary_transform.scale) <= 0.000_001
    {
        return;
    }

    let replicas = world
        .query::<AuthoredMapPlacement>()
        .filter_map(|(entity, candidate)| {
            (!candidate.primary
                && candidate.source == authored.source
                && candidate.map_ref == authored.map_ref
                && candidate.placement_id == authored.placement_id)
                .then_some(entity)
        })
        .collect::<Vec<_>>();

    for replica in replicas {
        if let Some(transform) = world.get_mut_tracked::<Transform>(replica) {
            transform.position = primary_transform.position;
            transform.rotation = primary_transform.rotation;
        }
        let Some(scale_state) = world
            .get::<AuthoredMapPlacementReplicaScaleState>(replica)
            .copied()
        else {
            continue;
        };
        let previous = scale_state.last_authored_scale;
        if !previous.is_finite() || min_vec3_component(previous) <= 0.000_001 {
            continue;
        }
        let ratio = Vec3::new(
            primary_transform.scale.x / previous.x,
            primary_transform.scale.y / previous.y,
            primary_transform.scale.z / previous.z,
        );
        if !ratio.is_finite()
            || min_vec3_component(ratio) <= 0.000_001
            || max_abs_vec3_component(ratio - Vec3::ONE) <= 1.0e-6
        {
            continue;
        }
        let Some(collider) = world.get::<StaticMeshCollider>(replica).cloned() else {
            continue;
        };
        let vertices = collider
            .vertices
            .iter()
            .map(|v| [v[0] * ratio.x, v[1] * ratio.y, v[2] * ratio.z])
            .collect::<Vec<_>>();
        let triangles = collider.triangles.as_ref().to_vec();
        let Ok(rescaled) = StaticMeshCollider::new(vertices, triangles)
            .map(|value| value.with_material(collider.friction, collider.restitution))
        else {
            continue;
        };
        let local_bounds = rescaled.local_bounds;
        let _ = world.insert(replica, rescaled);
        let _ = world.insert(replica, Bounds::from_local_aabb(local_bounds));
        let _ = world.insert(
            replica,
            AuthoredMapPlacementReplicaScaleState {
                last_authored_scale: primary_transform.scale,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_math::Quat;

    #[test]
    fn runtime_transform_side_effect_captures_editor_position_and_rotation() {
        let mut world = World::new();
        let entity = world.spawn();
        let base = Transform {
            position: Vec3::new(0.2, 1.0, -0.3),
            rotation: Quat::from_rotation_y(0.25),
            scale: Vec3::ONE,
        };
        let _ = world.insert(entity, base);
        let _ = world.insert(
            entity,
            newengine_transform::RuntimeTransformEditOverride::new(base),
        );
        let manual_rotation = Quat::from_rotation_z(-0.15);
        let edited = Transform {
            position: base.position + Vec3::new(0.04, -0.02, 0.08),
            rotation: base.rotation * manual_rotation,
            scale: Vec3::ONE,
        };
        let _ = world.insert(entity, edited);
        sync_editor_transform_side_effects(&mut world, entity);

        let next_base = Transform {
            position: Vec3::new(0.4, 1.2, -0.1),
            rotation: Quat::from_rotation_y(0.45),
            scale: Vec3::ONE,
        };
        let resolved = world
            .get_mut::<newengine_transform::RuntimeTransformEditOverride>(entity)
            .unwrap()
            .resolve_from_base(next_base);
        assert!(
            (resolved.position - (next_base.position + Vec3::new(0.04, -0.02, 0.08))).length()
                < 1.0e-6
        );
        let expected_rotation = (next_base.rotation * manual_rotation).normalize();
        assert!(resolved.rotation.dot(expected_rotation).abs() > 0.999_999);
    }
}
