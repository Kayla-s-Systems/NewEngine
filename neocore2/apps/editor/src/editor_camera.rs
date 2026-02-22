#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_camera::{CameraInput, CameraRig, FreeFlyController, OrbitController};

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
}

impl Default for EditorCameraController {
    #[inline]
    fn default() -> Self {
        let mut orbit = OrbitController::default();
        // Deterministic default framing; can be overridden by UI/scene framing logic.
        orbit.yaw = 0.7853982;
        orbit.pitch = -0.55;
        orbit.distance = 6.0;

        let mut fly = FreeFlyController::default();
        fly.yaw = 0.7853982;
        fly.pitch = -0.55;

        Self {
            mode: EditorCameraMode::Orbit,
            orbit,
            fly,
            fly_speed: 6.0,
        }
    }
}

impl EditorCameraController {
    #[inline]
    pub fn apply(&mut self, rig: &mut CameraRig, input: CameraInput, dt: f32) {
        if !(dt.is_finite() && dt > 0.0) {
            return;
        }

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
        let fwd = rig.forward();
        // Derive yaw/pitch from forward vector. Convention: forward is -Z.
        let yaw = (-fwd.x).atan2(-fwd.z);
        let pitch = (-fwd.y).clamp(-1.0, 1.0).asin();

        self.orbit.yaw = yaw;
        self.orbit.pitch = pitch;

        let d = self.orbit.distance.max(self.orbit.min_distance).min(self.orbit.max_distance);
        self.orbit.distance = d;

        // Orbit target is in front of the camera by distance along forward.
        self.orbit.target = rig.position + fwd * d;
    }

    /// Synchronizes fly controller orientation from the current rig transform.
    ///
    /// Avoids the first-frame rotation snap when entering Fly.
    #[inline]
    pub fn sync_fly_from_rig(&mut self, rig: &CameraRig) {
        let fwd = rig.forward();
        let yaw = (-fwd.x).atan2(-fwd.z);
        let pitch = (-fwd.y).clamp(-1.0, 1.0).asin();

        self.fly.yaw = yaw;
        self.fly.pitch = pitch;
    }
}

