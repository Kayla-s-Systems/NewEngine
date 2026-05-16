use std::collections::BTreeMap;

use newengine_ecs::{EntityId, World};
use newengine_physics_api::{
    PhysicsBodyPoseUpdate, PhysicsBodyVelocityUpdate, PhysicsFrameOutput, PhysicsStepReportDto,
};
use newengine_physics_contracts::{PhysicsBodyDesc, PhysicsEvent, PhysicsStepReport};
use newengine_sim::{CharacterMotor, Velocity};
use newengine_transform::Transform;

use super::util::{arr_to_quat, arr_to_vec3};

pub(super) fn apply_frame_output(world: &mut World, output: PhysicsFrameOutput) {
    let mut key_to_entity = BTreeMap::new();
    for (entity, _) in world.query::<PhysicsBodyDesc>() {
        key_to_entity.insert(entity.stable_u64(), entity);
    }

    for update in output.pose_updates {
        apply_pose_update(world, &key_to_entity, update);
    }

    for update in output.velocity_updates {
        apply_velocity_update(world, &key_to_entity, update);
    }

    world.insert_resource(report_from_dto(output.report, output.events, &key_to_entity));
}

fn apply_pose_update(
    world: &mut World,
    key_to_entity: &BTreeMap<u64, EntityId>,
    update: PhysicsBodyPoseUpdate,
) {
    let Some(entity) = key_to_entity.get(&update.entity).copied() else { return; };
    let controlled_body = is_directly_controlled_body(world, entity);
    if let Some(transform) = world.get_mut::<Transform>(entity) {
        transform.position = arr_to_vec3(update.position);
        if !controlled_body {
            transform.rotation = arr_to_quat(update.rotation);
        }
    }
}

fn apply_velocity_update(
    world: &mut World,
    key_to_entity: &BTreeMap<u64, EntityId>,
    update: PhysicsBodyVelocityUpdate,
) {
    let Some(entity) = key_to_entity.get(&update.entity).copied() else { return; };
    let physics_velocity = arr_to_vec3(update.linear_velocity);
    let next = if is_directly_controlled_body(world, entity) {
        let current = world.get::<Velocity>(entity).copied().unwrap_or_default().0;
        // Character motor owns lateral velocity and look/yaw. Physics owns vertical
        // resolution/gravity. This prevents Jolt from erasing WASD and mouse look
        // while still applying floor contacts.
        newengine_math::Vec3::new(current.x, physics_velocity.y, current.z)
    } else {
        physics_velocity
    };
    let _ = world.insert(entity, Velocity(next));
}

#[inline]
fn is_directly_controlled_body(world: &World, entity: EntityId) -> bool {
    world.get::<CharacterMotor>(entity).is_some()
}

fn report_from_dto(
    report: PhysicsStepReportDto,
    events: Vec<newengine_physics_api::PhysicsEventDto>,
    key_to_entity: &BTreeMap<u64, EntityId>,
) -> PhysicsStepReport {
    let mut converted_events = Vec::new();
    for event in events {
        match event {
            newengine_physics_api::PhysicsEventDto::BodyCreated { entity } => {
                if let Some(entity) = key_to_entity.get(&entity).copied() {
                    converted_events.push(PhysicsEvent::BodyCreated { entity });
                }
            }
            newengine_physics_api::PhysicsEventDto::BodyDestroyed { entity } => {
                if let Some(entity) = key_to_entity.get(&entity).copied() {
                    converted_events.push(PhysicsEvent::BodyDestroyed { entity });
                }
            }
            _ => {}
        }
    }

    PhysicsStepReport {
        fixed_tick: report.fixed_tick,
        dt: report.dt,
        substeps: report.substeps,
        active_bodies: report.active_bodies,
        static_bodies: report.static_bodies,
        dynamic_bodies: report.dynamic_bodies,
        contacts: report.contacts,
        commands_applied: report.commands_applied,
        events: converted_events,
    }
}
