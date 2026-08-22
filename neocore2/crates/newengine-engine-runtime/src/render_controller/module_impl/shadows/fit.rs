#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::{RectI32, Viewport};
use newengine_math::{Mat4, Vec3};
use newengine_render_feature_api::{BoundsSnap, MAX_DIRECTIONAL_SHADOW_CASCADES};

#[inline]
pub(super) fn directional_shadow_center(
    bounds: BoundsSnap,
    camera_position: [f32; 3],
    radius: f32,
) -> Vec3 {
    if bounds.radius > radius * 1.25 {
        let camera = Vec3::new(camera_position[0], camera_position[1], camera_position[2]);
        let stable_y = if bounds.center.y.is_finite() {
            bounds.center.y
        } else {
            camera.y
        };
        Vec3::new(camera.x, stable_y, camera.z)
    } else {
        bounds.center
    }
}

#[inline]
pub(super) fn snapped_directional_shadow_center(
    center: Vec3,
    light_dir: Vec3,
    up_hint: Vec3,
    half_x: f32,
    half_y: f32,
    resolution: u32,
) -> Vec3 {
    let resolution = resolution.max(1) as f32;
    let texel_x = (half_x.max(0.001) * 2.0) / resolution;
    let texel_y = (half_y.max(0.001) * 2.0) / resolution;

    let forward = light_dir.normalize_or_zero();
    let mut right = forward.cross(up_hint).normalize_or_zero();
    if right.length_squared() <= 1.0e-8 {
        right = forward.cross(Vec3::Z).normalize_or_zero();
    }
    if right.length_squared() <= 1.0e-8 {
        return center;
    }
    let up = right.cross(forward).normalize_or_zero();
    if up.length_squared() <= 1.0e-8 {
        return center;
    }

    let snap_x = |v: f32| (v / texel_x + 0.5).floor() * texel_x;
    let snap_y = |v: f32| (v / texel_y + 0.5).floor() * texel_y;
    let x = center.dot(right);
    let y = center.dot(up);
    center + right * (snap_x(x) - x) + up * (snap_y(y) - y)
}

#[derive(Clone, Copy, Debug)]
pub(super) struct DirectionalShadowFit {
    pub(super) center: Vec3,
    pub(super) half_x: f32,
    pub(super) half_y: f32,
    pub(super) depth_radius: f32,
}

#[inline]
#[cfg(test)]
pub(super) fn directional_shadow_stable_fit(
    viewproj: Mat4,
    camera: Vec3,
    camera_forward: Vec3,
    split_near: f32,
    split_far: f32,
    resolution: u32,
) -> Option<DirectionalShadowFit> {
    directional_shadow_stable_fit_with_padding(
        viewproj,
        camera,
        camera_forward,
        split_near,
        split_far,
        resolution,
        2.0,
    )
}

#[inline]
#[cfg(test)]
pub(super) fn directional_shadow_stable_fit_with_padding(
    viewproj: Mat4,
    camera: Vec3,
    camera_forward: Vec3,
    split_near: f32,
    split_far: f32,
    resolution: u32,
    kernel_guard_texels: f32,
) -> Option<DirectionalShadowFit> {
    let corners =
        camera_frustum_slice_corners(viewproj, camera, camera_forward, split_near, split_far)?;

    let mut center = Vec3::ZERO;
    for corner in corners {
        center += corner;
    }
    center /= corners.len() as f32;

    let mut radius = 0.0_f32;
    for corner in corners {
        radius = radius.max((corner - center).length());
    }
    if !center.is_finite() || !radius.is_finite() || radius <= 0.0 {
        return None;
    }

    let tile_resolution = resolution.max(1) as f32;
    let radius_quantum = ((split_far.max(1.0) * 2.0) / tile_resolution * 0.25).max(1.0e-4);
    radius = (radius / radius_quantum).ceil() * radius_quantum;

    // Guard the stable fit by the actual receiver-kernel footprint. A fixed two-
    // texel border is sufficient for hard/compact PCF, but wider PCSS kernels need
    // a correspondingly wider world-space safety band or blockers disappear near
    // cascade edges and the penumbra visibly collapses.
    let texel_world = (radius * 2.0) / tile_resolution;
    let guard_texels = if kernel_guard_texels.is_finite() {
        kernel_guard_texels.clamp(2.0, 16.0)
    } else {
        2.0
    };
    let guard = (texel_world * guard_texels).max(0.02);
    let stable_half = radius + guard;

    Some(DirectionalShadowFit {
        center,
        half_x: stable_half,
        half_y: stable_half,
        depth_radius: radius.max(1.0),
    })
}

/// Rotation-invariant CSM fit. The cascade follows camera translation but not
/// camera yaw/pitch: its center is the camera position and only the radius is
/// derived from the frustum slice. This behaves like a directional shadow clipmap
/// and prevents a stationary camera rotation from walking the shadow projection
/// across the light-space texel grid.
#[inline]
pub(super) fn directional_shadow_rotation_invariant_fit_with_padding(
    viewproj: Mat4,
    camera: Vec3,
    camera_forward: Vec3,
    split_near: f32,
    split_far: f32,
    resolution: u32,
    kernel_guard_texels: f32,
) -> Option<DirectionalShadowFit> {
    let corners =
        camera_frustum_slice_corners(viewproj, camera, camera_forward, split_near, split_far)?;
    let mut radius = 0.0_f32;
    for corner in corners {
        radius = radius.max((corner - camera).length());
    }
    if !radius.is_finite() || radius <= 0.0 {
        return None;
    }

    let tile_resolution = resolution.max(1) as f32;
    let radius_quantum = ((split_far.max(1.0) * 2.0) / tile_resolution * 0.25).max(1.0e-4);
    radius = (radius / radius_quantum).ceil() * radius_quantum;
    let texel_world = (radius * 2.0) / tile_resolution;
    let guard_texels = if kernel_guard_texels.is_finite() {
        kernel_guard_texels.clamp(2.0, 16.0)
    } else {
        2.0
    };
    let stable_half = radius + (texel_world * guard_texels).max(0.02);
    Some(DirectionalShadowFit {
        center: camera,
        half_x: stable_half,
        half_y: stable_half,
        depth_radius: radius.max(1.0),
    })
}

#[cfg(test)]
#[inline]
pub(super) fn directional_shadow_frustum_fit(
    viewproj: Mat4,
    camera: Vec3,
    camera_forward: Vec3,
    split_near: f32,
    split_far: f32,
    light_dir: Vec3,
    up_hint: Vec3,
    resolution: u32,
) -> Option<DirectionalShadowFit> {
    let corners =
        camera_frustum_slice_corners(viewproj, camera, camera_forward, split_near, split_far)?;

    let forward = light_dir.normalize_or_zero();
    let mut right = forward.cross(up_hint).normalize_or_zero();
    if right.length_squared() <= 1.0e-8 {
        right = forward.cross(Vec3::Z).normalize_or_zero();
    }
    let light_up = right.cross(forward).normalize_or_zero();
    if right.length_squared() <= 1.0e-8 || light_up.length_squared() <= 1.0e-8 {
        return None;
    }

    let mut center = Vec3::ZERO;
    for corner in corners {
        center += corner;
    }
    center /= corners.len() as f32;

    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for corner in corners {
        let x = corner.dot(right);
        let y = corner.dot(light_up);
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }
    if ![min_x, max_x, min_y, max_y].iter().all(|v| v.is_finite()) {
        return None;
    }

    let target_x = (min_x + max_x) * 0.5;
    let target_y = (min_y + max_y) * 0.5;
    center += right * (target_x - center.dot(right));
    center += light_up * (target_y - center.dot(light_up));

    let raw_half_x = ((max_x - min_x) * 0.5).max(0.5);
    let raw_half_y = ((max_y - min_y) * 0.5).max(0.5);
    let max_half = raw_half_x.max(raw_half_y);
    let texel_guard = (max_half * 2.0 / resolution.max(1) as f32) * 2.0;
    let guard = (max_half * 0.0125).max(texel_guard).max(0.05);
    let half_x = raw_half_x + guard;
    let half_y = raw_half_y + guard;

    let mut depth_radius = 0.0_f32;
    for corner in corners {
        depth_radius = depth_radius.max((corner - center).length());
    }

    Some(DirectionalShadowFit {
        center,
        half_x,
        half_y,
        depth_radius: depth_radius.max(1.0),
    })
}

#[inline]
fn camera_frustum_slice_corners(
    viewproj: Mat4,
    camera: Vec3,
    camera_forward: Vec3,
    split_near: f32,
    split_far: f32,
) -> Option<[Vec3; 8]> {
    let inv = viewproj.inverse();
    if !inv.to_cols_array().iter().all(|v| v.is_finite()) {
        return None;
    }
    let forward = camera_forward.normalize_or_zero();
    if forward.length_squared() <= 1.0e-8 {
        return None;
    }
    let near = split_near.max(0.05);
    let far = split_far.max(near + 0.05);
    let ndc_xy = [(-1.0_f32, -1.0_f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];
    let mut out = [Vec3::ZERO; 8];
    for (i, (x, y)) in ndc_xy.into_iter().enumerate() {
        let far_world = inv.transform_point3(Vec3::new(x, y, 1.0));
        if !far_world.is_finite() {
            return None;
        }
        let ray = (far_world - camera).normalize_or_zero();
        let forward_projection = ray.dot(forward);
        if !forward_projection.is_finite() || forward_projection <= 1.0e-4 {
            return None;
        }
        out[i] = camera + ray * (near / forward_projection);
        out[i + 4] = camera + ray * (far / forward_projection);
    }
    Some(out)
}

#[inline]
pub(super) fn csm_split_distances(
    near: f32,
    far: f32,
    cascade_count: u32,
) -> [f32; MAX_DIRECTIONAL_SHADOW_CASCADES] {
    let count = cascade_count.clamp(1, MAX_DIRECTIONAL_SHADOW_CASCADES as u32) as usize;
    let mut out = [far; MAX_DIRECTIONAL_SHADOW_CASCADES];
    let lambda = 0.68;
    let near = near.max(0.05);
    let far = far.max(near + 1.0);
    for (i, slot) in out.iter_mut().enumerate().take(count) {
        let p = (i + 1) as f32 / count as f32;
        let uniform = near + (far - near) * p;
        let logarithmic = near * (far / near).powf(p);
        *slot = logarithmic * lambda + uniform * (1.0 - lambda);
    }
    out[count - 1] = far;
    out
}

#[inline]
pub(super) fn csm_tile_viewport_scissor(
    index: u32,
    cascade_count: u32,
    resolution: u32,
) -> (Viewport, RectI32) {
    let cascades = cascade_count.clamp(1, MAX_DIRECTIONAL_SHADOW_CASCADES as u32);
    let columns = if cascades <= 1 { 1 } else { 2 };
    let x = (index % columns) * resolution;
    let y = (index / columns) * resolution;
    (
        Viewport {
            x: x as f32,
            y: y as f32,
            w: resolution as f32,
            h: resolution as f32,
            min_depth: 0.0,
            max_depth: 1.0,
        },
        RectI32::new(x as i32, y as i32, resolution as i32, resolution as i32),
    )
}

#[cfg(test)]
mod shadow_fit_tests {
    use super::*;

    fn test_viewproj() -> (Mat4, Vec3, Vec3) {
        let camera = Vec3::ZERO;
        let forward = -Vec3::Z;
        let view = Mat4::look_at_rh(camera, camera + forward, Vec3::Y);
        let projection = Mat4::perspective_rh(60.0_f32.to_radians(), 16.0 / 9.0, 0.1, 250.0);
        (projection * view, camera, forward)
    }

    #[test]
    fn frustum_slice_corners_match_requested_camera_forward_depths() {
        let (viewproj, camera, forward) = test_viewproj();
        let corners = camera_frustum_slice_corners(viewproj, camera, forward, 3.0, 25.0)
            .expect("valid frustum slice");
        for corner in &corners[..4] {
            let forward_depth = (*corner - camera).dot(forward);
            assert!(
                (forward_depth - 3.0).abs() < 0.002,
                "near depth={forward_depth}"
            );
        }
        for corner in &corners[4..] {
            let forward_depth = (*corner - camera).dot(forward);
            assert!(
                (forward_depth - 25.0).abs() < 0.01,
                "far depth={forward_depth}"
            );
        }
        assert!((corners[4] - camera).length() > 25.1);
    }

    #[test]
    fn directional_frustum_fit_is_compact_finite_and_padded() {
        let (viewproj, camera, forward) = test_viewproj();
        let light_dir = Vec3::new(0.42, -0.82, 0.31).normalize_or_zero();
        let fit = directional_shadow_frustum_fit(
            viewproj,
            camera,
            forward,
            0.5,
            30.0,
            light_dir,
            Vec3::Y,
            4096,
        )
        .expect("valid directional fit");
        assert!(fit.center.is_finite());
        assert!(fit.half_x.is_finite() && fit.half_x > 0.5);
        assert!(fit.half_y.is_finite() && fit.half_y > 0.5);
        assert!(fit.depth_radius.is_finite() && fit.depth_radius > 1.0);
        assert!(fit.half_x < 45.0, "half_x={}", fit.half_x);
        assert!(fit.half_y < 45.0, "half_y={}", fit.half_y);
    }

    #[test]
    fn stable_cascade_fit_does_not_breathe_when_camera_rotates() {
        let camera = Vec3::ZERO;
        let projection = Mat4::perspective_rh(60.0_f32.to_radians(), 16.0 / 9.0, 0.1, 250.0);
        let forward_a = -Vec3::Z;
        let forward_b = Vec3::new(0.342, 0.0, -0.940).normalize_or_zero();
        let view_a = Mat4::look_at_rh(camera, camera + forward_a, Vec3::Y);
        let view_b = Mat4::look_at_rh(camera, camera + forward_b, Vec3::Y);
        let fit_a =
            directional_shadow_stable_fit(projection * view_a, camera, forward_a, 0.5, 30.0, 4096)
                .expect("stable fit A");
        let fit_b =
            directional_shadow_stable_fit(projection * view_b, camera, forward_b, 0.5, 30.0, 4096)
                .expect("stable fit B");
        assert!((fit_a.half_x - fit_b.half_x).abs() < 0.01);
        assert!((fit_a.half_y - fit_b.half_y).abs() < 0.01);
        assert!((fit_a.half_x - fit_a.half_y).abs() < 1.0e-5);
    }

    #[test]
    fn wider_filter_kernel_reserves_more_stable_fit_padding() {
        let (viewproj, camera, forward) = test_viewproj();
        let compact = directional_shadow_stable_fit_with_padding(
            viewproj, camera, forward, 0.5, 30.0, 2048, 2.0,
        )
        .expect("compact fit");
        let wide = directional_shadow_stable_fit_with_padding(
            viewproj, camera, forward, 0.5, 30.0, 2048, 8.0,
        )
        .expect("wide fit");
        assert!(wide.half_x > compact.half_x);
        assert!(wide.half_y > compact.half_y);
        assert!((wide.depth_radius - compact.depth_radius).abs() < 1.0e-5);
    }

    #[test]
    fn texel_snapped_center_lands_on_light_space_texel_grid() {
        let light_dir = Vec3::new(0.42, -0.82, 0.31).normalize_or_zero();
        let up_hint = Vec3::Y;
        let half = 37.25;
        let resolution = 2048;
        let center = Vec3::new(13.271, 5.317, -8.913);
        let snapped =
            snapped_directional_shadow_center(center, light_dir, up_hint, half, half, resolution);

        let forward = light_dir.normalize_or_zero();
        let right = forward.cross(up_hint).normalize_or_zero();
        let up = right.cross(forward).normalize_or_zero();
        let texel = (half * 2.0) / resolution as f32;
        let x_units = snapped.dot(right) / texel;
        let y_units = snapped.dot(up) / texel;
        assert!(
            (x_units - x_units.round()).abs() < 2.0e-4,
            "x_units={x_units}"
        );
        assert!(
            (y_units - y_units.round()).abs() < 2.0e-4,
            "y_units={y_units}"
        );
    }

    #[test]
    fn rotation_invariant_fit_keeps_center_and_extent_when_only_view_yaw_changes() {
        let camera = Vec3::new(4.0, 2.0, -7.0);
        let projection = Mat4::perspective_rh(60.0_f32.to_radians(), 16.0 / 9.0, 0.1, 250.0);
        let forward_a = Vec3::Z;
        let forward_b = Vec3::new(0.82, 0.0, 0.57).normalize_or_zero();
        let view_a = Mat4::look_at_rh(camera, camera + forward_a, Vec3::Y);
        let view_b = Mat4::look_at_rh(camera, camera + forward_b, Vec3::Y);
        let a = directional_shadow_rotation_invariant_fit_with_padding(
            projection * view_a,
            camera,
            forward_a,
            0.5,
            24.0,
            2048,
            6.0,
        )
        .expect("fit a");
        let b = directional_shadow_rotation_invariant_fit_with_padding(
            projection * view_b,
            camera,
            forward_b,
            0.5,
            24.0,
            2048,
            6.0,
        )
        .expect("fit b");
        assert!((a.center - b.center).length() < 1.0e-6);
        assert!(
            (a.half_x - b.half_x).abs() < 1.0e-3,
            "{} vs {}",
            a.half_x,
            b.half_x
        );
        assert!((a.half_y - b.half_y).abs() < 1.0e-3);
    }
    #[test]
    fn subtexel_camera_motion_does_not_move_shadow_projection() {
        let light_dir = Vec3::new(0.42, -0.82, 0.31).normalize_or_zero();
        let up_hint = Vec3::Y;
        let half = 32.0;
        let resolution = 2048;
        let texel = half * 2.0 / resolution as f32;
        let right = light_dir.cross(up_hint).normalize_or_zero();
        let center = Vec3::ZERO;
        let a = snapped_directional_shadow_center(
            center + right * (texel * 0.10),
            light_dir,
            up_hint,
            half,
            half,
            resolution,
        );
        let b = snapped_directional_shadow_center(
            center + right * (texel * 0.40),
            light_dir,
            up_hint,
            half,
            half,
            resolution,
        );
        assert!(
            (a - b).length() < 1.0e-5,
            "sub-texel motion changed projection: {a:?} -> {b:?}"
        );
    }
}
