use newengine_ecs::World;
use newengine_physics_api::{
    PhysicsBodyFlagsDto, PhysicsFrameBodySnapshot, PhysicsFrameInput, PhysicsMaterialDto,
};
use newengine_physics_contracts::PhysicsBodyDesc;
use newengine_sim::Velocity;
use newengine_transform::Transform;

use crate::gameplay::FpsDemoRules;

use super::terrain_colliders::collect_terrain_colliders;
use super::util::{
    body_kind_to_dto, quat_to_arr, shape_to_dto, translated_shape_aabb, vec3_to_arr,
};

pub(super) fn build_frame_input(
    world: &World,
    frame_index: u64,
    fixed_tick: u64,
    dt: f32,
) -> PhysicsFrameInput {
    let player_tuning = world
        .resource::<FpsDemoRules>()
        .map(|rules| rules.player.sanitized())
        .unwrap_or_default();

    let mut bodies = collect_body_snapshots(world);
    bodies.sort_by_key(|body| body.entity);

    let mut colliders = collect_terrain_colliders(world, &bodies, player_tuning.contact_skin);
    colliders.sort_by_key(|collider| collider.entity);

    PhysicsFrameInput {
        frame_index,
        fixed_tick,
        dt: dt.clamp(0.0001, 0.05),
        gravity: player_tuning.gravity,
        contact_skin: player_tuning.contact_skin,
        bodies,
        colliders,
        commands: Vec::new(),
        queries: Vec::new(),
    }
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
            rotation: quat_to_arr(transform.rotation),
            linear_velocity: vec3_to_arr(velocity.0),
            bounds_min: vec3_to_arr(bounds.min),
            bounds_max: vec3_to_arr(bounds.max),
        });
    }
    bodies
}
