#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::{Vec2, Vec3};

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