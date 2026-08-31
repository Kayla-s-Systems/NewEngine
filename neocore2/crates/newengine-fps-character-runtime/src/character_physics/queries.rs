use newengine_ecs::{EntityId, World};
use newengine_engine_runtime::gameplay::{
    CollisionShapeDesc, PhysicsBodyDesc, PlayerController, PlayerStanceKind, PlayerStanceState,
};
use newengine_gameplay_fps_api::FpsPlayerTuning;
use newengine_model_contact_api::{ModelFootPoseState, ModelFootSide};
use newengine_physics_api::{PhysicsQueryDto, PhysicsQueryKindDto};
use newengine_sim::CharacterMotor;
use newengine_transform::Transform;

use super::tuning::tuning;

const FOOT_PROBE_QUERY_SALT: u64 = 0xa076_1d64_78bd_642f;
const FOOT_PROBE_SIDE_MIX: u64 = 0xe703_7ed1_a0b4_28db;
const FOOT_PROBE_ORIGIN_UP: f32 = 0.25;
const FOOT_PROBE_BELOW_FOOT: f32 = 0.30;

#[inline]
pub(super) fn foot_probe_query_seq(player_key: u64, side: ModelFootSide) -> u64 {
    let side_index = match side {
        ModelFootSide::Left => 1_u64,
        ModelFootSide::Right => 2_u64,
    };
    player_key.rotate_left(37)
        ^ FOOT_PROBE_QUERY_SALT
        ^ side_index.wrapping_mul(FOOT_PROBE_SIDE_MIX)
}

pub(super) fn foot_probe_owner(world: &World, query_seq: u64) -> Option<(EntityId, ModelFootSide)> {
    for (player, _) in world.query::<ModelFootPoseState>() {
        for side in [ModelFootSide::Left, ModelFootSide::Right] {
            if foot_probe_query_seq(player.stable_u64(), side) == query_seq {
                return Some((player, side));
            }
        }
    }
    None
}

fn collect_foot_contact_queries(world: &World, tuning: FpsPlayerTuning) -> Vec<PhysicsQueryDto> {
    let max_t = foot_probe_max_t(tuning.contact_skin);
    let mut queries = Vec::new();
    for (player, pose) in world.query::<ModelFootPoseState>() {
        if world.get::<PlayerController>(player).is_none()
            || world.get::<CharacterMotor>(player).is_none()
        {
            continue;
        }
        let Some(body) = world.get::<PhysicsBodyDesc>(player).copied() else {
            continue;
        };
        if !body.flags.participates_in_queries {
            continue;
        }
        for (side, foot) in [
            (ModelFootSide::Left, pose.left),
            (ModelFootSide::Right, pose.right),
        ] {
            if !foot.valid || !foot.position_world.is_finite() {
                continue;
            }
            queries.push(PhysicsQueryDto {
                seq: foot_probe_query_seq(player.stable_u64(), side),
                ignore_entity: Some(player.stable_u64()),
                kind: PhysicsQueryKindDto::Ray {
                    origin: [
                        foot.position_world.x,
                        foot.position_world.y + FOOT_PROBE_ORIGIN_UP,
                        foot.position_world.z,
                    ],
                    dir: [0.0, -1.0, 0.0],
                    max_t,
                },
            });
        }
    }
    queries
}

#[inline]
pub(super) fn foot_probe_max_t(contact_skin: f32) -> f32 {
    FOOT_PROBE_ORIGIN_UP
        + FOOT_PROBE_BELOW_FOOT
        + if contact_skin.is_finite() {
            contact_skin.max(0.0)
        } else {
            0.0
        }
}

const STAND_PROBE_SAMPLE_COUNT: usize = 5;
const STAND_PROBE_QUERY_SALT: u64 = 0x9e37_79b9_7f4a_7c15;
const STAND_PROBE_SAMPLE_MIX: u64 = 0xd6e8_feb8_6659_fd93;

pub fn collect_character_queries(world: &World) -> Vec<PhysicsQueryDto> {
    let tuning = tuning(world);
    let mut queries = collect_ground_queries(world, tuning);
    queries.extend(collect_foot_contact_queries(world, tuning));
    queries.extend(collect_stand_clearance_queries(world, tuning));
    queries
}

fn collect_ground_queries(world: &World, tuning: FpsPlayerTuning) -> Vec<PhysicsQueryDto> {
    let tuning = tuning.sanitized();
    let epsilon = ground_probe_origin_epsilon(tuning.contact_skin);
    let max_t = ground_probe_max_t(tuning.contact_skin, tuning.ground_probe_distance);
    let mut queries = Vec::new();

    for entity in world.query2_ids::<CharacterMotor, PhysicsBodyDesc>() {
        if world.get::<PlayerController>(entity).is_none() {
            continue;
        }
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
            ignore_entity: Some(entity.stable_u64()),
            kind: PhysicsQueryKindDto::Ray {
                origin: [
                    transform.position.x,
                    // The physics backend does not preserve `contact_skin` as a physical
                    // separation margin. At steady-state the capsule sole can sit exactly on
                    // the surface, so starting below the sole puts the ray behind the plane.
                    // Owner-seq queries exclude the player body in Gravitas/Jolt, therefore
                    // the robust origin is a tiny distance *inside/above* the sole.
                    transform.position.y - vertical_extent + epsilon,
                    transform.position.z,
                ],
                dir: [0.0, -1.0, 0.0],
                max_t,
            },
        });
    }
    queries
}

#[inline]
fn stand_probe_query_seq(player_key: u64, sample_index: usize) -> u64 {
    player_key.rotate_left(23)
        ^ STAND_PROBE_QUERY_SALT
        ^ (sample_index as u64).wrapping_mul(STAND_PROBE_SAMPLE_MIX)
}

pub(super) fn stand_probe_owner(world: &World, query_seq: u64) -> Option<EntityId> {
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

fn collect_stand_clearance_queries(world: &World, tuning: FpsPlayerTuning) -> Vec<PhysicsQueryDto> {
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
                ignore_entity: Some(player.stable_u64()),
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
pub(super) fn ground_probe_origin_epsilon(contact_skin: f32) -> f32 {
    (contact_skin.abs() * 0.25).clamp(0.001, 0.01)
}

#[inline]
pub(super) fn ground_probe_max_t(contact_skin: f32, ground_probe_distance: f32) -> f32 {
    let skin = if contact_skin.is_finite() {
        contact_skin.max(0.0)
    } else {
        0.0
    };
    let probe = if ground_probe_distance.is_finite() {
        ground_probe_distance.max(0.0)
    } else {
        0.0
    };
    // Origin moved epsilon above the sole, so preserve the authored reach below the sole.
    (skin + probe + ground_probe_origin_epsilon(skin)).max(0.01)
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
        CollisionShapeDesc::Cylinder { half_height, .. } => half_height,
    }
}
