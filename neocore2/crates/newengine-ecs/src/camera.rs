#![forbid(unsafe_op_in_unsafe_fn)]

//! Deterministic camera components + update pipeline for NewEngine ECS.
//!
//! Goals:
//! - zero hidden allocations per update
//! - deterministic (no RNG; pure functions of (state,input,dt))
//! - composition over inheritance: behavior + modifiers
//! - ECS-friendly: camera is just components; system operates on them
//!
//! This module intentionally ships with minimal math types to keep `newengine-ecs` lightweight.
//! If your engine already has math types, you can convert at the boundary.

use crate::{EntityId, World};

// -----------------------------
// Minimal math
// -----------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}
impl Vec3 {
    #[inline]
    pub const fn new(x: f32, y: f32, z: f32) -> Self { Self { x, y, z } }
    #[inline]
    pub const fn zero() -> Self { Self { x: 0.0, y: 0.0, z: 0.0 } }
    #[inline]
    pub fn add(self, b: Self) -> Self { Self::new(self.x + b.x, self.y + b.y, self.z + b.z) }
    #[inline]
    pub fn sub(self, b: Self) -> Self { Self::new(self.x - b.x, self.y - b.y, self.z - b.z) }
    #[inline]
    pub fn mul(self, s: f32) -> Self { Self::new(self.x * s, self.y * s, self.z * s) }
    #[inline]
    pub fn dot(self, b: Self) -> f32 { self.x * b.x + self.y * b.y + self.z * b.z }
    #[inline]
    pub fn len2(self) -> f32 { self.dot(self) }
    #[inline]
    pub fn len(self) -> f32 { self.len2().sqrt() }
    #[inline]
    pub fn normalized(self) -> Self {
        let l = self.len();
        if l > 0.0 { self.mul(1.0 / l) } else { self }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Quat {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}
impl Quat {
    #[inline]
    pub const fn identity() -> Self { Self { x: 0.0, y: 0.0, z: 0.0, w: 1.0 } }

    /// From yaw (around +Y) and pitch (around +X). Roll is 0.
    #[inline]
    pub fn from_yaw_pitch(yaw: f32, pitch: f32) -> Self {
        // yaw-pitch order: q = qyaw * qpitch
        let (sy, cy) = (0.5 * yaw).sin_cos();
        let (sp, cp) = (0.5 * pitch).sin_cos();
        // qyaw (0, sy, 0, cy)
        // qpitch (sp,0,0,cp)
        // multiply:
        Self {
            x: sp * cy,
            y: sy * cp,
            z: -sy * sp,
            w: cy * cp,
        }
    }
}

#[inline]
fn clamp(x: f32, lo: f32, hi: f32) -> f32 { x.max(lo).min(hi) }

// -----------------------------
// Camera components
// -----------------------------

/// Output state for renderer.
#[derive(Clone, Copy, Debug)]
pub struct CameraState {
    pub position: Vec3,
    pub rotation: Quat,
    pub fov_y_radians: f32,
    pub near: f32,
    pub far: f32,
}

impl Default for CameraState {
    fn default() -> Self {
        Self {
            position: Vec3::zero(),
            rotation: Quat::identity(),
            fov_y_radians: 60.0_f32.to_radians(),
            near: 0.05,
            far: 10_000.0,
        }
    }
}

/// A camera entity marker + the editable configuration.
#[derive(Clone, Copy, Debug)]
pub struct CameraRig {
    pub kind: CameraKind,
    pub target: Option<EntityId>,
    pub priority: i32,
    pub is_active: bool,
}

impl Default for CameraRig {
    fn default() -> Self {
        Self {
            kind: CameraKind::EditorOrbit,
            target: None,
            priority: 0,
            is_active: true,
        }
    }
}

/// Camera configuration that designers expect to tweak.
#[derive(Clone, Copy, Debug)]
pub struct CameraParams {
    pub min_pitch: f32,
    pub max_pitch: f32,
    pub min_radius: f32,
    pub max_radius: f32,
    pub orbit_sensitivity: f32,
    pub zoom_sensitivity: f32,
    /// Critically damped smoothing frequency (Hz-like). 0 = no smoothing.
    pub smooth_freq: f32,
}
impl Default for CameraParams {
    fn default() -> Self {
        Self {
            min_pitch: (-89.0_f32).to_radians(),
            max_pitch: (89.0_f32).to_radians(),
            min_radius: 0.25,
            max_radius: 250.0,
            orbit_sensitivity: 0.010,
            zoom_sensitivity: 0.15,
            smooth_freq: 10.0,
        }
    }
}

/// Per-frame user input for camera. Put into a resource, updated by platform/input module.
#[derive(Clone, Copy, Debug, Default)]
pub struct CameraInput {
    pub dt: f32,

    // Editor orbit inputs
    pub mouse_delta: (f32, f32), // pixels or normalized; sensitivity handles scale
    pub orbit_held: bool,        // e.g. MMB

    pub scroll_delta: f32,       // wheel: + = zoom in (convention)
    pub pan_delta: (f32, f32),   // pixels; e.g. Shift+MMB
    pub pan_held: bool,

    // Optional: WASD fly (can be reused later)
    pub move_axis: (f32, f32, f32), // x,y,z in local space
    pub boost_held: bool,
}

/// Stores orbit state (yaw/pitch/radius + target point). Editable and serializable.
#[derive(Clone, Copy, Debug)]
pub struct OrbitController {
    pub yaw: f32,
    pub pitch: f32,
    pub radius: f32,
    pub pivot: Vec3,        // world-space pivot point
    pub pivot_vel: Vec3,    // for smoothing
    pub radius_vel: f32,    // for smoothing
    pub yaw_vel: f32,       // for smoothing
    pub pitch_vel: f32,     // for smoothing
}

impl Default for OrbitController {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.35,
            radius: 6.0,
            pivot: Vec3::zero(),
            pivot_vel: Vec3::zero(),
            radius_vel: 0.0,
            yaw_vel: 0.0,
            pitch_vel: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum CameraKind {
    EditorOrbit,
    GameplayFollow, // reserved
    FreeFly,        // reserved
}

// -----------------------------
// Deterministic smoothing
// -----------------------------

/// Critically damped spring (scalar).
/// Deterministic and stable for varying dt.
#[inline]
fn spring_crit_f32(x: &mut f32, v: &mut f32, target: f32, freq: f32, dt: f32) {
    if freq <= 0.0 || dt <= 0.0 {
        *x = target;
        *v = 0.0;
        return;
    }
    // See: "Critically Damped Ease-In/Ease-Out Smoothing" style update (stable)
    let omega = 2.0 * core::f32::consts::PI * freq;
    let f = 1.0 + 2.0 * dt * omega;
    let oo = omega * omega;
    let dt_oo = dt * oo;
    let inv = 1.0 / (f + dt_oo * dt);
    let dx = *x - target;
    let new_x = (f * (*x) + dt * (*v) - dt_oo * dt * target) * inv;
    let new_v = (*v - dt_oo * dx) * inv;
    *x = new_x;
    *v = new_v;
}

#[inline]
fn spring_crit_vec3(x: &mut Vec3, v: &mut Vec3, target: Vec3, freq: f32, dt: f32) {
    let mut xx = x.x;
    let mut vx = v.x;
    spring_crit_f32(&mut xx, &mut vx, target.x, freq, dt);
    x.x = xx;
    v.x = vx;
    let mut xy = x.y;
    let mut vy = v.y;
    spring_crit_f32(&mut xy, &mut vy, target.y, freq, dt);
    x.y = xy;
    v.y = vy;
    let mut xz = x.z;
    let mut vz = v.z;
    spring_crit_f32(&mut xz, &mut vz, target.z, freq, dt);
    x.z = xz;
    v.z = vz;
}

// -----------------------------
// System
// -----------------------------

/// Updates all active cameras. The "winner" (highest priority active rig) can be written into a resource
/// by your engine layer, or you can render all and pick externally.
///
/// Requirements:
/// - Each camera entity should have: `CameraRig`, `CameraParams`, `OrbitController`, `CameraState`.
pub fn update_cameras(world: &mut World, input: CameraInput) {
    // For MVP we only implement EditorOrbit. Others can be composed later.
    // To avoid aliasing rules for mutable multi-component iteration we do 2-phase: ids then mutate.
    let ids: Vec<_> = world.query2_ids::<CameraRig, CameraState>().collect();

    for id in ids {
        let rig = match world.get::<CameraRig>(id).copied() {
            Some(r) if r.is_active => r,
            _ => continue,
        };

        match rig.kind {
            CameraKind::EditorOrbit => update_orbit_camera(world, id, input),
            _ => {}
        }
    }
}

#[inline]
fn update_orbit_camera(world: &mut World, cam: EntityId, input: CameraInput) {
    let params = world.get::<CameraParams>(cam).copied().unwrap_or_default();
    let rig = world.get::<CameraRig>(cam).copied().unwrap_or_default();

    // Fetch components mutably with minimal borrow spans.
    let mut ctrl = world.get::<OrbitController>(cam).copied().unwrap_or_default();

    // 1) Resolve target pivot (can be external system; here: if target has a Transform-like component in your engine, set it here).
    // In this crate we keep it explicit: if target exists and there's a `CameraPivot` component, we use it.
    if let Some(t) = rig.target {
        if let Some(p) = world.get::<CameraPivot>(t).copied() {
            ctrl.pivot = p.0;
        }
    }

    // 2) Apply user input to desired values (deterministic, no smoothing yet)
    let mut desired_yaw = ctrl.yaw;
    let mut desired_pitch = ctrl.pitch;
    let mut desired_radius = ctrl.radius;
    let mut desired_pivot = ctrl.pivot;

    if input.orbit_held {
        desired_yaw += input.mouse_delta.0 * params.orbit_sensitivity;
        desired_pitch += input.mouse_delta.1 * params.orbit_sensitivity;
    }

    if input.scroll_delta != 0.0 {
        // exponential-ish zoom feel while staying deterministic
        let factor = (1.0 - input.scroll_delta * params.zoom_sensitivity).max(0.05);
        desired_radius *= factor;
    }

    if input.pan_held {
        // Pan in world axes for MVP (engine layer can convert to view-space pan).
        desired_pivot = desired_pivot.add(Vec3::new(-input.pan_delta.0, input.pan_delta.1, 0.0).mul(0.01));
    }

    desired_pitch = clamp(desired_pitch, params.min_pitch, params.max_pitch);
    desired_radius = clamp(desired_radius, params.min_radius, params.max_radius);

    // 3) Smooth (critically damped) toward desired values
    let dt = input.dt;
    let freq = params.smooth_freq;

    spring_crit_f32(&mut ctrl.yaw, &mut ctrl.yaw_vel, desired_yaw, freq, dt);
    spring_crit_f32(&mut ctrl.pitch, &mut ctrl.pitch_vel, desired_pitch, freq, dt);
    spring_crit_f32(&mut ctrl.radius, &mut ctrl.radius_vel, desired_radius, freq, dt);
    spring_crit_vec3(&mut ctrl.pivot, &mut ctrl.pivot_vel, desired_pivot, freq, dt);

    // 4) Build final camera state
    let rot = Quat::from_yaw_pitch(ctrl.yaw, ctrl.pitch);

    // Orbit camera position = pivot - forward * radius. With our minimal math, approximate forward from yaw/pitch.
    let cy = ctrl.yaw.cos();
    let sy = ctrl.yaw.sin();
    let cp = ctrl.pitch.cos();
    let sp = ctrl.pitch.sin();
    // Forward in RHS (x right, y up, z forward): forward = (sy*cp, -sp, cy*cp)
    let forward = Vec3::new(sy * cp, -sp, cy * cp).normalized();
    let pos = ctrl.pivot.sub(forward.mul(ctrl.radius));

    // Write back camera state and controller.
    if let Some(s) = world.get_mut::<CameraState>(cam) {
        s.position = pos;
        s.rotation = rot;
    } else {
        let _ = world.insert(cam, CameraState { position: pos, rotation: rot, ..Default::default() });
    }
    let _ = world.insert(cam, ctrl);
}

/// Optional: attach to any entity you want camera to track as its pivot.
#[derive(Clone, Copy, Debug, Default)]
pub struct CameraPivot(pub Vec3);

/// Helper: spawns an "ideal" editor orbit camera entity with all required components.
pub fn spawn_editor_orbit_camera(world: &mut World) -> EntityId {
    let e = world.spawn();
    let _ = world.insert(e, CameraRig::default());
    let _ = world.insert(e, CameraParams::default());
    let _ = world.insert(e, OrbitController::default());
    let _ = world.insert(e, CameraState::default());
    e
}
