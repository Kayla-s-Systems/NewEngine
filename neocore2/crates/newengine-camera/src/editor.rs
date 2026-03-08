#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::{Quat, Vec2, Vec3};

use crate::{
    auto_near_far, default_perspective, frame_orbit_to_sphere, CameraController, CameraInput,
    CameraMatrices, CameraRig, CameraState, Frustum, OrbitController, Projection,
};

/// Axis-aligned bounding box.
#[derive(Clone, Copy, Debug, Default)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    #[inline]
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    #[inline]
    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    #[inline]
    pub fn extents(&self) -> Vec3 {
        (self.max - self.min) * 0.5
    }

    /// Bounding sphere radius around the AABB center.
    #[inline]
    pub fn radius(&self) -> f32 {
        self.extents().length().max(1e-6)
    }
}

/// Editor-grade camera wrapper around `CameraState`.
///
/// Goals:
/// - deterministic orbit navigation
/// - reliable framing (`frame_all`, `focus`) without clipping
/// - robust near/far handling (good depth precision)
#[derive(Clone, Debug)]
pub struct EditorCamera {
    pub state: CameraState,
    pub orbit: OrbitController,
    pub margin: f32,
    pub focus_radius: f32,
    was_look_active: bool,
}

impl Default for EditorCamera {
    #[inline]
    fn default() -> Self {
        let mut state = CameraState::default();
        state.controller = CameraController::None;

        let orbit = OrbitController {
            // Blender-ish defaults.
            yaw: 0.65,
            pitch: -0.55,
            distance: 6.0,
            ..OrbitController::default()
        };

        Self {
            state,
            orbit,
            margin: 1.08,
            focus_radius: 1.0,
            was_look_active: false,
        }
    }
}

impl EditorCamera {
    /// Creates a camera configured for editor usage.
    ///
    /// `viewport_aspect` must be `width / height` in **physical pixels**.
    #[inline]
    pub fn new(viewport_aspect: f32) -> Self {
        let mut this = Self::default();
        this.state.projection = default_perspective(viewport_aspect.max(1e-6));
        this.set_viewport_aspect(viewport_aspect);
        this
    }

    #[inline]
    pub fn rig(&self) -> &CameraRig {
        &self.state.rig
    }

    #[inline]
    pub fn rig_mut(&mut self) -> &mut CameraRig {
        &mut self.state.rig
    }

    #[inline]
    pub fn projection(&self) -> Projection {
        self.state.projection
    }

    #[inline]
    pub fn set_viewport(&mut self, width: u32, height: u32) {
        self.state.set_viewport(width, height);
        let w = width.max(1) as f32;
        let h = height.max(1) as f32;
        self.set_viewport_aspect(w / h);
    }

    #[inline]
    pub fn set_viewport_aspect(&mut self, aspect: f32) {
        match &mut self.state.projection {
            Projection::Perspective(p) => p.aspect = aspect.max(1e-6),
            Projection::Orthographic(o) => o.aspect = aspect.max(1e-6),
        }
    }

    /// Updates orbit controller, writes rig, computes matrices + frustum.
    #[inline]
    pub fn update(&mut self, input: Option<CameraInput>, dt: f32) -> (CameraMatrices, Frustum) {
        if let Some(mut i) = input {
            // RMB (or any look activation) must not teleport the camera.
            // If the rig has been modified externally (framing, switching controllers, etc.),
            // stale orbit state would reconstruct `rig.position` from `(target, distance)`.
            // Sync exactly on the activation edge and suppress the first-frame delta (cursor grab
            // frequently produces a synthetic large delta).
            if i.look_active && !self.was_look_active {
                self.orbit.sync_from_rig(&self.state.rig);
                i.look_delta = Vec2::ZERO;
            }
            self.was_look_active = i.look_active;

            self.orbit.apply(&mut self.state.rig, i, dt);
        } else {
            // Still must refresh rig from orbit (for cases when orbit was modified directly).
            self.was_look_active = false;
            self.orbit.apply(
                &mut self.state.rig,
                CameraInput {
                    look_active: false,
                    look_delta: Vec2::ZERO,
                    move_axis: Vec3::ZERO,
                    speed_mul: 1.0,
                    zoom_delta: 0.0,
                },
                0.0,
            );
        }

        // Keep near/far stable while orbiting.
        let (near, far) = auto_near_far(self.orbit.distance, self.focus_radius);
        match &mut self.state.projection {
            Projection::Perspective(p) => {
                p.near = near;
                p.far = far.max(p.near + 0.1);
            }
            Projection::Orthographic(o) => {
                o.near = near;
                o.far = far.max(o.near + 0.1);
            }
        }

        self.state.update(None, dt)
    }

    /// Frames the camera to a world-space sphere.
    #[inline]
    pub fn frame_sphere(&mut self, center: Vec3, radius: f32) {
        self.focus_radius = radius.abs().max(1e-6);
        let aspect = match self.state.projection {
            Projection::Perspective(p) => p.aspect,
            Projection::Orthographic(o) => o.aspect,
        };

        frame_orbit_to_sphere(
            &mut self.orbit,
            &mut self.state.projection,
            aspect,
            center,
            self.focus_radius,
            self.margin.max(1.0),
        );

        // Apply immediately.
        self.orbit.apply(
            &mut self.state.rig,
            CameraInput {
                look_active: false,
                look_delta: Vec2::ZERO,
                move_axis: Vec3::ZERO,
                speed_mul: 1.0,
                zoom_delta: 0.0,
            },
            0.0,
        );
    }

    /// Frames the camera to an AABB.
    #[inline]
    pub fn frame_aabb(&mut self, aabb: Aabb) {
        self.frame_sphere(aabb.center(), aabb.radius());
    }
}

// -------------------------------------------------------------------------------------------------
// Editor navigation controller (Orbit / Fly) intended for apps (e.g. `apps/editor`).
//
// Design goals:
// - app code is a thin adapter: input mapping + parameter tuning
// - controller guarantees: mode switch / look activation must not teleport the rig
// - optional ground clamp policy (min camera Y)

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorNavMode {
    Orbit,
    Fly,
}

impl Default for EditorNavMode {
    #[inline]
    fn default() -> Self {
        Self::Orbit
    }
}

#[derive(Clone, Copy, Debug)]
pub struct EditorNavLimits {
    pub min_distance: f32,
    pub max_pitch_abs: f32,
    pub min_camera_y: f32,

    /// Maximum allowed look delta (pixels) per frame.
    /// Prevents cursor-warp / pointer-lock glitches from injecting huge impulses.
    pub max_look_delta_px: f32,

    /// Minimum accumulated look delta quantum (pixels) before it affects yaw/pitch.
    ///
    /// Purpose:
    /// - suppress backend/raw-input micro-jitter while the mouse is idle;
    /// - preserve very slow motion by accumulating sub-quantum residuals deterministically.
    pub look_delta_quantum_px: f32,
}

impl Default for EditorNavLimits {
    #[inline]
    fn default() -> Self {
        Self {
            min_distance: 0.30,
            max_pitch_abs: 1.5184364,
            min_camera_y: 0.10,
            max_look_delta_px: 160.0,
            look_delta_quantum_px: 1.0,
        }
    }
}

/// Editor navigation controller.
///
/// Stores deterministic controller state (Orbit + Fly) and applies it to a `CameraRig`.
#[derive(Clone, Copy, Debug)]
pub struct EditorNavController {
    pub mode: EditorNavMode,

    pub orbit: OrbitController,
    pub fly: crate::FreeFlyController,

    /// Base fly speed (units/sec). `CameraInput.speed_mul` can still multiply it.
    pub fly_speed: f32,

    pub limits: EditorNavLimits,

    was_look_active: bool,
    look_delta_residual_px: Vec2,
}

impl Default for EditorNavController {
    #[inline]
    fn default() -> Self {
        let mut orbit = OrbitController::default();
        orbit.yaw = 0.7853982;
        orbit.pitch = -0.55;
        orbit.distance = 6.0;

        let mut fly = crate::FreeFlyController::default();
        fly.yaw = 0.7853982;
        fly.pitch = -0.55;

        Self {
            mode: EditorNavMode::Orbit,
            orbit,
            fly,
            fly_speed: 2.0,
            limits: EditorNavLimits::default(),
            was_look_active: false,
            look_delta_residual_px: Vec2::ZERO,
        }
    }
}

impl EditorNavController {
    #[inline]
    fn quantize_axis_with_residual(v: &mut f32, quantum: f32) -> f32 {
        let q = quantum.max(1.0e-4);
        if !v.is_finite() {
            *v = 0.0;
            return 0.0;
        }

        if v.abs() < q {
            return 0.0;
        }

        let out = (*v / q).trunc() * q;
        *v -= out;
        out
    }

    #[inline]
    fn quantize_look_delta_with_residual(&mut self, look_delta: Vec2) -> Vec2 {
        let mut acc = self.look_delta_residual_px + look_delta;
        let q = self.limits.look_delta_quantum_px.max(1.0e-4);
        let out = Vec2::new(
            Self::quantize_axis_with_residual(&mut acc.x, q),
            Self::quantize_axis_with_residual(&mut acc.y, q),
        );
        self.look_delta_residual_px = acc;
        out
    }

    /// Switches navigation mode and synchronizes internal controller state from the current rig.
    ///
    /// This function does **not** modify the rig.
    #[inline]
    pub fn set_mode(&mut self, next: EditorNavMode, rig: &CameraRig) {
        if self.mode == next {
            return;
        }

        match next {
            EditorNavMode::Orbit => {
                self.orbit.distance = self
                    .orbit
                    .distance
                    .clamp(self.orbit.min_distance, self.orbit.max_distance);
                self.orbit.sync_from_rig(rig);
            }
            EditorNavMode::Fly => {
                self.fly.sync_from_rig(rig);
            }
        }

        self.mode = next;
        self.was_look_active = false;
        self.look_delta_residual_px = Vec2::ZERO;
    }

    /// Applies input to the rig for the current mode.
    ///
    /// Guarantees:
    /// - look activation edge (e.g. RMB grab) never teleports the camera
    /// - first-frame synthetic deltas (pointer lock) are suppressed
    #[inline]
    pub fn step(&mut self, rig: &mut CameraRig, mut input: CameraInput, dt: f32) {
        if !(dt.is_finite() && dt > 0.0) {
            return;
        }

        // Sanitize look delta: cursor warps/pointer-lock can inject large per-frame deltas.
        // Clamp instead of zeroing to avoid locking movement/rotation on high-DPI devices.
        let max_delta = self.limits.max_look_delta_px.max(1.0);
        if input.look_delta.x.is_finite() {
            input.look_delta.x = input.look_delta.x.clamp(-max_delta, max_delta);
        } else {
            input.look_delta.x = 0.0;
        }
        if input.look_delta.y.is_finite() {
            input.look_delta.y = input.look_delta.y.clamp(-max_delta, max_delta);
        } else {
            input.look_delta.y = 0.0;
        }

        if !input.zoom_delta.is_finite() {
            input.zoom_delta = 0.0;
        }

        // Look activation edge: sync internal angles from current rig pose and suppress
        // synthetic grab delta (cursor warp / pointer-lock transitions).
        if input.look_active && !self.was_look_active {
            match self.mode {
                EditorNavMode::Orbit => {
                    self.orbit.distance = self
                        .orbit
                        .distance
                        .clamp(self.orbit.min_distance, self.orbit.max_distance);
                    self.orbit.sync_from_rig(rig);
                }
                EditorNavMode::Fly => {
                    self.fly.sync_from_rig(rig);
                }
            }

            input.look_delta = Vec2::ZERO;
            input.zoom_delta = 0.0;
            self.look_delta_residual_px = Vec2::ZERO;
        }

        if input.look_active {
            input.look_delta = self.quantize_look_delta_with_residual(input.look_delta);
        } else {
            self.look_delta_residual_px = Vec2::ZERO;
            input.look_delta = Vec2::ZERO;
        }

        self.was_look_active = input.look_active;

        match self.mode {
            EditorNavMode::Orbit => {
                self.apply_orbit(rig, input, dt);
            }
            EditorNavMode::Fly => {
                if self.fly_speed.is_finite() && self.fly_speed > 0.0 {
                    self.fly.move_speed = self.fly_speed;
                }
                self.fly.apply(rig, input, dt);
            }
        }
    }

    /// Synchronizes orbit controller state from the current rig pose.
    ///
    /// Useful when the rig is authored externally (e.g. Follow camera) and we want to return
    /// to Orbit without a snap.
    #[inline]
    pub fn sync_orbit_from_rig(&mut self, rig: &CameraRig) {
        self.orbit.distance = self
            .orbit
            .distance
            .clamp(self.orbit.min_distance, self.orbit.max_distance);
        self.orbit.sync_from_rig(rig);
        self.was_look_active = false;
        self.look_delta_residual_px = Vec2::ZERO;
    }

    /// Synchronizes fly controller state from the current rig pose.
    ///
    /// Useful when the rig is authored externally (e.g. parenting, follow retargeting, editor tools)
    /// and we want to enter/continue Fly navigation without a snap.
    #[inline]
    pub fn sync_fly_from_rig(&mut self, rig: &CameraRig) {
        self.fly.sync_from_rig(rig);
        self.was_look_active = false;
        self.look_delta_residual_px = Vec2::ZERO;
    }

    /// Rebuilds the rig from the current orbit state (no input).
    ///
    /// Use after changing `orbit.target / yaw / pitch / distance` directly (e.g. framing).
    #[inline]
    pub fn rebuild_orbit_rig(&mut self, rig: &mut CameraRig) {
        self.apply_orbit(
            rig,
            CameraInput {
                look_active: false,
                look_delta: Vec2::ZERO,
                move_axis: Vec3::ZERO,
                speed_mul: 1.0,
                zoom_delta: 0.0,
            },
            0.0,
        );
    }

    #[inline]
    fn apply_orbit(&mut self, rig: &mut CameraRig, input: CameraInput, dt: f32) {
        self.orbit.apply(rig, input, dt);

        // Limits + ground clamp are editor policies.
        self.orbit.distance = self.orbit.distance.max(self.limits.min_distance);
        self.orbit.pitch_limit = self.orbit.pitch_limit.min(self.limits.max_pitch_abs);
        self.orbit.pitch = self
            .orbit
            .pitch
            .clamp(-self.limits.max_pitch_abs, self.limits.max_pitch_abs);

        // Ground clamp: keep camera above the floor by lifting the pivot.
        // Iterative adjustment avoids large jumps when the camera is very close to the floor.
        if self.limits.min_camera_y.is_finite() {
            for _ in 0..2 {
                let rot_yaw = Quat::from_rotation_y(self.orbit.yaw);
                let rot_pitch = Quat::from_rotation_x(self.orbit.pitch);
                let rot = (rot_yaw * rot_pitch).normalize_or_identity();
                let pos = self.orbit.target + (rot * Vec3::Z) * self.orbit.distance;
                if pos.y >= self.limits.min_camera_y {
                    rig.position = pos;
                    rig.rotation = rot;
                    return;
                }
                let dy = self.limits.min_camera_y - pos.y;
                self.orbit.target.y += dy;
            }
        }

        // Final write (also covers the case when clamp is disabled).
        let rot_yaw = Quat::from_rotation_y(self.orbit.yaw);
        let rot_pitch = Quat::from_rotation_x(self.orbit.pitch);
        let rot = (rot_yaw * rot_pitch).normalize_or_identity();
        let mut pos = self.orbit.target + (rot * Vec3::Z) * self.orbit.distance;
        if self.limits.min_camera_y.is_finite() && pos.y < self.limits.min_camera_y {
            pos.y = self.limits.min_camera_y;
        }
        rig.position = pos;
        rig.rotation = rot;
    }
}
