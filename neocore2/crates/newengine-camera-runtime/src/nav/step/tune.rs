use crate::nav::BoundsSphere;
use newengine_camera::{RuntimeNavController, RuntimeNavMode};

#[inline]
pub(crate) fn tune_controller(ctrl: &mut RuntimeNavController, mode: RuntimeNavMode, bounds: BoundsSphere) {
    let radius = if bounds.radius.is_finite() { bounds.radius.max(0.001) } else { 1.0 };
    if mode == RuntimeNavMode::Orbit {
        ctrl.orbit.look_sens = 0.0045;
        ctrl.orbit.dolly_speed = (radius * 0.08).clamp(0.35, 120.0);
        ctrl.orbit.pan_speed = (radius * 0.0025).clamp(0.001, 25.0);
        ctrl.orbit.max_distance = ctrl.orbit.max_distance.max((radius * 20.0).clamp(50_000.0, 2_000_000.0));
    } else {
        ctrl.fly.look_sens = 0.0045;
        ctrl.fly_speed = (radius * 0.75).clamp(0.5, 5_000.0);
    }
}