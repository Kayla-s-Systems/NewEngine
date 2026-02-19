use super::camera::GizmoCamera;
use crate::GizmoAxis;
use egui::{Pos2, Rect};
use newengine_math::{Quat, Vec3, Vec4};

#[inline]
pub(crate) fn dist_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let ap = p - a;
    let d = ab.x * ab.x + ab.y * ab.y;
    if d <= 1e-6 {
        return ap.length();
    }
    let t = ((ap.x * ab.x + ap.y * ab.y) / d).clamp(0.0, 1.0);
    let q = a + ab * t;
    (p - q).length()
}

pub(crate) fn world_to_screen(camera: &impl GizmoCamera, rect: Rect, world: Vec3) -> Option<(Pos2, f32)> {
    let vp = camera.viewproj();
    let (vp_w, vp_h) = camera.viewport_px();

    let v = vp * Vec4::new(world.x, world.y, world.z, 1.0);
    // Reject points behind the camera (clip-space w <= 0) and pathological projections.
    // Without this, points behind the camera can project to finite NDC and produce huge rings/lines.
    if !v.w.is_finite() || v.w <= 1e-6 {
        return None;
    }
    let ndc = v / v.w;
    if !ndc.x.is_finite() || !ndc.y.is_finite() || !ndc.z.is_finite() {
        return None;
    }

    let sx_px = (ndc.x * 0.5 + 0.5) * vp_w as f32;
    let sy_px = (ndc.y * 0.5 + 0.5) * vp_h as f32;

    // Convert from physical px to egui points inside the given rect.
    let ppp = (rect.width() / vp_w as f32).max(1e-6);
    let x_pt = rect.min.x + sx_px * ppp;
    let y_pt = rect.min.y + sy_px * ppp;

    Some((Pos2::new(x_pt, y_pt), ndc.z))
}

pub(crate) fn screen_to_world_at_ndc_z(camera: &impl GizmoCamera, rect: Rect, screen: Pos2, ndc_z: f32) -> Vec3 {
    let (vp_w, vp_h) = camera.viewport_px();
    let ppp = (rect.width() / vp_w as f32).max(1e-6);

    let px = ((screen.x - rect.min.x) / ppp).clamp(0.0, vp_w as f32);
    let py = ((screen.y - rect.min.y) / ppp).clamp(0.0, vp_h as f32);

    let x = (px / vp_w as f32) * 2.0 - 1.0;
    let y = (py / vp_h as f32) * 2.0 - 1.0;

    let h = camera.inv_viewproj() * Vec4::new(x, y, ndc_z, 1.0);
    if h.w.abs() < 1e-6 {
        return Vec3::ZERO;
    }
    (h / h.w).truncate()
}

pub(crate) fn axis_end(
    camera: &impl GizmoCamera,
    rect: Rect,
    pos: Vec3,
    rot: Quat,
    axis: GizmoAxis,
    center: Pos2,
    desired_len_pt: f32,
) -> Pos2 {
    let dir_world = (rot * axis.vec3()).normalize_or_zero();
    let unit = pos + dir_world;
    let Some((unit_s, _)) = world_to_screen(camera, rect, unit) else {
        return center;
    };
    let d = (unit_s - center).length().max(1.0);
    let len_world = desired_len_pt / d;
    let end_world = pos + dir_world * len_world;
    world_to_screen(camera, rect, end_world).map(|x| x.0).unwrap_or(center)
}

#[inline]
pub(crate) fn plane_basis(n: Vec3) -> (Vec3, Vec3) {
    let refv = if n.z.abs() < 0.9 {
        Vec3::new(0.0, 0.0, 1.0)
    } else {
        Vec3::new(0.0, 1.0, 0.0)
    };
    let u = n.cross(refv).normalize_or_zero();
    let v = n.cross(u).normalize_or_zero();
    (u, v)
}

pub(crate) fn screen_ray(camera: &impl GizmoCamera, rect: Rect, screen: Pos2) -> (Vec3, Vec3) {
    let p0 = screen_to_world_at_ndc_z(camera, rect, screen, -1.0);
    let p1 = screen_to_world_at_ndc_z(camera, rect, screen, 1.0);
    let dir = (p1 - p0).normalize_or_zero();
    (p0, dir)
}

pub(crate) fn ray_plane_intersect(ray_o: Vec3, ray_d: Vec3, plane_p: Vec3, plane_n: Vec3) -> Option<Vec3> {
    let denom = plane_n.dot(ray_d);
    if denom.abs() < 1e-6 {
        return None;
    }
    let t = plane_n.dot(plane_p - ray_o) / denom;
    if !t.is_finite() {
        return None;
    }
    Some(ray_o + ray_d * t)
}

pub(crate) fn rotation_angle_on_plane(
    camera: &impl GizmoCamera,
    rect: Rect,
    pivot: Vec3,
    axis_n: Vec3,
    plane_u: Vec3,
    plane_v: Vec3,
    mouse: Pos2,
) -> f32 {
    let (ro, rd) = screen_ray(camera, rect, mouse);
    let Some(hit) = ray_plane_intersect(ro, rd, pivot, axis_n) else {
        return 0.0;
    };
    let r = hit - pivot;
    let x = r.dot(plane_u);
    let y = r.dot(plane_v);
    y.atan2(x)
}

pub(crate) fn world_radius_for_screen(camera: &impl GizmoCamera, rect: Rect, pivot: Vec3, dir: Vec3, desired_px: f32) -> f32 {
    let Some((c, _)) = world_to_screen(camera, rect, pivot) else {
        return 1.0;
    };
    let Some((u, _)) = world_to_screen(camera, rect, pivot + dir) else {
        return 1.0;
    };
    let d = (u - c).length().max(1.0);
    desired_px / d
}

/// Computes a stable world-space radius for a rotation ring that should appear with ~constant
/// on-screen size.
///
/// Uses two orthonormal in-plane directions and picks the more stable screen delta. This prevents
/// explosive radii when one direction becomes nearly parallel to the view ray.
#[inline]
pub(crate) fn world_radius_for_screen_plane(
    camera: &impl GizmoCamera,
    rect: Rect,
    pivot: Vec3,
    u: Vec3,
    v: Vec3,
    desired_px: f32,
) -> f32 {
    let Some((c, _)) = world_to_screen(camera, rect, pivot) else { return 1.0; };

    let du = world_to_screen(camera, rect, pivot + u).map(|x| (x.0 - c).length());
    let dv = world_to_screen(camera, rect, pivot + v).map(|x| (x.0 - c).length());

    // Prefer the larger delta (less degenerate projection). Clamp to avoid infinity.
    let mut d = 0.0_f32;
    if let Some(x) = du { d = d.max(x); }
    if let Some(x) = dv { d = d.max(x); }
    d = d.max(6.0);

    desired_px / d
}
