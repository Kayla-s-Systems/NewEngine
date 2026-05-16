use newengine_ecs::{Component, EntityId, World};
use newengine_math::collections::FxHashSet;
use newengine_sim::{
    AngularVelocity, CameraRigComp, CharacterMotor, FollowTargetCameraController,
    FollowTargetCameraMotor, MotorInput, Velocity,
};
use newengine_transform::Transform;

use super::{PhysicsBodyDesc, DisplayVisibility};

#[derive(Clone, Copy, Debug)]
pub struct RuntimeEntitySnapshot {
    pub entity: EntityId,
    pub transform: Option<Transform>,
    pub velocity: Option<Velocity>,
    pub angular_velocity: Option<AngularVelocity>,
    pub motor_input: Option<MotorInput>,
    pub character_motor: Option<CharacterMotor>,
    pub camera_rig: Option<CameraRigComp>,
    pub follow_controller: Option<FollowTargetCameraController>,
    pub follow_motor: Option<FollowTargetCameraMotor>,
    pub physics_body: Option<PhysicsBodyDesc>,
    pub display_visibility: Option<DisplayVisibility>,
}

#[derive(Clone, Debug, Default)]
pub struct RuntimeWorldSnapshot {
    pub entities: Vec<RuntimeEntitySnapshot>,
}

#[inline]
pub fn capture_runtime_world_snapshot(world: &World) -> RuntimeWorldSnapshot {
    let mut entities: Vec<RuntimeEntitySnapshot> = world
        .iter_entities()
        .map(|entity| RuntimeEntitySnapshot {
            entity,
            transform: world.get::<Transform>(entity).copied(),
            velocity: world.get::<Velocity>(entity).copied(),
            angular_velocity: world.get::<AngularVelocity>(entity).copied(),
            motor_input: world.get::<MotorInput>(entity).copied(),
            character_motor: world.get::<CharacterMotor>(entity).copied(),
            camera_rig: world.get::<CameraRigComp>(entity).copied(),
            follow_controller: world.get::<FollowTargetCameraController>(entity).copied(),
            follow_motor: world.get::<FollowTargetCameraMotor>(entity).copied(),
            physics_body: world.get::<PhysicsBodyDesc>(entity).copied(),
            display_visibility: world.get::<DisplayVisibility>(entity).copied(),
        })
        .collect();
    entities.sort_by_key(|it| it.entity.stable_u64());
    RuntimeWorldSnapshot { entities }
}

#[inline]
fn restore_component_opt<T: Component + Copy>(world: &mut World, entity: EntityId, value: Option<T>) {
    if let Some(v) = value {
        let _ = world.insert(entity, v);
    } else {
        let _ = world.remove::<T>(entity);
    }
}

#[inline]
pub fn restore_runtime_world_snapshot(world: &mut World, snapshot: RuntimeWorldSnapshot) {
    let live_ids: Vec<EntityId> = world.iter_entities().collect();
    let original_ids: FxHashSet<EntityId> = snapshot.entities.iter().map(|it| it.entity).collect();

    for entity in live_ids {
        if !original_ids.contains(&entity) {
            let _ = world.despawn(entity);
        }
    }

    for entry in snapshot.entities {
        if !world.exists(entry.entity) {
            continue;
        }

        restore_component_opt(world, entry.entity, entry.transform);
        restore_component_opt(world, entry.entity, entry.velocity);
        restore_component_opt(world, entry.entity, entry.angular_velocity);
        restore_component_opt(world, entry.entity, entry.motor_input);
        restore_component_opt(world, entry.entity, entry.character_motor);
        restore_component_opt(world, entry.entity, entry.camera_rig);
        restore_component_opt(world, entry.entity, entry.follow_controller);
        restore_component_opt(world, entry.entity, entry.follow_motor);
        restore_component_opt(world, entry.entity, entry.physics_body);
        restore_component_opt(world, entry.entity, entry.display_visibility);
    }
}
