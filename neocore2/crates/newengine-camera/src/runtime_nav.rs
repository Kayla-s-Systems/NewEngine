#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::{Quat, Vec2, Vec3};

use crate::{
    auto_near_far, default_perspective, frame_orbit_to_sphere, CameraChannel, CameraChannelState,
    CameraControlInput, CameraFrame, CameraRig, CameraViewport, OrbitController, Projection,
};

/// Axis-aligned bounding box used by runtime framing.
#[derive(Clone, Copy, Debug, Default)]
pub struct CameraFrameAabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl CameraFrameAabb {
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

    #[inline]
    pub fn radius(&self) -> f32 {
        self.extents().length().max(1.0e-6)
    }
}

/// Runtime preview camera built directly on the stable camera frame contract.
///
/// It owns pose, lens and viewport explicitly and emits a `CameraFrame` every update.
#[derive(Clone, Debug)]
pub struct RuntimePreviewCamera {
    pub rig: CameraRig,
    pub projection: Projection,
    pub orbit: OrbitController,
    pub viewport: CameraViewport,
    pub channel: CameraChannelState,
    pub jitter_px: Vec2,
    pub margin: f32,
    pub focus_radius: f32,
    was_look_active: bool,
}

impl Default for RuntimePreviewCamera {
    #[inline]
    fn default() -> Self {
        let orbit = OrbitController {
            yaw: 0.65,
            pitch: -0.55,
            distance: 6.0,
            ..OrbitController::default()
        };

        Self {
            rig: CameraRig::default(),
            projection: default_perspective(16.0 / 9.0),
            orbit,
            viewport: CameraViewport::default(),
            channel: CameraChannelState::dominant(CameraChannel::Runtime),
            jitter_px: Vec2::ZERO,
            margin: 1.08,
            focus_radius: 1.0,
            was_look_active: false,
        }
    }
}

impl RuntimePreviewCamera {
    #[inline]
    pub fn new(viewport: CameraViewport) -> Self {
        let mut this = Self::default();
        this.set_viewport(viewport.width, viewport.height);
        this
    }

    #[inline]
    pub fn from_size(width: u32, height: u32) -> Self {
        Self::new(CameraViewport::from_size(width, height))
    }

    #[inline]
    pub fn set_viewport(&mut self, width: u32, height: u32) {
        self.viewport = CameraViewport::from_size(width, height);
        self.projection.set_viewport(width, height);
    }

    #[inline]
    pub fn update(&mut self, input: Option<CameraControlInput>, dt: f32) -> CameraFrame {
        if let Some(mut i) = input {
            if i.look_active && !self.was_look_active {
                self.orbit.sync_from_rig(&self.rig);
                i.look_delta = Vec2::ZERO;
            }
            self.was_look_active = i.look_active;
            self.orbit.apply(&mut self.rig, i, dt);
        } else {
            self.was_look_active = false;
            self.orbit
                .apply(&mut self.rig, CameraControlInput::idle(), 0.0);
        }

        let (near, far) = auto_near_far(self.orbit.distance, self.focus_radius);
        match &mut self.projection {
            Projection::Perspective(p) => {
                p.near = near;
                p.far = far.max(p.near + 0.1);
            }
            Projection::Orthographic(o) => {
                o.near = near;
                o.far = far.max(o.near + 0.1);
            }
        }

        CameraFrame::build(
            self.channel,
            self.rig,
            self.projection,
            self.viewport,
            self.jitter_px,
        )
    }

    #[inline]
    pub fn frame_sphere(&mut self, center: Vec3, radius: f32) {
        self.focus_radius = radius.abs().max(1.0e-6);
        frame_orbit_to_sphere(
            &mut self.orbit,
            &mut self.projection,
            self.viewport.aspect(),
            center,
            self.focus_radius,
            self.margin.max(1.0),
        );
        self.orbit
            .apply(&mut self.rig, CameraControlInput::idle(), 0.0);
    }

    #[inline]
    pub fn frame_aabb(&mut self, aabb: CameraFrameAabb) {
        self.frame_sphere(aabb.center(), aabb.radius());
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeNavMode {
    Orbit,
    Fly,
}

impl Default for RuntimeNavMode {
    #[inline]
    fn default() -> Self {
        Self::Orbit
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RuntimeNavLimits {
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

    /// Per-axis raw look noise floor in pixels.
    ///
    /// Deltas below this threshold are treated as backend/device noise and do not accumulate
    /// into residuals. This prevents captured fly/orbit look from slowly drifting in the last
    /// movement direction after the mouse has already stopped.
    pub look_noise_floor_px: f32,
}

impl Default for RuntimeNavLimits {
    #[inline]
    fn default() -> Self {
        Self {
            min_distance: 0.30,
            max_pitch_abs: 1.5184364,
            min_camera_y: 0.10,
            max_look_delta_px: 160.0,
            look_delta_quantum_px: 1.0,
            look_noise_floor_px: 0.35,
        }
    }
}

/// Runtime navigation controller.
///
/// Stores deterministic controller state (Orbit + Fly) and applies it to a `CameraRig`.
#[derive(Clone, Copy, Debug)]
pub struct RuntimeNavController {
    pub mode: RuntimeNavMode,

    pub orbit: OrbitController,
    pub fly: crate::FreeFlyController,

    /// Base fly speed (units/sec). `CameraControlInput.speed_mul` can still multiply it.
    pub fly_speed: f32,

    pub limits: RuntimeNavLimits,

    was_look_active: bool,
    look_delta_residual_px: Vec2,
    look_activation_guard_frames: u8,
}

impl Default for RuntimeNavController {
    #[inline]
    fn default() -> Self {
        let orbit = OrbitController {
            yaw: core::f32::consts::FRAC_PI_4,
            pitch: -0.55,
            distance: 6.0,
            ..OrbitController::default()
        };

        let fly = crate::FreeFlyController {
            yaw: core::f32::consts::FRAC_PI_4,
            pitch: -0.55,
            ..crate::FreeFlyController::default()
        };

        Self {
            mode: RuntimeNavMode::Orbit,
            orbit,
            fly,
            fly_speed: 2.0,
            limits: RuntimeNavLimits::default(),
            was_look_active: false,
            look_delta_residual_px: Vec2::ZERO,
            look_activation_guard_frames: 0,
        }
    }
}

impl RuntimeNavController {
    const LOOK_ACTIVATION_GUARD_FRAMES: u8 = 3;

    #[inline]
    fn quantize_axis_with_residual(
        residual: &mut f32,
        input: f32,
        quantum: f32,
        noise_floor: f32,
    ) -> f32 {
        let q = quantum.max(1.0e-4);
        let floor = noise_floor.max(0.0).min(q);

        if !input.is_finite() {
            *residual = 0.0;
            return 0.0;
        }

        if input.abs() < floor {
            *residual = 0.0;
            return 0.0;
        }

        if residual.signum() != 0.0 && input.signum() != 0.0 && residual.signum() != input.signum()
        {
            *residual = 0.0;
        }

        *residual += input;

        if !residual.is_finite() {
            *residual = 0.0;
            return 0.0;
        }

        if residual.abs() < q {
            return 0.0;
        }

        let out = (*residual / q).trunc() * q;
        *residual -= out;

        if residual.abs() < floor {
            *residual = 0.0;
        }

        out
    }

    #[inline]
    fn quantize_look_delta_with_residual(&mut self, look_delta: Vec2) -> Vec2 {
        let q = self.limits.look_delta_quantum_px.max(1.0e-4);
        let floor = self.limits.look_noise_floor_px.max(0.0).min(q);
        let out = Vec2::new(
            Self::quantize_axis_with_residual(
                &mut self.look_delta_residual_px.x,
                look_delta.x,
                q,
                floor,
            ),
            Self::quantize_axis_with_residual(
                &mut self.look_delta_residual_px.y,
                look_delta.y,
                q,
                floor,
            ),
        );
        if !self.look_delta_residual_px.is_finite() {
            self.look_delta_residual_px = Vec2::ZERO;
        }
        out
    }

    /// Switches navigation mode and synchronizes internal controller state from the current rig.
    ///
    /// This function does **not** modify the rig.
    #[inline]
    pub fn set_mode(&mut self, next: RuntimeNavMode, rig: &CameraRig) {
        if self.mode == next {
            return;
        }

        match next {
            RuntimeNavMode::Orbit => {
                self.orbit.distance = self
                    .orbit
                    .distance
                    .clamp(self.orbit.min_distance, self.orbit.max_distance);
                self.orbit.sync_from_rig(rig);
            }
            RuntimeNavMode::Fly => {
                self.fly.sync_from_rig(rig);
            }
        }

        self.mode = next;
        self.was_look_active = false;
        self.look_delta_residual_px = Vec2::ZERO;
        self.look_activation_guard_frames = Self::LOOK_ACTIVATION_GUARD_FRAMES;
    }

    /// Applies input to the rig for the current mode.
    ///
    /// Guarantees:
    /// - look activation edge (e.g. RMB grab) never teleports the camera
    /// - first-frame synthetic deltas (pointer lock) are suppressed
    #[inline]
    pub fn step(&mut self, rig: &mut CameraRig, mut input: CameraControlInput, dt: f32) {
        input = input.sanitized();
        if !(dt.is_finite() && dt > 0.0) {
            return;
        }

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

        if input.look_active && !self.was_look_active {
            match self.mode {
                RuntimeNavMode::Orbit => {
                    self.orbit.distance = self
                        .orbit
                        .distance
                        .clamp(self.orbit.min_distance, self.orbit.max_distance);
                    self.orbit.sync_from_rig(rig);
                }
                RuntimeNavMode::Fly => {
                    self.fly.sync_from_rig(rig);
                }
            }

            input.look_delta = Vec2::ZERO;
            input.zoom_delta = 0.0;
            self.look_delta_residual_px = Vec2::ZERO;
            self.look_activation_guard_frames = Self::LOOK_ACTIVATION_GUARD_FRAMES;
        }

        if input.look_active {
            if self.look_activation_guard_frames != 0 {
                self.look_activation_guard_frames =
                    self.look_activation_guard_frames.saturating_sub(1);
                self.look_delta_residual_px = Vec2::ZERO;
                input.look_delta = Vec2::ZERO;
                input.zoom_delta = 0.0;
            } else {
                input.look_delta = self.quantize_look_delta_with_residual(input.look_delta);
            }
        } else {
            self.look_activation_guard_frames = 0;
            self.look_delta_residual_px = Vec2::ZERO;
            input.look_delta = Vec2::ZERO;
        }

        self.was_look_active = input.look_active;

        match self.mode {
            RuntimeNavMode::Orbit => {
                self.apply_orbit(rig, input, dt);
            }
            RuntimeNavMode::Fly => {
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
        self.look_activation_guard_frames = Self::LOOK_ACTIVATION_GUARD_FRAMES;
    }

    /// Synchronizes fly controller state from the current rig pose.
    ///
    /// Useful when the rig is authored externally (e.g. parenting, follow retargeting, runtime tools)
    /// and we want to enter/continue Fly navigation without a snap.
    #[inline]
    pub fn sync_fly_from_rig(&mut self, rig: &CameraRig) {
        self.fly.sync_from_rig(rig);
        self.was_look_active = false;
        self.look_delta_residual_px = Vec2::ZERO;
        self.look_activation_guard_frames = Self::LOOK_ACTIVATION_GUARD_FRAMES;
    }

    /// Rebuilds the rig from the current orbit state (no input).
    ///
    /// Use after changing `orbit.target / yaw / pitch / distance` directly (e.g. framing).
    #[inline]
    pub fn rebuild_orbit_rig(&mut self, rig: &mut CameraRig) {
        self.apply_orbit(rig, CameraControlInput::idle(), 0.0);
    }

    #[inline]
    fn apply_orbit(&mut self, rig: &mut CameraRig, input: CameraControlInput, dt: f32) {
        self.orbit.apply(rig, input, dt);

        self.orbit.distance = self.orbit.distance.max(self.limits.min_distance);
        self.orbit.pitch_limit = self.orbit.pitch_limit.min(self.limits.max_pitch_abs);
        self.orbit.pitch = self
            .orbit
            .pitch
            .clamp(-self.limits.max_pitch_abs, self.limits.max_pitch_abs);

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
