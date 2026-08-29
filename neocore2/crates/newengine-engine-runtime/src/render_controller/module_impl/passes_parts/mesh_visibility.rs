use newengine_math::{Mat4, Vec3};
use std::sync::OnceLock;

/// Runtime draw budgets keep the current non-instanced backend path stable.
/// They are intentionally deterministic: nearest objects win, ties are stable-key ordered.
pub(super) const RUNTIME_OPAQUE_PRIMITIVE_BUDGET: usize = 96;
pub(super) const RUNTIME_SHADOW_PRIMITIVE_BUDGET: usize = 48;
pub(super) const EDITOR_OPAQUE_PRIMITIVE_BUDGET: usize = 256;
pub(super) const EDITOR_SHADOW_PRIMITIVE_BUDGET: usize = 160;
pub(super) const RUNTIME_FOLIAGE_INSTANCE_BUDGET: usize = 16 * 1024;
pub(super) const EDITOR_FOLIAGE_INSTANCE_BUDGET: usize = 16 * 1024;
pub(super) const RUNTIME_TERRAIN_FORWARD_BUDGET: usize = 64;
pub(super) const RUNTIME_TERRAIN_SHADOW_BUDGET: usize = 64;
pub(super) const EDITOR_TERRAIN_FORWARD_BUDGET: usize = 64;
pub(super) const EDITOR_TERRAIN_SHADOW_BUDGET: usize = 64;

#[derive(Clone, Copy, Debug)]
struct MeshRuntimePolicy {
    primitive_budgets: [[usize; 2]; 2],
    foliage_budgets: [[usize; 2]; 2],
    terrain_budgets: [[usize; 2]; 2],
    terrain_receive_shadows_override: Option<bool>,
    scene_culling_enabled: bool,
    terrain_render_distance: f32,
    primitive_render_distance: [f32; 2],
    primitive_shadow_distance: [f32; 2],
    forward_cone_dot: f32,
    terrain_near_accept_override: Option<f32>,
    primitive_near_accept_distance: f32,
}

impl MeshRuntimePolicy {
    fn from_process_config() -> Self {
        let usize_var = |name: &str, default: usize, min: usize, max: usize| {
            crate::env_config::var_u64(name, default as u64, min as u64, max as u64) as usize
        };
        let optional_bool = |name: &str| {
            crate::env_config::var(name).map(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
        };
        let optional_f32 = |name: &str, min: f32, max: f32| {
            crate::env_config::var(name)
                .and_then(|value| value.trim().parse::<f32>().ok())
                .map(|value| value.clamp(min, max))
        };
        let lod_distance_scale = crate::env_config::var_f32(
            newengine_core::startup_window::ENV_LOD_DISTANCE_SCALE,
            1.0,
            0.5,
            2.0,
        );
        let lod_scaled =
            |value: f32, min: f32, max: f32| (value * lod_distance_scale).clamp(min, max);

        Self {
            primitive_budgets: [
                [
                    usize_var(
                        "NEWENGINE_EDITOR_OPAQUE_PRIMITIVE_BUDGET",
                        EDITOR_OPAQUE_PRIMITIVE_BUDGET,
                        8,
                        512,
                    ),
                    usize_var(
                        "NEWENGINE_EDITOR_SHADOW_PRIMITIVE_BUDGET",
                        EDITOR_SHADOW_PRIMITIVE_BUDGET,
                        8,
                        512,
                    ),
                ],
                [
                    usize_var(
                        "NEWENGINE_RUNTIME_OPAQUE_PRIMITIVE_BUDGET",
                        RUNTIME_OPAQUE_PRIMITIVE_BUDGET,
                        8,
                        512,
                    ),
                    usize_var(
                        "NEWENGINE_RUNTIME_SHADOW_PRIMITIVE_BUDGET",
                        RUNTIME_SHADOW_PRIMITIVE_BUDGET,
                        8,
                        512,
                    ),
                ],
            ],
            foliage_budgets: [
                [
                    usize_var(
                        "NEWENGINE_EDITOR_FOLIAGE_INSTANCE_BUDGET",
                        EDITOR_FOLIAGE_INSTANCE_BUDGET,
                        256,
                        64 * 1024,
                    ),
                    usize_var(
                        "NEWENGINE_EDITOR_SHADOW_FOLIAGE_INSTANCE_BUDGET",
                        EDITOR_FOLIAGE_INSTANCE_BUDGET,
                        256,
                        64 * 1024,
                    ),
                ],
                [
                    usize_var(
                        "NEWENGINE_RUNTIME_FOLIAGE_INSTANCE_BUDGET",
                        RUNTIME_FOLIAGE_INSTANCE_BUDGET,
                        256,
                        64 * 1024,
                    ),
                    usize_var(
                        "NEWENGINE_RUNTIME_SHADOW_FOLIAGE_INSTANCE_BUDGET",
                        RUNTIME_FOLIAGE_INSTANCE_BUDGET,
                        256,
                        64 * 1024,
                    ),
                ],
            ],
            terrain_budgets: [
                [
                    usize_var(
                        "NEWENGINE_EDITOR_TERRAIN_FORWARD_BUDGET",
                        EDITOR_TERRAIN_FORWARD_BUDGET,
                        0,
                        256,
                    ),
                    usize_var(
                        "NEWENGINE_EDITOR_TERRAIN_SHADOW_BUDGET",
                        EDITOR_TERRAIN_SHADOW_BUDGET,
                        0,
                        256,
                    ),
                ],
                [
                    usize_var(
                        "NEWENGINE_RUNTIME_TERRAIN_FORWARD_BUDGET",
                        RUNTIME_TERRAIN_FORWARD_BUDGET,
                        0,
                        256,
                    ),
                    usize_var(
                        "NEWENGINE_RUNTIME_TERRAIN_SHADOW_BUDGET",
                        RUNTIME_TERRAIN_SHADOW_BUDGET,
                        0,
                        256,
                    ),
                ],
            ],
            terrain_receive_shadows_override: optional_bool("NEWENGINE_TERRAIN_RECEIVE_SHADOWS"),
            scene_culling_enabled: crate::env_config::var_bool(
                "NEWENGINE_RENDER_SCENE_CULLING",
                false,
            ),
            terrain_render_distance: lod_scaled(
                crate::env_config::var_f32("NEWENGINE_TERRAIN_RENDER_DISTANCE", 96.0, 32.0, 2048.0),
                16.0,
                4096.0,
            ),
            primitive_render_distance: [
                lod_scaled(
                    crate::env_config::var_f32(
                        "NEWENGINE_PRIMITIVE_RENDER_DISTANCE",
                        180.0,
                        8.0,
                        2048.0,
                    ),
                    4.0,
                    4096.0,
                ),
                lod_scaled(
                    crate::env_config::var_f32(
                        "NEWENGINE_PRIMITIVE_RENDER_DISTANCE",
                        64.0,
                        8.0,
                        2048.0,
                    ),
                    4.0,
                    4096.0,
                ),
            ],
            primitive_shadow_distance: [
                lod_scaled(
                    crate::env_config::var_f32(
                        "NEWENGINE_PRIMITIVE_SHADOW_DISTANCE",
                        240.0,
                        16.0,
                        4096.0,
                    ),
                    8.0,
                    4096.0,
                ),
                lod_scaled(
                    crate::env_config::var_f32(
                        "NEWENGINE_PRIMITIVE_SHADOW_DISTANCE",
                        80.0,
                        16.0,
                        4096.0,
                    ),
                    8.0,
                    4096.0,
                ),
            ],
            forward_cone_dot: crate::env_config::var_f32(
                "NEWENGINE_RENDER_FORWARD_CONE_DOT",
                -0.12,
                -0.95,
                0.95,
            ),
            terrain_near_accept_override: optional_f32(
                "NEWENGINE_TERRAIN_NEAR_ACCEPT_DISTANCE",
                8.0,
                2048.0,
            ),
            primitive_near_accept_distance: crate::env_config::var_f32(
                "NEWENGINE_PRIMITIVE_NEAR_ACCEPT_DISTANCE",
                12.0,
                1.0,
                512.0,
            ),
        }
    }
}

#[inline]
#[cfg(test)]
fn scale_lod_distance(value: f32, scale: f32, min: f32, max: f32) -> f32 {
    (value * scale.clamp(0.5, 2.0)).clamp(min, max)
}

#[inline]
fn mesh_runtime_policy() -> &'static MeshRuntimePolicy {
    static POLICY: OnceLock<MeshRuntimePolicy> = OnceLock::new();
    POLICY.get_or_init(MeshRuntimePolicy::from_process_config)
}

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
    mesh_runtime_policy().primitive_budgets[runtime as usize][shadow_pass as usize]
}

#[inline]
pub(super) fn foliage_instance_budget(runtime: bool, shadow_pass: bool) -> usize {
    mesh_runtime_policy().foliage_budgets[runtime as usize][shadow_pass as usize]
}

#[inline]
pub(super) fn terrain_budget(runtime: bool, shadow_pass: bool) -> usize {
    mesh_runtime_policy().terrain_budgets[runtime as usize][shadow_pass as usize]
}

#[inline]
pub(super) fn terrain_receive_shadows_enabled(
    policy: newengine_model_domain_api::MeshShadowPolicy,
) -> bool {
    let authored = matches!(
        policy,
        newengine_model_domain_api::MeshShadowPolicy::ReceiveOnly
            | newengine_model_domain_api::MeshShadowPolicy::CastAndReceive
            | newengine_model_domain_api::MeshShadowPolicy::ProfileControlled
    );
    mesh_runtime_policy()
        .terrain_receive_shadows_override
        .unwrap_or(authored)
}

#[inline]
pub(super) fn terrain_cast_shadows_enabled(
    policy: newengine_model_domain_api::MeshShadowPolicy,
) -> bool {
    matches!(
        policy,
        newengine_model_domain_api::MeshShadowPolicy::CastOnly
            | newengine_model_domain_api::MeshShadowPolicy::CastAndReceive
            | newengine_model_domain_api::MeshShadowPolicy::ProfileControlled
    )
}

#[inline]
pub(super) fn primitive_cast_shadows_enabled(
    options: &newengine_model_domain_api::MeshRenderOptions,
) -> bool {
    use newengine_model_domain_api::{MeshRenderRole, MeshShadowPolicy};

    if matches!(
        options.role,
        MeshRenderRole::SkyBackground
            | MeshRenderRole::CelestialBillboard
            | MeshRenderRole::WeatherVolume
            | MeshRenderRole::FirstPersonViewModel
            | MeshRenderRole::CollisionProxy
            | MeshRenderRole::EditorGizmo
            | MeshRenderRole::DebugPrimitive
    ) {
        return false;
    }

    matches!(
        options.shadow_policy,
        MeshShadowPolicy::CastOnly
            | MeshShadowPolicy::CastAndReceive
            | MeshShadowPolicy::ProfileControlled
    )
}

pub(super) trait DistanceKeyEntry {
    fn distance_sq(&self) -> f32;
    fn stable_key(&self) -> u64;
}

impl<T> DistanceKeyEntry for (f32, u64, T) {
    #[inline]
    fn distance_sq(&self) -> f32 {
        self.0
    }

    #[inline]
    fn stable_key(&self) -> u64 {
        self.1
    }
}

impl<T0, T1, T2> DistanceKeyEntry for (f32, u64, T0, T1, T2) {
    #[inline]
    fn distance_sq(&self) -> f32 {
        self.0
    }

    #[inline]
    fn stable_key(&self) -> u64 {
        self.1
    }
}

impl<T0, T1, T2, T3> DistanceKeyEntry for (f32, u64, T0, T1, T2, T3) {
    #[inline]
    fn distance_sq(&self) -> f32 {
        self.0
    }

    #[inline]
    fn stable_key(&self) -> u64 {
        self.1
    }
}

impl<T0, T1, T2, T3, T4> DistanceKeyEntry for (f32, u64, T0, T1, T2, T3, T4) {
    #[inline]
    fn distance_sq(&self) -> f32 {
        self.0
    }

    #[inline]
    fn stable_key(&self) -> u64 {
        self.1
    }
}

impl<T0, T1, T2, T3, T4, T5> DistanceKeyEntry for (f32, u64, T0, T1, T2, T3, T4, T5) {
    #[inline]
    fn distance_sq(&self) -> f32 {
        self.0
    }

    #[inline]
    fn stable_key(&self) -> u64 {
        self.1
    }
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
    cull.map(|c| c.contains_sphere(center_ws, radius_ws))
        .unwrap_or(true)
}

#[derive(Clone, Copy, Debug)]
pub(super) struct PrimitiveVisibilitySettings {
    pub(super) culling_enabled: bool,
    pub(super) max_distance: f32,
    pub(super) cone_dot: f32,
    pub(super) near_accept_distance: f32,
}

#[inline]
pub(super) fn primitive_visibility_settings(runtime: bool) -> PrimitiveVisibilitySettings {
    PrimitiveVisibilitySettings {
        culling_enabled: render_scene_culling_enabled(),
        max_distance: primitive_forward_max_distance(runtime),
        cone_dot: scene_forward_cone_dot(),
        near_accept_distance: primitive_near_accept_distance(),
    }
}

#[inline]
pub(super) fn render_scene_culling_enabled() -> bool {
    // Do not hide world objects on the CPU extraction path by default. The
    // renderer/backend owns actual frustum clipping; the streaming system owns
    // residency. A cheap forward-cone cull is useful as an opt-in stress knob,
    // but as a default it causes visible pop/disappearance while the camera
    // turns, which is not acceptable for gameplay/world presentation.
    mesh_runtime_policy().scene_culling_enabled
}

/// Conservative forward-visibility test used by the runtime draw lists.
///
/// This is intentionally a cheap scene-streamer bucket test, not a replacement
/// for backend frustum clipping. It keeps the CPU extraction path from treating
/// every resident cell as visible. Nearby objects are always accepted to avoid
/// near-plane popping; farther objects must be inside a wide forward cone.
#[inline]
pub(super) fn forward_sphere_visible(
    camera_position: Vec3,
    camera_forward: Vec3,
    center_ws: Vec3,
    radius_ws: f32,
    max_distance: f32,
    cone_dot: f32,
    near_accept_distance: f32,
) -> bool {
    // The caller owns the culling policy snapshot. Keep this predicate pure so
    // entity extraction never re-enters process configuration in the inner loop.
    let radius = radius_ws.abs().max(0.001);
    let delta = center_ws - camera_position;
    let dist2 = delta.length_squared();
    let max_d = max_distance.max(near_accept_distance).max(radius);
    if dist2 > (max_d + radius) * (max_d + radius) {
        return false;
    }

    let near = near_accept_distance.max(radius * 1.15).max(0.001);
    if dist2 <= near * near {
        return true;
    }

    let forward = camera_forward.normalize_or_zero();
    if forward.length_squared() <= 1.0e-6 {
        return true;
    }

    let dir = delta.normalize_or_zero();
    dir.dot(forward) >= cone_dot.clamp(-0.95, 0.95)
}

#[inline]
pub(super) fn terrain_forward_max_distance() -> f32 {
    mesh_runtime_policy().terrain_render_distance
}

#[inline]
pub(super) fn primitive_forward_max_distance(runtime: bool) -> f32 {
    mesh_runtime_policy().primitive_render_distance[runtime as usize]
}

#[inline]
pub(super) fn primitive_shadow_max_distance(runtime: bool) -> f32 {
    mesh_runtime_policy().primitive_shadow_distance[runtime as usize]
}

#[inline]
pub(super) fn scene_forward_cone_dot() -> f32 {
    // -0.25 is intentionally wider than a strict camera frustum. It keeps
    // objects around the edges alive while cutting resident cells fully behind
    // the player/camera, matching the reference scene streamer's active buckets.
    mesh_runtime_policy().forward_cone_dot
}

#[inline]
pub(super) fn terrain_near_accept_distance(radius_ws: f32) -> f32 {
    mesh_runtime_policy()
        .terrain_near_accept_override
        .unwrap_or_else(|| (radius_ws.abs().max(1.0) * 1.20).clamp(8.0, 2048.0))
}

#[inline]
pub(super) fn primitive_near_accept_distance() -> f32 {
    mesh_runtime_policy().primitive_near_accept_distance
}

#[cfg(test)]
mod startup_lod_scale_tests {
    use super::scale_lod_distance;

    #[test]
    fn lod_distance_scale_preserves_and_scales_runtime_ranges() {
        assert_eq!(scale_lod_distance(100.0, 1.0, 8.0, 4096.0), 100.0);
        assert_eq!(scale_lod_distance(100.0, 0.75, 8.0, 4096.0), 75.0);
        assert_eq!(scale_lod_distance(100.0, 1.5, 8.0, 4096.0), 150.0);
        assert_eq!(scale_lod_distance(3000.0, 2.0, 8.0, 4096.0), 4096.0);
    }
}
