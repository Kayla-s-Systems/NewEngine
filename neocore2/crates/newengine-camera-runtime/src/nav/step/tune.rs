use crate::nav::BoundsSphere;
use newengine_camera::{RuntimeNavController, RuntimeNavMode};

#[inline]
pub(crate) fn tune_controller(ctrl: &mut RuntimeNavController, mode: RuntimeNavMode, bounds: BoundsSphere) {
    if mode == RuntimeNavMode::Orbit {
        ctrl.orbit.look_sens = 0.0045;
        ctrl.orbit.dolly_speed = (bounds.radius * 0.08).clamp(0.35, 3.0);
        ctrl.orbit.pan_speed = (bounds.radius * 0.0025).clamp(0.001, 1.0);
    } else {
        ctrl.fly.look_sens = 0.0045;
        ctrl.fly_speed = (bounds.radius * 0.75).clamp(0.5, 200.0);
    }
}