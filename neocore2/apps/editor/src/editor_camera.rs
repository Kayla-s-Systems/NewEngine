#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_camera::{CameraInput, CameraRig, FreeFlyController, OrbitController};
use newengine_math::Vec2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorCameraMode {
    Orbit,
    Fly,
}

impl Default for EditorCameraMode {
    #[inline]
    fn default() -> Self {
        Self::Orbit
    }
}

/// Editor-side camera controller component.
///
/// Pure controller state (no renderer/world access). Systems/adapters provide input and apply results.
#[derive(Clone, Copy, Debug)]
pub struct EditorCameraController {
    pub mode: EditorCameraMode,

    pub orbit: OrbitController,
    pub fly: FreeFlyController,

    pub fly_speed: f32,

    was_look_active: bool,
}

impl Default for EditorCameraController {
    #[inline]
    fn default() -> Self {
        let mut orbit = OrbitController::default();
        // Deterministic default framing; can be overridden by UI/scene framing logic.
        orbit.yaw = std::f32::consts::FRAC_PI_4;
        orbit.pitch = -0.55;
        orbit.distance = 6.0;

        let mut fly = FreeFlyController::default();
        fly.yaw = 0.7853982;
        fly.pitch = -0.55;

        Self {
            mode: EditorCameraMode::Orbit,
            orbit,
            fly,
            fly_speed: 2.0,
            was_look_active: false,
        }
    }
}

impl EditorCameraController {
    #[inline]
    pub fn apply(&mut self, rig: &mut CameraRig, input: CameraInput, dt: f32) {
        if !(dt.is_finite() && dt > 0.0) {
            return;
        }

        // Look activation edge: synchronize controller state from the current rig pose and
        // suppress the first-frame look delta (cursor grab often produces a large synthetic delta).
        let mut input = input;
        if input.look_active && !self.was_look_active {
            match self.mode {
                EditorCameraMode::Orbit => {
                    self.orbit.distance = self.orbit.distance.clamp(self.orbit.min_distance, self.orbit.max_distance);
                    self.orbit.sync_from_rig(rig);
                }
                EditorCameraMode::Fly => {
                    self.fly.sync_from_rig(rig);
                }
            }

            input.look_delta = Vec2::ZERO;
        }
        self.was_look_active = input.look_active;

        match self.mode {
            EditorCameraMode::Orbit => {
                self.orbit.apply(rig, input, dt);
            }
            EditorCameraMode::Fly => {
                // Enforce fly base speed through controller-owned scalar, but still allow sprint multiplier.
                if self.fly_speed.is_finite() && self.fly_speed > 0.0 {
                    self.fly.move_speed = self.fly_speed;
                }
                self.fly.apply(rig, input, dt);
            }
        }
    }

    /// Synchronizes orbit controller state from the current rig transform.
    ///
    /// Ensures seamless Fly -> Orbit transition without position "snap-back".
    #[inline]
    pub fn sync_orbit_from_rig(&mut self, rig: &CameraRig) {
        // Fly -> Orbit must preserve the UPDATED world pose (position + rotation) produced by Fly.
        //
        // The canonical `OrbitController::sync_from_rig()` does exactly that:
        // - extracts yaw/pitch from `rig.rotation`
        // - recomputes `target` so that `rig.position` stays unchanged for the current `distance`
        //
        // Any attempt to keep the previous pivot (`target`) will project the camera onto the orbit ray,
        // producing a visible snap/backwards kick when the Fly camera is not exactly on the orbit sphere.
        self.orbit.distance = self.orbit.distance.clamp(self.orbit.min_distance, self.orbit.max_distance);
        self.orbit.sync_from_rig(rig);

        // Avoid re-triggering a look edge after explicit mode sync.
        self.was_look_active = false;
    }

    /// Synchronizes fly controller orientation from the current rig transform.
    ///
    /// Avoids the first-frame rotation snap when entering Fly.
    #[inline]
    pub fn sync_fly_from_rig(&mut self, rig: &CameraRig) {
        // Delegate to the canonical implementation to avoid convention drift.
        // Any mismatch here causes a visible snap when re-entering Fly.
        self.fly.sync_from_rig(rig);

        // Avoid re-triggering a look edge after explicit mode sync.
        self.was_look_active = false;
    }
}