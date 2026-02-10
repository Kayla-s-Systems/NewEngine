#![forbid(unsafe_op_in_unsafe_fn)]

use std::mem::MaybeUninit;

use glam::{Quat, Vec3};
use joltc_sys as sys;
use newengine_ecs::EntityId;
use newengine_physics_jolt::PhysicsWorld;
use newengine_transform::Transform;
use slotmap::Key;

use super::types::{Collider, RigidBody, RigidBodyKind};

#[inline]
pub fn stable_entity_key(id: EntityId) -> u64 {
    id.data().as_ffi()
}

#[inline]
pub fn jpc_vec3(v: Vec3) -> sys::JPC_Vec3 {
    sys::JPC_Vec3 { x: v.x, y: v.y, z: v.z, _w: 0.0 }
}

#[inline]
pub fn jpc_rvec3(v: Vec3) -> sys::JPC_RVec3 {
    sys::JPC_RVec3 { x: v.x, y: v.y, z: v.z, _w: 0.0 }
}

#[inline]
pub fn jpc_quat(q: Quat) -> sys::JPC_Quat {
    sys::JPC_Quat { x: q.x, y: q.y, z: q.z, w: q.w }
}

pub fn jolt_create_body(
    phys: &mut PhysicsWorld,
    entity: EntityId,
    t: &Transform,
    rb: RigidBody,
    col: Collider,
) -> Option<sys::JPC_BodyID> {
    let system = phys.system_raw();

    let body_iface = unsafe { sys::JPC_PhysicsSystem_GetBodyInterface(system) };
    if body_iface.is_null() {
        return None;
    }

    let shape = match col {
        Collider::Sphere { radius } => jolt_create_sphere_shape(radius)?,
        Collider::Box { half_extents, convex_radius } => jolt_create_box_shape(half_extents, convex_radius)?,
    };

    let motion = match rb.kind {
        RigidBodyKind::Static => sys::JPC_MOTION_TYPE_STATIC,
        RigidBodyKind::Dynamic => sys::JPC_MOTION_TYPE_DYNAMIC,
        RigidBodyKind::Kinematic => sys::JPC_MOTION_TYPE_KINEMATIC,
    };

    let mut bcs_uninit = MaybeUninit::<sys::JPC_BodyCreationSettings>::uninit();
    unsafe { sys::JPC_BodyCreationSettings_default(bcs_uninit.as_mut_ptr()) };
    let mut bcs = unsafe { bcs_uninit.assume_init() };

    bcs.Position = jpc_rvec3(t.position);
    bcs.Rotation = jpc_quat(t.rotation);
    bcs.MotionType = motion;
    bcs.ObjectLayer = rb.object_layer;
    bcs.Shape = shape;
    bcs.UserData = stable_entity_key(entity);

    let activation = if rb.kind == RigidBodyKind::Dynamic {
        sys::JPC_ACTIVATION_ACTIVATE
    } else {
        sys::JPC_ACTIVATION_DONT_ACTIVATE
    };

    let body_id = unsafe { sys::JPC_BodyInterface_CreateAndAddBody(body_iface, &bcs, activation) };
    unsafe { sys::JPC_Shape_Release(shape) };

    Some(body_id)
}

fn jolt_create_sphere_shape(radius: f32) -> Option<*mut sys::JPC_Shape> {
    let mut ss_uninit = MaybeUninit::<sys::JPC_SphereShapeSettings>::uninit();
    unsafe { sys::JPC_SphereShapeSettings_default(ss_uninit.as_mut_ptr()) };
    let mut ss = unsafe { ss_uninit.assume_init() };
    ss.Radius = radius.abs().max(1.0e-4);

    let mut out_shape: *mut sys::JPC_Shape = core::ptr::null_mut();
    let mut err: *mut sys::JPC_String = core::ptr::null_mut();

    let ok = unsafe { sys::JPC_SphereShapeSettings_Create(&ss, &mut out_shape, &mut err) };
    if !err.is_null() {
        unsafe { sys::JPC_String_delete(err) };
    }
    if !ok || out_shape.is_null() {
        return None;
    }

    Some(out_shape)
}

fn jolt_create_box_shape(half_extents: Vec3, convex_radius: f32) -> Option<*mut sys::JPC_Shape> {
    let mut bs_uninit = MaybeUninit::<sys::JPC_BoxShapeSettings>::uninit();
    unsafe { sys::JPC_BoxShapeSettings_default(bs_uninit.as_mut_ptr()) };
    let mut bs = unsafe { bs_uninit.assume_init() };

    bs.HalfExtent = jpc_vec3(Vec3::new(
        half_extents.x.abs().max(1.0e-4),
        half_extents.y.abs().max(1.0e-4),
        half_extents.z.abs().max(1.0e-4),
    ));
    bs.ConvexRadius = convex_radius.max(0.0);

    let mut out_shape: *mut sys::JPC_Shape = core::ptr::null_mut();
    let mut err: *mut sys::JPC_String = core::ptr::null_mut();

    let ok = unsafe { sys::JPC_BoxShapeSettings_Create(&bs, &mut out_shape, &mut err) };
    if !err.is_null() {
        unsafe { sys::JPC_String_delete(err) };
    }
    if !ok || out_shape.is_null() {
        return None;
    }

    Some(out_shape)
}