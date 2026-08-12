use super::*;

pub(super) fn apply_pose_update(
    world: &mut World,
    key_to_entity: &BTreeMap<u64, EntityId>,
    update: PhysicsBodyPoseUpdate,
) {
    let Some(entity) = key_to_entity.get(&update.entity).copied() else {
        return;
    };
    let controlled_body = is_directly_controlled_body(world, entity);
    if let Some(transform) = world.get_mut::<Transform>(entity) {
        transform.position = arr_to_vec3(update.position);
        if !controlled_body {
            transform.rotation = arr_to_quat(update.rotation);
        }
    }
}

pub(super) fn apply_velocity_update(
    world: &mut World,
    key_to_entity: &BTreeMap<u64, EntityId>,
    update: PhysicsBodyVelocityUpdate,
) {
    let Some(entity) = key_to_entity.get(&update.entity).copied() else {
        return;
    };
    let physics_velocity = arr_to_vec3(update.linear_velocity);
    let next = if is_directly_controlled_body(world, entity) {
        let current = world.get::<Velocity>(entity).copied().unwrap_or_default().0;
        // Character motor owns lateral velocity and look/yaw. Physics owns vertical
        // resolution/gravity. This prevents the backend from erasing WASD and
        // mouse look while still applying floor contacts.
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

pub(super) fn contact_from_dto(
    contact: newengine_physics_api::PhysicsContactEventDto,
    key_to_entity: &BTreeMap<u64, EntityId>,
) -> Option<PhysicsContactEvent> {
    let a = key_to_entity.get(&contact.a).copied()?;
    let b = key_to_entity.get(&contact.b).copied()?;
    Some(PhysicsContactEvent {
        a: a.into(),
        b: b.into(),
        point: arr_to_vec3(contact.point),
        normal: arr_to_vec3(contact.normal),
        impulse: if contact.impulse.is_finite() {
            contact.impulse.max(0.0)
        } else {
            0.0
        },
    })
}
