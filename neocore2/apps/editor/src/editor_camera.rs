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
}

