#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::{Quat, Vec3};

#[inline]
pub fn quat_from_forward_z(dir_ws: Vec3) -> Quat {
    let fwd = Vec3::Z;
    let d = dir_ws.normalize_or_zero();
    if d.length_squared() <= 1e-8 {
        return Quat::IDENTITY;
    }
    Quat::from_rotation_arc(fwd, d)
}
