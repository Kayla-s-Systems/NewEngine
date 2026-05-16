use newengine_bounds::Aabb;
use newengine_math::{Quat, Vec3};
use newengine_physics_api::{CollisionShapeDto, PhysicsBodyKindDto};
use newengine_physics_contracts::{CollisionShapeDesc, PhysicsBodyDesc, PhysicsBodyKind};

#[inline]
pub(super) fn body_kind_to_dto(kind: PhysicsBodyKind) -> PhysicsBodyKindDto {
    match kind {
        PhysicsBodyKind::Static => PhysicsBodyKindDto::Static,
        PhysicsBodyKind::Dynamic => PhysicsBodyKindDto::Dynamic,
        PhysicsBodyKind::Kinematic => PhysicsBodyKindDto::Kinematic,
    }
}

#[inline]
pub(super) fn shape_to_dto(shape: CollisionShapeDesc) -> CollisionShapeDto {
    match shape {
        CollisionShapeDesc::Box { half_extents } => CollisionShapeDto::Box { half_extents },
        CollisionShapeDesc::Sphere { radius } => CollisionShapeDto::Sphere { radius },
        CollisionShapeDesc::Capsule { radius, half_height } => CollisionShapeDto::Capsule { radius, half_height },
    }
}

pub(super) fn translated_shape_aabb(body: PhysicsBodyDesc, position: Vec3) -> Aabb {
    let local = body.shape.local_aabb();
    Aabb::new(local.min + position, local.max + position)
}

#[inline]
pub(super) fn vec3_to_arr(v: Vec3) -> [f32; 3] { [v.x, v.y, v.z] }

#[inline]
pub(super) fn arr_to_vec3(v: [f32; 3]) -> Vec3 { Vec3::new(v[0], v[1], v[2]) }

#[inline]
pub(super) fn quat_to_arr(q: Quat) -> [f32; 4] { [q.x, q.y, q.z, q.w] }

#[inline]
pub(super) fn arr_to_quat(q: [f32; 4]) -> Quat { Quat::from_xyzw(q[0], q[1], q[2], q[3]).normalize_or_identity() }
