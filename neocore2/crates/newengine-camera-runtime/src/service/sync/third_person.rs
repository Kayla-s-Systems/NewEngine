#[inline]
fn step_catch_up_offset(
    current: Vec3,
    velocity: Vec3,
    target: Vec3,
    dt: f32,
    frequency_hz: f32,
    damping_ratio: f32,
) -> (Vec3, Vec3) {
    if !current.is_finite() || !velocity.is_finite() || !target.is_finite() {
        return (target, Vec3::ZERO);
    }
    if !(dt.is_finite() && dt > 0.0) {
        return (target, Vec3::ZERO);
    }
    let frequency_hz = if frequency_hz.is_finite() {
        frequency_hz.clamp(0.01, 60.0)
    } else {
        2.4
    };
    let damping_ratio = if damping_ratio.is_finite() {
        damping_ratio.clamp(0.05, 4.0)
    } else {
        1.0
    };
    let dt = dt.min(0.05);
    let omega = core::f32::consts::TAU * frequency_hz;
    let f = 1.0 + 2.0 * dt * damping_ratio * omega;
    let omega_sq = omega * omega;
    let h_omega_sq = dt * omega_sq;
    let hh_omega_sq = dt * h_omega_sq;
    let inv_det = (f + hh_omega_sq).recip();
    let mut next = (current * f + velocity * dt + target * hh_omega_sq) * inv_det;
    let mut next_velocity = (velocity + (target - current) * h_omega_sq) * inv_det;
    if !next.is_finite() || !next_velocity.is_finite() {
        return (target, Vec3::ZERO);
    }
    // A catch-up trajectory may approach the authored relative frame but never cross it and
    // oscillate around the player. Collision is evaluated after this step.
    if (target - current).dot(target - next) <= 0.0 {
        next = target;
        next_velocity = Vec3::ZERO;
    }
    (next, next_velocity)
}

#[inline]
fn collision_aware_look_rotation(
    pre_collision_camera_ws: Vec3,
    collision_safe_camera_ws: Vec3,
    camera_target_position: Vec3,
    focus_position: Vec3,
    collision_ratio: f32,
    collision_blend: f32,
) -> Quat {
    let pre_collision_rotation = orbit_look_at_rotation(pre_collision_camera_ws, focus_position);
    let collision_ratio = if collision_ratio.is_finite() {
        collision_ratio.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let collision_focus = camera_target_position.lerp(focus_position, collision_ratio);
    let post_collision_rotation = orbit_look_at_rotation(collision_safe_camera_ws, collision_focus);
    let collision_blend = if collision_blend.is_finite() {
        collision_blend.clamp(0.0, 1.0)
    } else {
        0.0
    };
    pre_collision_rotation
        .slerp(post_collision_rotation, collision_blend)
        .normalize_or_identity()
}

#[inline]
fn step_bounded_look_rotation(
    current: Quat,
    desired: Quat,
    dt: f32,
    response_hz: f32,
    max_error_radians: f32,
) -> Quat {
    let desired = desired.normalize_or_identity();
    if !current.is_finite() || !(dt.is_finite() && dt > 0.0) {
        return desired;
    }
    let current = current.normalize_or_identity();
    let max_error = if max_error_radians.is_finite() {
        max_error_radians.clamp(0.0, core::f32::consts::PI)
    } else {
        0.0
    };
    if max_error <= 1.0e-6 {
        return desired;
    }
    let dot = current.dot(desired).abs().clamp(0.0, 1.0);
    let error = 2.0 * dot.acos();
    let bounded_current = if error > max_error && error > 1.0e-6 {
        desired.slerp(current, (max_error / error).clamp(0.0, 1.0))
    } else {
        current
    };
    let response_hz = if response_hz.is_finite() {
        response_hz.clamp(0.01, 120.0)
    } else {
        14.0
    };
    let alpha = (1.0 - (-response_hz * dt.min(0.05)).exp()).clamp(0.0, 1.0);
    bounded_current
        .slerp(desired, alpha)
        .normalize_or_identity()
}
