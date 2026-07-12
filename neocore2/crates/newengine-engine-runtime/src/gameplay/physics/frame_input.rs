use newengine_ecs::World;
use newengine_math::Quat;
use newengine_physics_api::{
    MeshColliderDto, PhysicsBodyFlagsDto, PhysicsColliderDto, PhysicsCommandDto,
    PhysicsCommandKindDto, PhysicsFrameBodySnapshot, PhysicsFrameColliderSnapshot,
    PhysicsFrameInput, PhysicsMaterialDto, PhysicsQueryDto, PhysicsQueryKindDto,
};
use newengine_physics_contracts::{CollisionShapeDesc, PhysicsBodyDesc};
use newengine_sim::{CharacterMotor, Velocity};
use newengine_transform::Transform;
use std::collections::{BTreeMap, BTreeSet};

use crate::gameplay::{
    collect_combat_queries, FpsDemoRules, PlayerStanceKind, PlayerStanceState, StaticMeshCollider,
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
) -> PhysicsFrameInput {
    let player_tuning = world
        .resource::<FpsDemoRules>()
        .map(|rules| rules.player.sanitized())
        .unwrap_or_default();

    let mut bodies = collect_body_snapshots(world);
    bodies.sort_by_key(|body| body.entity);

    let mut colliders = collect_terrain_colliders(world, &bodies, player_tuning.contact_skin);
    let (static_colliders, mut static_commands) =
        collect_static_mesh_colliders(world, static_mesh_revisions, fixed_tick);
    colliders.extend(static_colliders);
    colliders.sort_by_key(|collider| collider.entity);
    static_commands.sort_by_key(|command| command.seq);
    let mut queries = collect_ground_queries(world, player_tuning);
    queries.extend(collect_stand_clearance_queries(world, player_tuning));
    queries.extend(collect_combat_queries(world));
    queries.sort_by_key(|query| query.seq);

    PhysicsFrameInput {
        frame_index,
        fixed_tick,
        dt: dt.clamp(0.0001, 0.05),
        gravity: player_tuning.gravity,
        contact_skin: player_tuning.contact_skin,
        bodies,
        colliders,
        commands: static_commands,
        queries,
    }
}

fn collect_static_mesh_colliders(
    world: &World,
    known_revisions: &mut BTreeMap<u64, u64>,
    fixed_tick: u64,
) -> (Vec<PhysicsFrameColliderSnapshot>, Vec<PhysicsCommandDto>) {
    let mut snapshots = Vec::new();
    let mut current_revisions = BTreeMap::<u64, u64>::new();
    let mut delta_vertices = 0usize;
    let mut delta_triangles = 0usize;

    for (entity, collider) in world.query::<StaticMeshCollider>() {
        let transform = world.get::<Transform>(entity).copied().unwrap_or_default();
        let entity_key = entity.stable_u64();
        let revision = collider.runtime_revision(transform);
        current_revisions.insert(entity_key, revision);
        if known_revisions.get(&entity_key).copied() == Some(revision) {
            continue;
        }

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
    }

    let current_entities = current_revisions.keys().copied().collect::<BTreeSet<_>>();
    let mut commands = known_revisions
        .keys()
        .copied()
        .filter(|entity| !current_entities.contains(entity))
        .enumerate()
        .map(|(index, entity)| PhysicsCommandDto {
            seq: fixed_tick.rotate_left(17) ^ entity ^ index as u64,
            kind: PhysicsCommandKindDto::DestroyBody { entity },
        })
        .collect::<Vec<_>>();
    commands.sort_by_key(|command| command.seq);

    if !snapshots.is_empty() || !commands.is_empty() {
        newengine_ulog_api::ulog::info!(
            "physics sync: static mesh delta fixed_tick={} registered={} removed={} vertices={} triangles={} policy='register-on-change; geometry omitted from steady-state packets'",
            fixed_tick,
            snapshots.len(),
            commands.len(),
            delta_vertices,
            delta_triangles,
        );
    }

    *known_revisions = current_revisions;
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
        let bounds = world
            .get::<newengine_bounds::Bounds>(entity)
            .map(|b| b.world_aabb)
            .unwrap_or_else(|| translated_shape_aabb(*body, transform.position));

        let physics_rotation = world
            .get::<CharacterMotor>(entity)
            .map(|motor| Quat::from_rotation_y(motor.yaw))
            .unwrap_or(transform.rotation);

        bodies.push(PhysicsFrameBodySnapshot {
            entity: entity.stable_u64(),
            kind: body_kind_to_dto(body.kind),
            shape: shape_to_dto(body.shape),
            flags: PhysicsBodyFlagsDto {
                is_trigger: body.flags.is_trigger,
                participates_in_queries: body.flags.participates_in_queries,
                casts_contacts: body.flags.casts_contacts,
            },
            material: PhysicsMaterialDto {
                friction: body.material.friction,
                restitution: body.material.restitution,
                density: body.material.density,
            },
            position: vec3_to_arr(transform.position),
            rotation: quat_to_arr(physics_rotation),
            linear_velocity: vec3_to_arr(velocity.0),
            bounds_min: vec3_to_arr(bounds.min),
            bounds_max: vec3_to_arr(bounds.max),
        });
    }
    bodies
}

fn collect_ground_queries(
    world: &World,
    tuning: crate::gameplay::FpsPlayerTuning,
) -> Vec<PhysicsQueryDto> {
    let tuning = tuning.sanitized();
    let epsilon = ground_probe_origin_epsilon(tuning.contact_skin);
    let max_t = (tuning.contact_skin + tuning.ground_probe_distance).max(0.01);
    let mut queries = Vec::new();

    for entity in world.query2_ids::<CharacterMotor, PhysicsBodyDesc>() {
        let Some(transform) = world.get::<Transform>(entity).copied() else {
            continue;
        };
        let Some(body) = world.get::<PhysicsBodyDesc>(entity).copied() else {
            continue;
        };
        if !body.flags.participates_in_queries {
            continue;
        }
        let vertical_extent = collision_shape_vertical_extent(body.shape);
        queries.push(PhysicsQueryDto {
            seq: entity.stable_u64(),
            kind: PhysicsQueryKindDto::Ray {
                origin: [
                    transform.position.x,
                    transform.position.y - vertical_extent - epsilon,
                    transform.position.z,
                ],
                dir: [0.0, -1.0, 0.0],
                max_t,
            },
        });
    }
    queries
}

pub(super) const STAND_PROBE_SAMPLE_COUNT: usize = 5;
const STAND_PROBE_QUERY_SALT: u64 = 0x9e37_79b9_7f4a_7c15;

#[inline]
pub(super) fn stand_probe_query_seq(player_key: u64, sample_index: usize) -> u64 {
    player_key.rotate_left(23)
        ^ STAND_PROBE_QUERY_SALT
        ^ (sample_index as u64).wrapping_mul(0xd6e8_feb8_6659_fd93)
}

pub(super) fn stand_probe_owner(world: &World, query_seq: u64) -> Option<newengine_ecs::EntityId> {
    for (player, stance) in world.query::<PlayerStanceState>() {
        if stance.current != PlayerStanceKind::Crouched || !stance.stand_requested {
            continue;
        }
        for sample_index in 0..STAND_PROBE_SAMPLE_COUNT {
            if stand_probe_query_seq(player.stable_u64(), sample_index) == query_seq {
                return Some(player);
            }
        }
    }
    None
}

fn collect_stand_clearance_queries(
    world: &World,
    tuning: crate::gameplay::FpsPlayerTuning,
) -> Vec<PhysicsQueryDto> {
    let tuning = tuning.sanitized();
    let epsilon = ground_probe_origin_epsilon(tuning.contact_skin);
    let mut queries = Vec::new();

    for (player, stance) in world.query::<PlayerStanceState>() {
        if stance.current != PlayerStanceKind::Crouched || !stance.stand_requested {
            continue;
        }
        let Some(transform) = world.get::<Transform>(player).copied() else {
            continue;
        };
        let Some(body) = world.get::<PhysicsBodyDesc>(player).copied() else {
            continue;
        };
        let CollisionShapeDesc::Capsule {
            radius,
            half_height: current_half_height,
        } = body.shape.sanitized()
        else {
            continue;
        };
        let half_height_delta = (tuning.body_half_height - current_half_height).max(0.0);
        if half_height_delta <= 1.0e-5 {
            continue;
        }

        let max_t = (2.0 * half_height_delta + tuning.contact_skin).max(0.01);
        let top_y = transform.position.y + current_half_height + radius + epsilon;
        let radial = (radius * 0.62).max(0.01);
        let offsets = [
            [0.0, 0.0],
            [radial, 0.0],
            [-radial, 0.0],
            [0.0, radial],
            [0.0, -radial],
        ];
        for (sample_index, [offset_x, offset_z]) in offsets.into_iter().enumerate() {
            queries.push(PhysicsQueryDto {
                seq: stand_probe_query_seq(player.stable_u64(), sample_index),
                kind: PhysicsQueryKindDto::Ray {
                    origin: [
                        transform.position.x + offset_x,
                        top_y,
                        transform.position.z + offset_z,
                    ],
                    dir: [0.0, 1.0, 0.0],
                    max_t,
                },
            });
        }
    }
    queries
}

#[inline]
fn ground_probe_origin_epsilon(contact_skin: f32) -> f32 {
    (contact_skin.abs() * 0.25).clamp(0.001, 0.01)
}

#[inline]
fn collision_shape_vertical_extent(shape: CollisionShapeDesc) -> f32 {
    match shape.sanitized() {
        CollisionShapeDesc::Box { half_extents } => half_extents[1],
        CollisionShapeDesc::Sphere { radius } => radius,
        CollisionShapeDesc::Capsule {
            radius,
            half_height,
        } => radius + half_height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay::{spawn_default_player, FpsPlayerTuning};
    use newengine_math::Vec3;

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
        let input = build_frame_input(&world, 1, 1, 1.0 / 60.0, &mut static_mesh_revisions);
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

        let second = build_frame_input(&world, 2, 2, 1.0 / 60.0, &mut static_mesh_revisions);
        assert!(
            second.colliders.is_empty(),
            "unchanged static mesh must not be resent"
        );
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
        let first = build_frame_input(&world, 1, 1, 1.0 / 60.0, &mut revisions);
        assert_eq!(first.colliders.len(), 1);
        world.despawn(entity);
        let second = build_frame_input(&world, 2, 2, 1.0 / 60.0, &mut revisions);
        assert!(second.commands.iter().any(|command| matches!(
            command.kind,
            PhysicsCommandKindDto::DestroyBody { entity } if entity == entity_key
        )));
    }

    #[test]
    fn frame_input_places_ground_ray_below_player_capsule() {
        let mut world = World::new();
        let tuning = FpsPlayerTuning::default().sanitized();
        let vertical_extent = tuning.body_half_height + tuning.body_radius;
        let center_y = vertical_extent + tuning.contact_skin;
        let player = spawn_default_player(
            &mut world,
            None,
            "ground-probe-player",
            Vec3::new(3.0, center_y, -2.0),
        );

        let input = build_frame_input(&world, 4, 9, 1.0 / 60.0, &mut BTreeMap::new());
        let query = input
            .queries
            .iter()
            .find(|query| query.seq == player.stable_u64())
            .expect("player ground query");

        match query.kind {
            PhysicsQueryKindDto::Ray { origin, dir, max_t } => {
                let epsilon = ground_probe_origin_epsilon(tuning.contact_skin);
                assert!((origin[0] - 3.0).abs() <= 1.0e-6);
                assert!((origin[1] - (tuning.contact_skin - epsilon)).abs() <= 1.0e-6);
                assert!((origin[2] + 2.0).abs() <= 1.0e-6);
                assert_eq!(dir, [0.0, -1.0, 0.0]);
                assert!(
                    (max_t - (tuning.contact_skin + tuning.ground_probe_distance)).abs() <= 1.0e-6
                );
            }
            ref other => panic!("expected ground ray, got {other:?}"),
        }
    }

    #[test]
    fn crouched_player_requests_five_stand_clearance_rays() {
        let mut world = World::new();
        let tuning = FpsPlayerTuning::default().sanitized();
        let player = spawn_default_player(
            &mut world,
            None,
            "stand-probe-player",
            Vec3::new(0.0, tuning.body_half_height + tuning.body_radius, 0.0),
        );
        crate::gameplay::apply_player_stance_geometry(
            &mut world,
            player,
            PlayerStanceKind::Crouched,
            tuning,
            3,
        );
        if let Some(stance) = world.get_mut::<PlayerStanceState>(player) {
            stance.stand_requested = true;
        }

        let input = build_frame_input(&world, 5, 10, 1.0 / 60.0, &mut BTreeMap::new());
        let stand_queries = input
            .queries
            .iter()
            .filter(|query| stand_probe_owner(&world, query.seq) == Some(player))
            .collect::<Vec<_>>();
        assert_eq!(stand_queries.len(), STAND_PROBE_SAMPLE_COUNT);
        assert!(stand_queries.iter().all(|query| matches!(
            query.kind,
            PhysicsQueryKindDto::Ray {
                dir: [0.0, 1.0, 0.0],
                ..
            }
        )));
    }
}
