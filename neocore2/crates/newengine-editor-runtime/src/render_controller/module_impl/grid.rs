#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_camera::CameraRig;
use newengine_math::{Mat4, Quat, Vec3};

pub(super) const HALF_LINES: i32 = 512;
pub(super) const MAJOR_EVERY: i32 = 10;
pub(super) const MINOR_COLOR: [f32; 4] = [0.32, 0.32, 0.34, 1.0];
pub(super) const MAJOR_COLOR: [f32; 4] = [0.45, 0.45, 0.48, 1.0];
pub(super) const BACKGROUND_COLOR: [f32; 4] = [0.10, 0.10, 0.11, 1.0];
pub(super) const WORLD_SPACING: f32 = 1.0;

#[inline]
fn snap_world(value: f32, spacing: f32) -> f32 {
    if !value.is_finite() {
        return 0.0;
    }

    let cell = spacing.max(1.0e-4);
    (value / cell).round() * cell
}

#[inline]
pub(super) fn origin_from_camera(rig: &CameraRig) -> Vec3 {
    Vec3::new(
        snap_world(rig.position.x, WORLD_SPACING),
        0.0,
        snap_world(rig.position.z, WORLD_SPACING),
    )
}

#[inline]
pub(super) fn model_from_camera(rig: &CameraRig) -> Mat4 {
    Mat4::from_scale_rotation_translation(
        Vec3::new(WORLD_SPACING, 1.0, WORLD_SPACING),
        Quat::IDENTITY,
        origin_from_camera(rig),
    )
}
