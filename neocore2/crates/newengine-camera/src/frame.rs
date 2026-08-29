#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::Vec3;

use crate::controller::OrbitController;
use crate::projection::{Perspective, Projection};

/// Computes the orbit distance required to fit a sphere into the view frustum.
///
/// The result is valid for both runtime framing and gameplay cameras.
///
/// Assumptions:
/// - Right-handed world.
/// - Camera forward is -Z (rig convention).
///
/// `margin` is a multiplicative factor applied to the radius (e.g. 1.15).
#[inline]
pub fn fit_distance_for_sphere_perspective(
    fovy: f32,
    aspect: f32,
    radius: f32,
    margin: f32,
) -> f32 {
    let r = (radius.abs().max(1e-6)) * margin.max(1.0);
    let aspect = aspect.max(1e-6);
    let fovy = fovy.max(1e-6);

    let tan_y = (0.5 * fovy).tan().max(1e-6);
    let fovx = 2.0 * (tan_y * aspect).atan();
    let tan_x = (0.5 * fovx).tan().max(1e-6);

    let d_y = r / tan_y;
    let d_x = r / tan_x;
    d_y.max(d_x).max(1e-4)
}

/// Computes robust near/far planes from an orbit distance and target sphere radius.
///
/// Designed to prevent near-plane clipping while allowing close zoom.
#[inline]
pub fn auto_near_far(distance: f32, radius: f32) -> (f32, f32) {
    let d = distance.abs().max(1e-4);
    let r = radius.abs().max(1e-6);

    // Keep near proportional to distance to preserve depth precision.
    // Also ensure the target sphere is not clipped when orbiting closely.
    let near = (d * 0.001).max((d - r * 1.25).max(0.0005));
    let far = (d + r * 4.0).max(near + 0.1);
    (near, far)
}

/// Frames an `OrbitController` and a `Projection` to a bounding sphere.
///
/// This is intentionally editor-agnostic: a game can call it for cutscenes,
/// photo mode, or strategic camera framing.
#[inline]
pub fn frame_orbit_to_sphere(
    orbit: &mut OrbitController,
    projection: &mut Projection,
    viewport_aspect: f32,
    center: Vec3,
    radius: f32,
    margin: f32,
) {
    orbit.target = center;

    match projection {
        Projection::Perspective(p) => {
            p.aspect = viewport_aspect.max(1e-6);
            let dist = fit_distance_for_sphere_perspective(p.fovy, p.aspect, radius, margin);
            orbit.distance = dist.clamp(orbit.min_distance, orbit.max_distance);
            let (near, far) = auto_near_far(orbit.distance, radius);
            p.near = near;
            p.far = far;
        }
        Projection::Orthographic(o) => {
            o.aspect = viewport_aspect.max(1e-6);
            o.half_height = (radius.abs().max(1e-6)) * margin.max(1.0);
            let (near, far) = auto_near_far(orbit.distance, radius);
            o.near = near;
            o.far = far;
        }
    }
}

/// Convenience constructor for a reasonable default perspective used by both editor and game.
#[inline]
pub fn default_perspective(viewport_aspect: f32) -> Projection {
    Projection::Perspective(Perspective::new(
        60.0f32.to_radians(),
        viewport_aspect,
        0.01,
        10_000.0,
    ))
}
