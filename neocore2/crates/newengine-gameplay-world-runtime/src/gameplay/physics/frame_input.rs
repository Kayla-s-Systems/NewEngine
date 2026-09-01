use newengine_ecs::World;
use newengine_physics_api::{
    MeshColliderDto, PhysicsBodyFlagsDto, PhysicsColliderDto, PhysicsCommandDto,
    PhysicsCommandKindDto, PhysicsFrameBodySnapshot, PhysicsFrameColliderSnapshot,
    PhysicsFrameInput, PhysicsMaterialDto,
};
use newengine_physics_contracts::PhysicsBodyDesc;
use newengine_sim::{AngularVelocity, Velocity};
use newengine_transform::Transform;
use std::collections::{BTreeMap, BTreeSet};

use crate::gameplay::{
    GameplayPhysicsQueryProviderRegistry, PendingPhysicsImpulse, PhysicsWorldSettings,
    StaticMeshCollider,
};

use super::terrain_colliders::collect_terrain_colliders;
use super::util::{
    body_kind_to_dto, quat_to_arr, shape_to_dto, translated_shape_aabb, vec3_to_arr,
};

pub(super) fn build_frame_input(
    world: &World,
    frame_index: u64,
    fixed_tick: u64,
    dt: f32,
    static_mesh_revisions: &mut BTreeMap<u64, u64>,
    gameplay_queries: &GameplayPhysicsQueryProviderRegistry,
) -> PhysicsFrameInput {
    let physics_world = world
        .resource::<PhysicsWorldSettings>()
        .copied()
        .unwrap_or_default()
        .sanitized();

    // Launch readiness is allowed to advance static collision while gameplay bodies stay
    // dormant. Without this gate a character can start integrating gravity before streamed
    // authored collision has crossed the physics service boundary and fall through the map.
    let gameplay_bodies_active = world
        .resource::<crate::gameplay::WorldActivationState>()
        .map(|gate| gate.is_ready())
        .unwrap_or(true);
    let mut bodies = if gameplay_bodies_active {
        collect_body_snapshots(world)
    } else {
        Vec::new()
    };
    bodies.sort_by_key(|body| body.entity);

    let mut colliders = collect_terrain_colliders(world, &bodies, physics_world.contact_skin);
    let (static_colliders, mut static_commands) = collect_static_mesh_colliders(
        world,
        static_mesh_revisions,
        fixed_tick,
        physics_world.static_collider_batch_size,
    );
    colliders.extend(static_colliders);
    colliders.sort_by_key(|collider| collider.entity);
    static_commands.sort_by_key(|command| command.seq);
    let mut commands = static_commands;
    commands.extend(
        world
            .query::<PendingPhysicsImpulse>()
            .map(|(entity, request)| PhysicsCommandDto {
                seq: request.sequence ^ entity.stable_u64().rotate_left(23),
                kind: PhysicsCommandKindDto::ApplyImpulse {
                    entity: entity.stable_u64(),
                    impulse: vec3_to_arr(request.impulse),
                    point: vec3_to_arr(request.point),
                },
            }),
    );
    commands.sort_by_key(|command| command.seq);
    let mut queries = gameplay_queries.collect_queries(world);
    queries.sort_by_key(|query| query.seq);

    PhysicsFrameInput {
        frame_index,
        fixed_tick,
        dt: dt.clamp(0.0001, 0.05),
        gravity: physics_world.gravity,
        contact_skin: physics_world.contact_skin,
        bodies,
        colliders,
        commands,
        queries,
    }
}

fn collect_static_mesh_colliders(
    world: &World,
    known_revisions: &mut BTreeMap<u64, u64>,
    fixed_tick: u64,
    batch_size: usize,
) -> (Vec<PhysicsFrameColliderSnapshot>, Vec<PhysicsCommandDto>) {
    // First collect only cheap revision metadata. Mesh arrays are cloned only for the bounded
    // batch that actually crosses the service boundary on this fixed tick.
    let mut current_entities = BTreeSet::<u64>::new();
    let mut changed = Vec::new();
    for (entity, collider) in world.query::<StaticMeshCollider>() {
        let transform = world.get::<Transform>(entity).copied().unwrap_or_default();
        let entity_key = entity.stable_u64();
        let revision = collider.runtime_revision(transform);
        current_entities.insert(entity_key);
        if known_revisions.get(&entity_key).copied() != Some(revision) {
            changed.push((entity_key, entity, revision, transform));
        }
    }
    changed.sort_by_key(|(entity_key, _, _, _)| *entity_key);

    // Removals are not warmup work: propagate them immediately so stale collision disappears
    // on the next service step even while a large add/change backlog is being streamed in.
    let removed = known_revisions
        .keys()
        .copied()
        .filter(|entity| !current_entities.contains(entity))
        .collect::<Vec<_>>();
    let mut commands = removed
        .iter()
        .copied()
        .enumerate()
        .map(|(index, entity)| PhysicsCommandDto {
            seq: fixed_tick.rotate_left(17) ^ entity ^ index as u64,
            kind: PhysicsCommandKindDto::DestroyBody { entity },
        })
        .collect::<Vec<_>>();
    commands.sort_by_key(|command| command.seq);
    for entity in removed {
        known_revisions.remove(&entity);
    }

    let pending_before = changed.len();
    let mut snapshots = Vec::with_capacity(pending_before.min(batch_size));
    let mut delta_vertices = 0usize;
    let mut delta_triangles = 0usize;
    for (entity_key, entity, revision, transform) in changed.into_iter().take(batch_size) {
        let Some(collider) = world.get::<StaticMeshCollider>(entity) else {
            continue;
        };
        let (bounds_min, bounds_max) = rotated_aabb(collider.local_bounds, transform);
        delta_vertices = delta_vertices.saturating_add(collider.vertices.len());
        delta_triangles = delta_triangles.saturating_add(collider.triangles.len());
        snapshots.push(PhysicsFrameColliderSnapshot {
            entity: entity_key,
            collider: PhysicsColliderDto::Mesh(MeshColliderDto {
                vertices: collider.vertices.as_ref().to_vec(),
                triangles: collider.triangles.as_ref().to_vec(),
                material_indices: Vec::new(),
            }),
            flags: PhysicsBodyFlagsDto {
                is_trigger: false,
                participates_in_queries: true,
                casts_contacts: true,
                continuous_collision: false,
            },
            material: PhysicsMaterialDto {
                friction: collider.friction,
                restitution: collider.restitution,
                density: 0.0,
            },
            position: vec3_to_arr(transform.position),
            rotation: quat_to_arr(transform.rotation),
            bounds_min: vec3_to_arr(bounds_min),
            bounds_max: vec3_to_arr(bounds_max),
        });
        // This revision is acknowledged as sent only for the batch that actually crossed the
        // service boundary. Unsent entries remain absent/old and are selected next fixed tick.
        known_revisions.insert(entity_key, revision);
    }

    if !snapshots.is_empty() || !commands.is_empty() {
        newengine_ulog_api::ulog::info!(
            "physics sync: static mesh delta fixed_tick={} registered={} removed={} pending_before={} pending_after={} batch_limit={} vertices={} triangles={} policy='bounded register-on-change; removals immediate; geometry omitted from steady-state packets'",
            fixed_tick,
            snapshots.len(),
            commands.len(),
            pending_before,
            pending_before.saturating_sub(snapshots.len()),
            batch_size,
            delta_vertices,
            delta_triangles,
        );
    }

    (snapshots, commands)
}

fn rotated_aabb(
    local: newengine_bounds::Aabb,
    transform: Transform,
) -> (newengine_math::Vec3, newengine_math::Vec3) {
    let min = local.min;
    let max = local.max;
    let mut world_min = newengine_math::Vec3::splat(f32::INFINITY);
    let mut world_max = newengine_math::Vec3::splat(f32::NEG_INFINITY);
    for x in [min.x, max.x] {
        for y in [min.y, max.y] {
            for z in [min.z, max.z] {
                let point =
                    transform.position + transform.rotation * newengine_math::Vec3::new(x, y, z);
                world_min = world_min.min(point);
                world_max = world_max.max(point);
            }
        }
    }
    (world_min, world_max)
}

fn collect_body_snapshots(world: &World) -> Vec<PhysicsFrameBodySnapshot> {
    let mut bodies = Vec::new();
    for (entity, body) in world.query::<PhysicsBodyDesc>() {
        let transform = world.get::<Transform>(entity).copied().unwrap_or_default();
        let velocity = world.get::<Velocity>(entity).copied().unwrap_or_default();
        let angular_velocity = world
            .get::<AngularVelocity>(entity)
            .copied()
            .unwrap_or_default();
        let bounds = world
            .get::<newengine_bounds::Bounds>(entity)
            .map(|b| b.world_aabb)
            .unwrap_or_else(|| translated_shape_aabb(*body, transform.position));

        // CharacterMotor yaw is camera/view orientation. Physics follows the
        // PlayerActor body transform so free-look cannot rotate the character body.
        let physics_rotation = transform.rotation;

        bodies.push(PhysicsFrameBodySnapshot {
            entity: entity.stable_u64(),
            kind: body_kind_to_dto(body.kind),
            shape: shape_to_dto(body.shape),
            flags: PhysicsBodyFlagsDto {
                is_trigger: body.flags.is_trigger,
                participates_in_queries: body.flags.participates_in_queries,
                casts_contacts: body.flags.casts_contacts,
                continuous_collision: body.flags.continuous_collision,
            },
            material: PhysicsMaterialDto {
                friction: body.material.friction,
                restitution: body.material.restitution,
                density: body.material.density,
            },
            position: vec3_to_arr(transform.position),
            rotation: quat_to_arr(physics_rotation),
            linear_velocity: vec3_to_arr(velocity.0),
            angular_velocity: vec3_to_arr(angular_velocity.0),
            linear_damping: body.linear_damping,
            angular_damping: body.angular_damping,
            bounds_min: vec3_to_arr(bounds.min),
            bounds_max: vec3_to_arr(bounds.max),
        });
    }
    bodies
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_math::Quat;

    #[test]
    fn prelaunch_gate_streams_static_collision_without_dynamic_bodies() {
        let mut world = World::new();
        world.insert_resource(crate::gameplay::WorldActivationState::new(
            "waiting for authored collision",
        ));

        let body_entity = world.spawn();
        let _ = world.insert(body_entity, Transform::default());
        let _ = world.insert(
            body_entity,
            PhysicsBodyDesc::dynamic_solid(newengine_physics_contracts::CollisionShapeDesc::Box {
                half_extents: [0.5, 0.5, 0.5],
            }),
        );

        let collider_entity = world.spawn();
        let _ = world.insert(collider_entity, Transform::default());
        let _ = world.insert(
            collider_entity,
            StaticMeshCollider::new(
                vec![[-1.0, 0.0, -1.0], [1.0, 0.0, -1.0], [0.0, 0.0, 1.0]],
                vec![[0, 1, 2]],
            )
            .unwrap(),
        );

        let mut revisions = BTreeMap::new();
        let queries = GameplayPhysicsQueryProviderRegistry::new();
        let prelaunch = build_frame_input(&world, 1, 1, 1.0 / 60.0, &mut revisions, &queries);
        assert!(prelaunch.bodies.is_empty());
        assert_eq!(prelaunch.colliders.len(), 1);

        world
            .resource_mut::<crate::gameplay::WorldActivationState>()
            .unwrap()
            .mark_ready(2, "collision ready");
        let active = build_frame_input(&world, 2, 2, 1.0 / 60.0, &mut revisions, &queries);
        assert_eq!(active.bodies.len(), 1);
    }

    #[test]
    fn frame_input_preserves_dynamic_body_angular_velocity() {
        let mut world = World::new();
        let entity = world.spawn();
        let _ = world.insert(entity, Transform::default());
        let _ = world.insert(
            entity,
            PhysicsBodyDesc::dynamic_solid(newengine_physics_contracts::CollisionShapeDesc::Box {
                half_extents: [0.1, 0.1, 0.5],
            }),
        );
        let _ = world.insert(entity, Velocity(newengine_math::Vec3::new(1.0, 2.0, 3.0)));
        let _ = world.insert(
            entity,
            AngularVelocity(newengine_math::Vec3::new(1.8, 3.2, 0.9)),
        );
        let snapshot = collect_body_snapshots(&world)
            .into_iter()
            .find(|snapshot| snapshot.entity == entity.stable_u64())
            .expect("dynamic body snapshot");
        assert_eq!(snapshot.linear_velocity, [1.0, 2.0, 3.0]);
        assert_eq!(snapshot.angular_velocity, [1.8, 3.2, 0.9]);
    }

    #[test]
    fn frame_input_projects_static_mesh_collider() {
        let mut world = World::new();
        let entity = world.spawn();
        let _ = world.insert(
            entity,
            Transform {
                position: newengine_math::Vec3::new(2.0, 3.0, 4.0),
                rotation: Quat::IDENTITY,
                scale: newengine_math::Vec3::ONE,
            },
        );
        let collider = StaticMeshCollider::new(
            vec![[-1.0, 0.0, -1.0], [1.0, 0.0, -1.0], [0.0, 0.0, 1.0]],
            vec![[0, 1, 2]],
        )
        .expect("valid collider");
        let _ = world.insert(entity, collider);

        let mut static_mesh_revisions = BTreeMap::new();
        let input = build_frame_input(
            &world,
            1,
            1,
            1.0 / 60.0,
            &mut static_mesh_revisions,
            &GameplayPhysicsQueryProviderRegistry::new(),
        );
        let snapshot = input
            .colliders
            .iter()
            .find(|snapshot| snapshot.entity == entity.stable_u64())
            .expect("static collider snapshot");
        assert_eq!(snapshot.position, [2.0, 3.0, 4.0]);
        match &snapshot.collider {
            PhysicsColliderDto::Mesh(mesh) => {
                assert_eq!(mesh.vertices.len(), 3);
                assert_eq!(mesh.triangles, vec![[0, 1, 2]]);
            }
            other => panic!("expected mesh collider, got {other:?}"),
        }

        let second = build_frame_input(
            &world,
            2,
            2,
            1.0 / 60.0,
            &mut static_mesh_revisions,
            &GameplayPhysicsQueryProviderRegistry::new(),
        );
        assert!(
            second.colliders.is_empty(),
            "unchanged static mesh must not be resent"
        );
    }

    #[test]
    fn static_mesh_registration_is_bounded_and_incremental() {
        let mut world = World::new();
        for index in 0..130 {
            let entity = world.spawn();
            let _ = world.insert(
                entity,
                Transform {
                    position: newengine_math::Vec3::new(index as f32 * 2.0, 0.0, 0.0),
                    ..Transform::default()
                },
            );
            let _ = world.insert(
                entity,
                StaticMeshCollider::new(
                    vec![[-0.5, 0.0, -0.5], [0.5, 0.0, -0.5], [0.0, 0.0, 0.5]],
                    vec![[0, 1, 2]],
                )
                .unwrap(),
            );
        }

        let mut revisions = BTreeMap::new();
        let queries = GameplayPhysicsQueryProviderRegistry::new();
        let first = collect_static_mesh_colliders(&world, &mut revisions, 1, 128);
        assert_eq!(first.0.len(), 128);
        assert_eq!(revisions.len(), 128);

        let second = collect_static_mesh_colliders(&world, &mut revisions, 2, 128);
        assert_eq!(second.0.len(), 2);
        assert_eq!(revisions.len(), 130);

        let third = collect_static_mesh_colliders(&world, &mut revisions, 3, 128);
        assert!(third.0.is_empty());
        assert!(third.1.is_empty());
        let _ = queries;
    }

    #[test]
    fn frame_input_emits_destroy_for_removed_static_mesh() {
        let mut world = World::new();
        let entity = world.spawn();
        let _ = world.insert(entity, Transform::default());
        let _ = world.insert(
            entity,
            StaticMeshCollider::new(
                vec![[-1.0, 0.0, -1.0], [1.0, 0.0, -1.0], [0.0, 0.0, 1.0]],
                vec![[0, 1, 2]],
            )
            .unwrap(),
        );
        let entity_key = entity.stable_u64();
        let mut revisions = BTreeMap::new();
        let first = build_frame_input(
            &world,
            1,
            1,
            1.0 / 60.0,
            &mut revisions,
            &GameplayPhysicsQueryProviderRegistry::new(),
        );
        assert_eq!(first.colliders.len(), 1);
        world.despawn(entity);
        let second = build_frame_input(
            &world,
            2,
            2,
            1.0 / 60.0,
            &mut revisions,
            &GameplayPhysicsQueryProviderRegistry::new(),
        );
        assert!(second.commands.iter().any(|command| matches!(
            command.kind,
            PhysicsCommandKindDto::DestroyBody { entity } if entity == entity_key
        )));
    }
}
