#![forbid(unsafe_op_in_unsafe_fn)]

use std::mem::MaybeUninit;
use std::sync::Mutex;

use glam::{Quat, Vec3};
use hashbrown::{HashMap, HashSet};

use joltc_sys as sys;
use newengine_ecs::{EntityId, World};
use newengine_physics_jolt::{JoltInitDesc, PhysicsWorld};
use newengine_transform::Transform;
use slotmap::Key;

/// Physics singleton stored inside ECS.
///
/// The simulation layer owns the physics world so both editor and game can reuse the same pipeline.
pub struct PhysicsCtx {
    world: Mutex<PhysicsWorld>,
    /// Best-effort map to delete orphaned bodies deterministically.
    /// Key: stable entity key.
    bodies: HashMap<u64, sys::JPC_BodyID>,
}

impl PhysicsCtx {
    #[inline]
    pub fn new(desc: JoltInitDesc) -> Result<Self, newengine_physics_jolt::PhysicsError> {
        Ok(Self {
            world: Mutex::new(PhysicsWorld::new(desc)?),
            bodies: HashMap::new(),
        })
    }
}

/// Simulation stepping settings.
///
/// This implements a UE-like fixed timestep with sub-stepping and render interpolation.
#[derive(Clone, Copy, Debug)]
pub struct PhysicsSettings {
    /// Fixed simulation dt (seconds).
    pub fixed_dt: f32,
    /// Upper bound on sub-steps per frame (prevents spiral-of-death).
    pub max_substeps: u32,
    /// Clamp for incoming frame dt (seconds).
    pub max_frame_dt: f32,
}

impl Default for PhysicsSettings {
    #[inline]
    fn default() -> Self {
        Self {
            fixed_dt: 1.0 / 60.0,
            max_substeps: 8,
            max_frame_dt: 0.25,
        }
    }
}

/// Runtime step state.
#[derive(Clone, Copy, Debug, Default)]
pub struct PhysicsStepState {
    accum: f32,
    /// Interpolation alpha in [0..1] after stepping.
    pub alpha: f32,
    /// Monotonic physics tick (increments per fixed step).
    pub tick: u64,
}

/// Rigidbody kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RigidBodyKind {
    Static,
    Dynamic,
    Kinematic,
}

impl Default for RigidBodyKind {
    #[inline]
    fn default() -> Self {
        Self::Dynamic
    }
}

/// Rigidbody component.
#[derive(Clone, Copy, Debug, Default)]
pub struct RigidBody {
    pub kind: RigidBodyKind,
    /// Object layer (Jolt). Kept explicit to avoid hard-coding engine-wide rules.
    pub object_layer: u16,
}

/// Collider component.
#[derive(Clone, Copy, Debug)]
pub enum Collider {
    Sphere { radius: f32 },
    Box {
        /// Half extents in local space.
        half_extents: Vec3,
        /// Convex radius (aka bevel). Use 0.0 for a sharp box.
        convex_radius: f32,
    },
}

impl Default for Collider {
    #[inline]
    fn default() -> Self {
        Self::Sphere { radius: 0.5 }
    }
}

/// Baked physics body handle stored on an entity.
///
/// This is intentionally opaque: higher layers should not depend on Jolt types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysicsBody {
    pub id: sys::JPC_BodyID,
}

/// Cached physics pose for interpolation.
///
/// The renderer and gameplay should read `Transform` (already interpolated) and never talk to Jolt.
#[derive(Clone, Copy, Debug, Default)]
pub struct PhysicsPose {
    pub prev_pos: Vec3,
    pub prev_rot: Quat,
    pub curr_pos: Vec3,
    pub curr_rot: Quat,
}

// -----------------------------------------------------------------------------
// Stable keys / determinism
// -----------------------------------------------------------------------------

#[inline]
fn stable_entity_key(id: EntityId) -> u64 {
    id.data().as_ffi()
}

// -----------------------------------------------------------------------------
// ECS utilities
// -----------------------------------------------------------------------------

#[inline]
fn ensure_physics_ctx(world: &mut World) -> bool {
    if world.resource::<PhysicsCtx>().is_some() {
        if world.resource::<PhysicsSettings>().is_none() {
            world.insert_resource(PhysicsSettings::default());
        }
        if world.resource::<PhysicsStepState>().is_none() {
            world.insert_resource(PhysicsStepState::default());
        }
        return true;
    }

    if world.resource::<PhysicsSettings>().is_none() {
        world.insert_resource(PhysicsSettings::default());
    }
    if world.resource::<PhysicsStepState>().is_none() {
        world.insert_resource(PhysicsStepState::default());
    }

    if let Ok(ctx) = PhysicsCtx::new(JoltInitDesc::default()) {
        world.insert_resource(ctx);
        true
    } else {
        false
    }
}

// -----------------------------------------------------------------------------
// Baking (ECS -> Jolt)
// -----------------------------------------------------------------------------

/// Creates missing Jolt bodies for entities that have `Transform` + `RigidBody` + `Collider`.
///
/// Determinism: iterate entities in stable-key order.
pub fn physics_bake_bodies(world: &mut World, _frame: super::SimFrame) {
    if !ensure_physics_ctx(world) {
        return;
    }

    // Snapshot all data first.
    let mut todo: Vec<(u64, EntityId, Transform, RigidBody, Collider)> = Vec::new();
    for id in world.query2_ids::<RigidBody, Collider>() {
        if world.get::<PhysicsBody>(id).is_some() {
            continue;
        }
        let Some(t) = world.get::<Transform>(id).copied() else { continue; };
        let Some(rb) = world.get::<RigidBody>(id).copied() else { continue; };
        let Some(col) = world.get::<Collider>(id).copied() else { continue; };
        todo.push((stable_entity_key(id), id, t, rb, col));
    }
    todo.sort_unstable_by_key(|(k, _, _, _, _)| *k);

    // Create bodies in Jolt under one ctx borrow + lock.
    let mut created: Vec<(u64, EntityId, sys::JPC_BodyID)> = Vec::new();
    {
        let ctx = world.resource_mut::<PhysicsCtx>().expect("physics ctx must exist");
        let mut pw_guard = ctx.world.lock().ok();
        let Some(pw) = pw_guard.as_deref_mut() else { return; };

        for (k, id, t, rb, col) in &todo {
            let Some(body_id) = (jolt_create_body(pw, *id, t, *rb, *col)) else {
                continue;
            };
            created.push((*k, *id, body_id));
        }
    }

    if created.is_empty() {
        return;
    }

    // Apply ECS components WITHOUT holding PhysicsCtx borrow.
    for &(_, id, body_id) in &created {
        let _ = world.insert(id, PhysicsBody { id: body_id });
    }

    // Update bookkeeping in PhysicsCtx with a separate borrow.
    {
        let ctx = world.resource_mut::<PhysicsCtx>().expect("physics ctx must exist");
        for &(k, _id, body_id) in &created {
            ctx.bodies.insert(k, body_id);
        }
    }
}

// -----------------------------------------------------------------------------
// Cleanup (orphan removal)
// -----------------------------------------------------------------------------

/// Removes Jolt bodies for entities that were despawned or lost `PhysicsBody`.
///
/// This keeps the physics world bounded and prevents leaked bodies across hot-reloads.
pub fn physics_cleanup_bodies(world: &mut World, _frame: super::SimFrame) {
    if !ensure_physics_ctx(world) {
        return;
    }

    let mut live: HashSet<u64> = HashSet::new();
    for (id, _) in world.query::<PhysicsBody>() {
        live.insert(stable_entity_key(id));
    }

    let mut to_remove: Vec<(u64, sys::JPC_BodyID)> = Vec::new();
    {
        let ctx = world.resource::<PhysicsCtx>().expect("physics ctx must exist");
        if ctx.bodies.is_empty() {
            return;
        }
        for (&k, &body_id) in ctx.bodies.iter() {
            if !live.contains(&k) {
                to_remove.push((k, body_id));
            }
        }
    }

    if to_remove.is_empty() {
        return;
    }
    to_remove.sort_unstable_by_key(|(k, _)| *k);

    {
        let ctx = world.resource_mut::<PhysicsCtx>().expect("physics ctx must exist");
        let mut pw_guard = ctx.world.lock().ok();
        let Some(pw) = pw_guard.as_deref_mut() else { return; };

        let system = pw.system_raw();
        let body_iface = unsafe { sys::JPC_PhysicsSystem_GetBodyInterface(system) };
        if body_iface.is_null() {
            return;
        }

        for (k, body_id) in to_remove {
            ctx.bodies.remove(&k);
            unsafe {
                sys::JPC_BodyInterface_RemoveBody(body_iface, body_id);
                sys::JPC_BodyInterface_DestroyBody(body_iface, body_id);
            }
        }
    }
}

// -----------------------------------------------------------------------------
// Stepping
// -----------------------------------------------------------------------------

/// Advances the Jolt simulation.
///
/// Implements fixed timestep sub-stepping and produces an interpolation alpha.
pub fn physics_step_jolt(world: &mut World, frame: super::SimFrame) {
    if !frame.dt.is_finite() || frame.dt <= 0.0 {
        return;
    }
    if !ensure_physics_ctx(world) {
        return;
    }

    let settings = *world
        .resource::<PhysicsSettings>()
        .unwrap_or(&PhysicsSettings::default());

    let dt = frame.dt.min(settings.max_frame_dt);

    // Update accumulator first (short mutable borrow).
    let (mut accum, mut tick) = {
        let state = world
            .resource_mut::<PhysicsStepState>()
            .expect("physics step state must exist");
        state.accum = (state.accum + dt).max(0.0);
        (state.accum, state.tick)
    };

    // Snapshot kinematic/static targets once per frame (no locks held).
    let kin_targets = gather_kinematic_targets(world);

    let mut steps: u32 = 0;

    // Step physics under a dedicated scope so the lock + ctx borrow ends BEFORE we borrow world mutably again.
    {
        let mut pw_guard = {
            let ctx = world.resource::<PhysicsCtx>().expect("physics ctx must exist");
            ctx.world.lock().ok()
        };
        let Some(mut pw_guard) = pw_guard else { return; };
        let pw = &mut *pw_guard;

        while accum + 1.0e-6 >= settings.fixed_dt && steps < settings.max_substeps {
            apply_kinematic_targets_locked(pw, &kin_targets);

            if pw.step(settings.fixed_dt).is_err() {
                break;
            }

            accum -= settings.fixed_dt;
            tick = tick.wrapping_add(1);
            steps += 1;
        }
        // pw_guard dropped here
    }

    let alpha = if settings.fixed_dt > 0.0 {
        (accum / settings.fixed_dt).clamp(0.0, 1.0)
    } else {
        0.0
    };

    // Now it's safe: no outstanding borrow from PhysicsCtx lock.
    let state = world
        .resource_mut::<PhysicsStepState>()
        .expect("physics step state must exist");
    state.accum = accum;
    state.alpha = alpha;
    state.tick = tick;
}


// -----------------------------------------------------------------------------
// Sync back (Jolt -> ECS)
// -----------------------------------------------------------------------------

/// Writes back Jolt transforms into ECS `Transform` for dynamic bodies.
///
/// Kinematic/static bodies are driven by ECS transforms and pushed into Jolt during stepping.
pub fn physics_sync_transforms(world: &mut World, _frame: super::SimFrame) {
    if !ensure_physics_ctx(world) {
        return;
    }

    let alpha = world
        .resource::<PhysicsStepState>()
        .map(|s| s.alpha)
        .unwrap_or(1.0);

    let mut dyn_read: Vec<(u64, EntityId, sys::JPC_BodyID)> = Vec::new();
    for (id, pb) in world.query::<PhysicsBody>() {
        let Some(rb) = world.get::<RigidBody>(id).copied() else { continue; };
        if rb.kind != RigidBodyKind::Dynamic {
            continue;
        }
        dyn_read.push((stable_entity_key(id), id, pb.id));
    }
    dyn_read.sort_unstable_by_key(|(k, _, _)| *k);

    let mut dyn_out: Vec<(EntityId, Vec3, Quat)> = Vec::new();
    {
        let ctx = world.resource_mut::<PhysicsCtx>().expect("physics ctx must exist");
        let mut pw_guard = ctx.world.lock().ok();
        let Some(pw) = pw_guard.as_deref_mut() else { return; };

        let system = pw.system_raw();
        let body_iface = unsafe { sys::JPC_PhysicsSystem_GetBodyInterface(system) };
        if body_iface.is_null() {
            return;
        }

        for (_, id, body_id) in dyn_read {
            let mut pos = sys::JPC_RVec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                _w: 0.0,
            };
            let mut rot = sys::JPC_Quat {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            };

            unsafe {
                sys::JPC_BodyInterface_GetPositionAndRotation(
                    body_iface,
                    body_id,
                    &mut pos,
                    &mut rot,
                );
            }

            dyn_out.push((
                id,
                Vec3::new(pos.x, pos.y, pos.z),
                Quat::from_xyzw(rot.x, rot.y, rot.z, rot.w),
            ));
        }
    }

    for (id, p, r) in dyn_out {
        let pose = match world.get::<PhysicsPose>(id).copied() {
            Some(mut pose) => {
                pose.prev_pos = pose.curr_pos;
                pose.prev_rot = pose.curr_rot;
                pose.curr_pos = p;
                pose.curr_rot = r;
                pose
            }
            None => PhysicsPose {
                prev_pos: p,
                prev_rot: r,
                curr_pos: p,
                curr_rot: r,
            },
        };
        let _ = world.insert(id, pose);

        if let Some(t) = world.get_mut::<Transform>(id) {
            t.position = pose.prev_pos.lerp(pose.curr_pos, alpha);
            t.rotation = pose.prev_rot.slerp(pose.curr_rot, alpha).normalize();
        }
    }
}

// -----------------------------------------------------------------------------
// Kinematic push (ECS -> Jolt)
// -----------------------------------------------------------------------------

#[inline]
fn gather_kinematic_targets(world: &World) -> Vec<(u64, sys::JPC_BodyID, Transform)> {
    let mut out: Vec<(u64, sys::JPC_BodyID, Transform)> = Vec::new();
    for (id, pb) in world.query::<PhysicsBody>() {
        let Some(rb) = world.get::<RigidBody>(id).copied() else { continue; };
        if rb.kind == RigidBodyKind::Dynamic {
            continue;
        }
        let Some(t) = world.get::<Transform>(id).copied() else { continue; };
        out.push((stable_entity_key(id), pb.id, t));
    }
    out.sort_unstable_by_key(|(k, _, _)| *k);
    out
}

#[inline]
fn apply_kinematic_targets_locked(
    pw: &mut PhysicsWorld,
    targets: &[(u64, sys::JPC_BodyID, Transform)],
) {
    if targets.is_empty() {
        return;
    }

    let system = pw.system_raw();
    let body_iface = unsafe { sys::JPC_PhysicsSystem_GetBodyInterface(system) };
    if body_iface.is_null() {
        return;
    }

    for &(_, body_id, t) in targets {
        let pos = jpc_rvec3(t.position);
        let rot = jpc_quat(t.rotation);
        unsafe {
            sys::JPC_BodyInterface_SetPositionAndRotation(
                body_iface,
                body_id,
                pos,
                rot,
                sys::JPC_ACTIVATION_DONT_ACTIVATE,
            );
        }
    }
}

// -----------------------------------------------------------------------------
// Jolt helpers
// -----------------------------------------------------------------------------

#[inline]
fn jpc_vec3(v: Vec3) -> sys::JPC_Vec3 {
    sys::JPC_Vec3 {
        x: v.x,
        y: v.y,
        z: v.z,
        _w: 0.0,
    }
}

#[inline]
fn jpc_rvec3(v: Vec3) -> sys::JPC_RVec3 {
    sys::JPC_RVec3 {
        x: v.x,
        y: v.y,
        z: v.z,
        _w: 0.0,
    }
}

#[inline]
fn jpc_quat(q: Quat) -> sys::JPC_Quat {
    sys::JPC_Quat {
        x: q.x,
        y: q.y,
        z: q.z,
        w: q.w,
    }
}

fn jolt_create_body(
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
        Collider::Box {
            half_extents,
            convex_radius,
        } => jolt_create_box_shape(half_extents, convex_radius)?,
    };

    let motion = match rb.kind {
        RigidBodyKind::Static => sys::JPC_MOTION_TYPE_STATIC,
        RigidBodyKind::Dynamic => sys::JPC_MOTION_TYPE_DYNAMIC,
        RigidBodyKind::Kinematic => sys::JPC_MOTION_TYPE_KINEMATIC,
    };

    let mut bcs_uninit = MaybeUninit::<sys::JPC_BodyCreationSettings>::uninit();
    unsafe {
        sys::JPC_BodyCreationSettings_default(bcs_uninit.as_mut_ptr());
    }
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
    unsafe {
        sys::JPC_SphereShapeSettings_default(ss_uninit.as_mut_ptr());
    }
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
    unsafe {
        sys::JPC_BoxShapeSettings_default(bs_uninit.as_mut_ptr());
    }
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