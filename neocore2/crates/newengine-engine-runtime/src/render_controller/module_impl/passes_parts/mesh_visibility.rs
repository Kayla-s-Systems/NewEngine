use newengine_math::{Mat4, Vec3};

/// Runtime draw budgets keep the current non-instanced Vulkan path stable.
/// They are intentionally deterministic: nearest objects win, ties are stable-key ordered.
pub(super) const RUNTIME_OPAQUE_PRIMITIVE_BUDGET: usize = 128;
pub(super) const RUNTIME_SHADOW_PRIMITIVE_BUDGET: usize = 96;
pub(super) const EDITOR_OPAQUE_PRIMITIVE_BUDGET: usize = 256;
pub(super) const EDITOR_SHADOW_PRIMITIVE_BUDGET: usize = 160;

#[inline]
pub(super) fn translation_of(model: Mat4) -> Vec3 {
    Vec3::new(model.w_axis.x, model.w_axis.y, model.w_axis.z)
}

#[inline]
pub(super) fn distance_sq_to_camera(model: Mat4, camera_position: Vec3) -> f32 {
    let delta = translation_of(model) - camera_position;
    delta.length_squared()
}

#[inline]
pub(super) fn primitive_budget(runtime: bool, shadow_pass: bool) -> usize {
    match (runtime, shadow_pass) {
        (true, true) => RUNTIME_SHADOW_PRIMITIVE_BUDGET,
        (true, false) => RUNTIME_OPAQUE_PRIMITIVE_BUDGET,
        (false, true) => EDITOR_SHADOW_PRIMITIVE_BUDGET,
        (false, false) => EDITOR_OPAQUE_PRIMITIVE_BUDGET,
    }
}

pub(super) trait DistanceKeyEntry {
    fn distance_sq(&self) -> f32;
    fn stable_key(&self) -> u64;
}

impl<T> DistanceKeyEntry for (f32, u64, T) {
    #[inline]
    fn distance_sq(&self) -> f32 { self.0 }

    #[inline]
    fn stable_key(&self) -> u64 { self.1 }
}

impl<T0, T1, T2> DistanceKeyEntry for (f32, u64, T0, T1, T2) {
    #[inline]
    fn distance_sq(&self) -> f32 { self.0 }

    #[inline]
    fn stable_key(&self) -> u64 { self.1 }
}

impl<T0, T1, T2, T3> DistanceKeyEntry for (f32, u64, T0, T1, T2, T3) {
    #[inline]
    fn distance_sq(&self) -> f32 { self.0 }

    #[inline]
    fn stable_key(&self) -> u64 { self.1 }
}

#[inline]
pub(super) fn sort_by_distance_then_key<T: DistanceKeyEntry>(items: &mut [T]) {
    items.sort_by(|a, b| {
        a.distance_sq()
            .partial_cmp(&b.distance_sq())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.stable_key().cmp(&b.stable_key()))
    });
}


#[inline]
pub(super) fn max_axis_scale(model: Mat4) -> f32 {
    let sx = model.x_axis.truncate().length();
    let sy = model.y_axis.truncate().length();
    let sz = model.z_axis.truncate().length();
    sx.max(sy).max(sz).max(0.001)
}

#[inline]
pub(super) fn transform_sphere(model: Mat4, local_center: Vec3, local_radius: f32) -> (Vec3, f32) {
    (
        model.transform_point3(local_center),
        local_radius.abs().max(0.001) * max_axis_scale(model),
    )
}

#[inline]
pub(super) fn shadow_caster_visible(
    cull: Option<super::super::shadows::ShadowCasterCull>,
    center_ws: Vec3,
    radius_ws: f32,
) -> bool {
    cull.map(|c| c.contains_sphere(center_ws, radius_ws)).unwrap_or(true)
}
