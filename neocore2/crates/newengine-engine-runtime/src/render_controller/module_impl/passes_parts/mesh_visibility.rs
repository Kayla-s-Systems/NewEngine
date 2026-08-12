use newengine_math::{Mat4, Vec3};

/// Runtime draw budgets keep the current non-instanced Vulkan path stable.
/// They are intentionally deterministic: nearest objects win, ties are stable-key ordered.
pub(super) const RUNTIME_OPAQUE_PRIMITIVE_BUDGET: usize = 96;
pub(super) const RUNTIME_SHADOW_PRIMITIVE_BUDGET: usize = 48;
pub(super) const EDITOR_OPAQUE_PRIMITIVE_BUDGET: usize = 256;
pub(super) const EDITOR_SHADOW_PRIMITIVE_BUDGET: usize = 160;
pub(super) const RUNTIME_TERRAIN_FORWARD_BUDGET: usize = 64;
pub(super) const RUNTIME_TERRAIN_SHADOW_BUDGET: usize = 64;
pub(super) const EDITOR_TERRAIN_FORWARD_BUDGET: usize = 64;
pub(super) const EDITOR_TERRAIN_SHADOW_BUDGET: usize = 64;

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
    let default = match (runtime, shadow_pass) {
        (true, true) => RUNTIME_SHADOW_PRIMITIVE_BUDGET,
        (true, false) => RUNTIME_OPAQUE_PRIMITIVE_BUDGET,
        (false, true) => EDITOR_SHADOW_PRIMITIVE_BUDGET,
        (false, false) => EDITOR_OPAQUE_PRIMITIVE_BUDGET,
    };

    let key = match (runtime, shadow_pass) {
        (true, true) => "NEWENGINE_RUNTIME_SHADOW_PRIMITIVE_BUDGET",
        (true, false) => "NEWENGINE_RUNTIME_OPAQUE_PRIMITIVE_BUDGET",
        (false, true) => "NEWENGINE_EDITOR_SHADOW_PRIMITIVE_BUDGET",
        (false, false) => "NEWENGINE_EDITOR_OPAQUE_PRIMITIVE_BUDGET",
    };

    crate::env_config::var(key)
        .and_then(|value| value.trim().parse::<usize>().ok())
        .map(|value| value.clamp(8, 512))
        .unwrap_or(default)
}

#[inline]
pub(super) fn terrain_budget(runtime: bool, shadow_pass: bool) -> usize {
    let default = match (runtime, shadow_pass) {
        (true, true) => RUNTIME_TERRAIN_SHADOW_BUDGET,
        (true, false) => RUNTIME_TERRAIN_FORWARD_BUDGET,
        (false, true) => EDITOR_TERRAIN_SHADOW_BUDGET,
        (false, false) => EDITOR_TERRAIN_FORWARD_BUDGET,
    };

    let key = match (runtime, shadow_pass) {
        (true, true) => "NEWENGINE_RUNTIME_TERRAIN_SHADOW_BUDGET",
        (true, false) => "NEWENGINE_RUNTIME_TERRAIN_FORWARD_BUDGET",
        (false, true) => "NEWENGINE_EDITOR_TERRAIN_SHADOW_BUDGET",
        (false, false) => "NEWENGINE_EDITOR_TERRAIN_FORWARD_BUDGET",
    };

    crate::env_config::var(key)
        .and_then(|value| value.trim().parse::<usize>().ok())
        .map(|value| value.clamp(0, 256))
        .unwrap_or(default)
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
    crate::env_config::var("NEWENGINE_TERRAIN_RECEIVE_SHADOWS")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
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

#[inline]
pub(super) fn render_scene_culling_enabled() -> bool {
    // Do not hide world objects on the CPU extraction path by default. The
    // renderer/backend owns actual frustum clipping; the streaming system owns
    // residency. A cheap forward-cone cull is useful as an opt-in stress knob,
    // but as a default it causes visible pop/disappearance while the camera
    // turns, which is not acceptable for gameplay/world presentation.
    crate::env_config::var_bool("NEWENGINE_RENDER_SCENE_CULLING", false)
}

#[inline]
fn env_f32(name: &str, default: f32, min: f32, max: f32) -> f32 {
    crate::env_config::var_f32(name, default, min, max)
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
    if !render_scene_culling_enabled() {
        return true;
    }

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
    env_f32("NEWENGINE_TERRAIN_RENDER_DISTANCE", 96.0, 32.0, 2048.0)
}

#[inline]
pub(super) fn primitive_forward_max_distance(runtime: bool) -> f32 {
    let default = if runtime { 64.0 } else { 180.0 };
    env_f32("NEWENGINE_PRIMITIVE_RENDER_DISTANCE", default, 8.0, 2048.0)
}

#[inline]
pub(super) fn primitive_shadow_max_distance(runtime: bool) -> f32 {
    let default = if runtime { 80.0 } else { 240.0 };
    env_f32("NEWENGINE_PRIMITIVE_SHADOW_DISTANCE", default, 16.0, 4096.0)
}

#[inline]
pub(super) fn scene_forward_cone_dot() -> f32 {
    // -0.25 is intentionally wider than a strict camera frustum. It keeps
    // objects around the edges alive while cutting resident cells fully behind
    // the player/camera, matching the reference scene streamer's active buckets.
    env_f32("NEWENGINE_RENDER_FORWARD_CONE_DOT", -0.12, -0.95, 0.95)
}

#[inline]
pub(super) fn terrain_near_accept_distance(radius_ws: f32) -> f32 {
    env_f32(
        "NEWENGINE_TERRAIN_NEAR_ACCEPT_DISTANCE",
        radius_ws.abs().max(1.0) * 1.20,
        8.0,
        2048.0,
    )
}

#[inline]
pub(super) fn primitive_near_accept_distance() -> f32 {
    env_f32("NEWENGINE_PRIMITIVE_NEAR_ACCEPT_DISTANCE", 12.0, 1.0, 512.0)
}
