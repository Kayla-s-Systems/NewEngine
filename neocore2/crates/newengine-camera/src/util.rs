#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::Vec3;

use crate::controller::OrbitController;

/// Computes a distance that fits a sphere into a perspective frustum.
///
/// `fov_y` in radians.
#[inline]
pub fn fit_distance_for_sphere(fov_y: f32, aspect: f32, radius: f32, margin: f32) -> f32 {
    let r = radius.max(0.000_001) * margin.max(1.0);

    // Use the limiting FOV: vertical or horizontal.
    let tan_y = (0.5 * fov_y).tan().max(1e-6);
    let fov_x = 2.0 * (tan_y * aspect.max(1e-6)).atan();
    let tan_x = (0.5 * fov_x).tan().max(1e-6);

    let d_y = r / tan_y;
    let d_x = r / tan_x;
    d_y.max(d_x).max(0.01)
}

/// Sets an orbit controller to frame a sphere (works for editor orbit AND gameplay orbit cameras).
#[inline]
pub fn orbit_frame_sphere(
    orbit: &mut OrbitController,
    center: Vec3,
    radius: f32,
    fov_y: f32,
    aspect: f32,
    margin: f32,
) {
    orbit.target = center;
    orbit.distance = fit_distance_for_sphere(fov_y, aspect, radius, margin)
        .clamp(orbit.min_distance, orbit.max_distance);
}

/// Computes stable near/far based on camera distance to sphere.
///
/// This is runtime friendly: prevents near clipping during dolly, avoids huge z-fighting.
#[inline]
pub fn auto_near_far_from_sphere(distance: f32, radius: f32) -> (f32, f32) {
    let d = distance.max(0.01);
    let r = radius.max(0.000_001);

    // near should be small enough to never clip the framed sphere,
    // but not too tiny to avoid catastrophic depth precision.
    //
    // IMPORTANT: a `max(near_by_dist, near_by_sphere)` heuristic can make `near` extremely
    // large when the camera is far from a small sphere (d >> r), causing aggressive near clipping.
    let near_by_dist = d * 0.01; // 1% of distance (stable, keeps good precision at scale)
    let near_by_sphere = (d - r * 1.2).max(0.01);
    let near = near_by_dist.min(near_by_sphere).max(0.01);

    // far: include the sphere back + margin, keep a minimum span.
    let far = (d + r * 4.0).max(near + 100.0).max(1000.0);
    (near, far)
}

/// Helper if you want a "default" orbit orientation (like Blender-ish).
#[inline]
pub fn orbit_set_angles(orbit: &mut OrbitController, yaw: f32, pitch: f32) {
    orbit.yaw = yaw;
    orbit.pitch = pitch.clamp(-orbit.pitch_limit, orbit.pitch_limit);
}

/// Exponential smoothing factor for a given speed and delta time.
///
/// Returns `k` in [0..1] such that: `x = x + (target - x) * k`.
#[inline]
pub fn exp_smooth(speed: f32, dt: f32) -> f32 {
    if !speed.is_finite() || speed <= 0.0 {
        return 1.0;
    }
    let dt = dt.max(0.0);
    (1.0 - (-speed * dt).exp()).clamp(0.0, 1.0)
}