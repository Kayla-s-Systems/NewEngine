#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::{
    Extent2D, RectI32, RenderApi, RenderTargetDesc, RenderTargetId, TextureFormat, TextureId,
    Viewport,
};
use newengine_core::EngineResult;
use newengine_lighting::{ShadowMethod, ShadowSettings};
use newengine_math::{Mat4, Vec3};
pub(crate) use newengine_render_feature_api::{
    BoundsSnap, LightExtractionCommand, LightExtractionCtx, LightShadowPlan, ShadowCascadeFrame,
    ShadowCasterCull, ShadowFrame, ShadowLightKind, MAX_DIRECTIONAL_SHADOW_CASCADES,
};

use super::lights;
use crate::render_controller::RuntimeRenderController;

#[inline]
pub(super) fn build_light_shadow_plan(
    this: &mut RuntimeRenderController,
    r: &mut dyn RenderApi,
    scene: &newengine_scene::Scene,
    bounds: BoundsSnap,
    lit: newengine_material_domain_api::LitPipeline,
    viewproj: newengine_math::Mat4,
    camera_position: [f32; 3],
    camera_forward: [f32; 3],
    viewport_extent: Extent2D,
    surface_extent: Extent2D,
    plugin_snapshot: Option<&newengine_plugin_host::PluginsSnapshot>,
) -> EngineResult<LightShadowPlan> {
    let world = scene.world();
    let settings = world
        .resource::<ShadowSettings>()
        .copied()
        .unwrap_or_default()
        .sanitized();

    if !settings.enabled || matches!(settings.method, ShadowMethod::None) {
        retire_shadow_rt(this);
        return Ok(LightShadowPlan::disabled(lit.white_texture));
    }

    let mut registry = super::light_extraction::LightExtractionProviderRegistry::from_runtime_providers(
        this.features.light_extraction_providers.runtime_provider_arcs(),
    );
    if let Some(snapshot) = plugin_snapshot {
        registry.sync_plugin_capabilities(snapshot);
    }

    let trace_frame = super::trace_policy::should_trace_frame(this.frame.frame_index);
    if trace_frame && log::log_enabled!(log::Level::Debug) {
        log::debug!(
            "render light extraction providers: {}",
            registry.labels().join(",")
        );
    }

    let ctx = LightExtractionCtx::new(
        world,
        bounds,
        lit,
        settings,
        this.frame.frame_index,
        viewproj,
        camera_position,
        viewport_extent,
        surface_extent,
    );

    if let Some(plan) = registry.extract_external_shadow_plan(&ctx)? {
        return Ok(plan);
    }

    if let Some(command) = registry.extract_runtime_command(&ctx)? {
        return lower_light_extraction_command(
            this,
            r,
            world,
            bounds,
            lit,
            settings,
            camera_position,
            Vec3::new(camera_forward[0], camera_forward[1], camera_forward[2]),
            command,
        );
    }

    retire_shadow_rt(this);
    Ok(LightShadowPlan::disabled(lit.white_texture))
}

#[inline]
fn lower_light_extraction_command(
    this: &mut RuntimeRenderController,
    r: &mut dyn RenderApi,
    world: &newengine_ecs::World,
    bounds: BoundsSnap,
    lit: newengine_material_domain_api::LitPipeline,
    settings: ShadowSettings,
    camera_position: [f32; 3],
    camera_forward: Vec3,
    command: LightExtractionCommand,
) -> EngineResult<LightShadowPlan> {
    match command {
        LightExtractionCommand::DirectionalShadow => {
            if let Some(plan) = try_build_directional_shadow_plan(
                this,
                r,
                world,
                bounds,
                lit,
                settings,
                camera_position,
                camera_forward,
            )? {
                Ok(plan)
            } else {
                retire_shadow_rt(this);
                Ok(LightShadowPlan::disabled(lit.white_texture))
            }
        }
        LightExtractionCommand::Unsupported(ShadowLightKind::Point) => {
            warn_unsupported_point_shadow_once(this);
            retire_shadow_rt(this);
            Ok(LightShadowPlan::unsupported(
                ShadowLightKind::Point,
                lit.white_texture,
                settings.resolution,
            ))
        }
        LightExtractionCommand::Unsupported(ShadowLightKind::Spot) => {
            warn_unsupported_spot_shadow_once(this);
            retire_shadow_rt(this);
            Ok(LightShadowPlan::unsupported(
                ShadowLightKind::Spot,
                lit.white_texture,
                settings.resolution,
            ))
        }
        LightExtractionCommand::Unsupported(ShadowLightKind::Directional) => {
            retire_shadow_rt(this);
            Ok(LightShadowPlan::unsupported(
                ShadowLightKind::Directional,
                lit.white_texture,
                settings.resolution,
            ))
        }
        LightExtractionCommand::Disabled => {
            retire_shadow_rt(this);
            Ok(LightShadowPlan::disabled(lit.white_texture))
        }
    }
}

#[inline]
pub fn try_build_directional_shadow_plan(
    this: &mut RuntimeRenderController,
    r: &mut dyn RenderApi,
    world: &newengine_ecs::World,
    bounds: BoundsSnap,
    _lit: newengine_material_domain_api::LitPipeline,
    settings: ShadowSettings,
    camera_position: [f32; 3],
    camera_forward: Vec3,
) -> EngineResult<Option<LightShadowPlan>> {
    let Some(dir_light) = lights::primary_directional_light(world) else {
        return Ok(None);
    };

    let cascade_count = if matches!(settings.method, ShadowMethod::CascadedShadowMaps) {
        settings.cascade_count.clamp(2, MAX_DIRECTIONAL_SHADOW_CASCADES as u32)
    } else {
        1
    };

    let Some((rt, shadow_texture)) = ensure_shadow_rt(this, r, settings.resolution, cascade_count)?
    else {
        return Ok(None);
    };

    let dir = Vec3::new(
        dir_light.direction_ws[0],
        dir_light.direction_ws[1],
        dir_light.direction_ws[2],
    )
    .normalize_or_zero();
    if dir.length_squared() <= 1.0e-8 {
        return Ok(None);
    }

    let camera = Vec3::new(camera_position[0], camera_position[1], camera_position[2]);
    let camera_forward = camera_forward.normalize_or_zero();
    let camera_forward = if camera_forward.length_squared() > 1.0e-8 {
        camera_forward
    } else {
        Vec3::Z
    };
    let up = if dir.dot(Vec3::Y).abs() > 0.92 { Vec3::Z } else { Vec3::Y };
    let max_distance = settings.max_distance.max(16.0);
    let params = [
        1.0,
        settings.bias,
        settings.contact_strength.clamp(0.0, super::super::render_quality::SHADOW_STRENGTH_MAX),
        settings.softness.clamp(0.0, super::super::render_quality::SHADOW_SOFTNESS_MAX),
    ];
    let extra = [
        settings.normal_bias.clamp(0.0, 0.5) * 0.012,
        cascade_count as f32,
        settings.resolution as f32,
        max_distance,
    ];

    if cascade_count <= 1 {
        let radius = bounds.radius.max(4.0).min(max_distance.max(4.0));
        let center = snapped_directional_shadow_center(
            directional_shadow_center(bounds, camera_position, radius),
            dir,
            up,
            radius,
            settings.resolution,
        );
        let eye = center - dir * (radius * 1.75);
        let view = Mat4::look_at_rh(eye, center, up);
        let near = 0.1;
        let far = radius * 4.0;
        let proj = Mat4::orthographic_rh(-radius, radius, -radius, radius, near, far);
        let caster_cull = Some(ShadowCasterCull::directional(view, radius, near, far));
        return Ok(Some(LightShadowPlan::directional(
            rt,
            shadow_texture,
            settings.resolution,
            proj * view,
            params,
            extra,
            caster_cull,
        )));
    }

    let splits = csm_split_distances(0.5, max_distance, cascade_count);
    let mut cascades = [ShadowCascadeFrame::disabled(); MAX_DIRECTIONAL_SHADOW_CASCADES];
    let mut union_cull = None;
    for i in 0..cascade_count as usize {
        let split_near = if i == 0 { 0.5 } else { splits[i - 1] };
        let split_far = splits[i].max(split_near + 0.1);
        let segment_mid = (split_near + split_far) * 0.5;
        let center = camera + camera_forward * segment_mid;
        let radius = csm_cascade_radius(split_near, split_far, max_distance);
        let snapped_center = snapped_directional_shadow_center(center, dir, up, radius, settings.resolution);
        let eye = snapped_center - dir * (radius * 1.85);
        let view = Mat4::look_at_rh(eye, snapped_center, up);
        let near = 0.1;
        let far = radius * 4.25;
        let proj = Mat4::orthographic_rh(-radius, radius, -radius, radius, near, far);
        let cull = ShadowCasterCull::directional(view, radius, near, far);
        union_cull = Some(cull);
        let (viewport, scissor) = csm_tile_viewport_scissor(i as u32, cascade_count, settings.resolution);
        cascades[i] = ShadowCascadeFrame {
            light_mvp: proj * view,
            viewport,
            scissor,
            split_near,
            split_far,
            texel_world_size: (radius * 2.0) / settings.resolution.max(1) as f32,
            caster_cull: cull,
        };
    }

    Ok(Some(LightShadowPlan::directional_cascaded(
        rt,
        shadow_texture,
        settings.resolution,
        cascade_count,
        cascades,
        params,
        extra,
        union_cull,
    )))
}

#[inline]
fn directional_shadow_center(bounds: BoundsSnap, camera_position: [f32; 3], radius: f32) -> Vec3 {
    if bounds.radius > radius * 1.25 {
        Vec3::new(camera_position[0], camera_position[1], camera_position[2])
    } else {
        bounds.center
    }
}

#[inline]
fn snapped_directional_shadow_center(
    center: Vec3,
    light_dir: Vec3,
    up_hint: Vec3,
    radius: f32,
    resolution: u32,
) -> Vec3 {
    let texel_world_size = (radius * 2.0) / resolution.max(1) as f32;
    if texel_world_size <= 0.0 {
        return center;
    }

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

    let snap = |v: f32| (v / texel_world_size).round() * texel_world_size;
    let x = center.dot(right);
    let y = center.dot(up);
    center + right * (snap(x) - x) + up * (snap(y) - y)
}

#[inline]
fn csm_split_distances(near: f32, far: f32, cascade_count: u32) -> [f32; MAX_DIRECTIONAL_SHADOW_CASCADES] {
    let count = cascade_count.clamp(1, MAX_DIRECTIONAL_SHADOW_CASCADES as u32) as usize;
    let mut out = [far; MAX_DIRECTIONAL_SHADOW_CASCADES];
    let lambda = 0.68;
    let near = near.max(0.05);
    let far = far.max(near + 1.0);
    for i in 0..count {
        let p = (i + 1) as f32 / count as f32;
        let uniform = near + (far - near) * p;
        let logarithmic = near * (far / near).powf(p);
        out[i] = logarithmic * lambda + uniform * (1.0 - lambda);
    }
    out[count - 1] = far;
    out
}

#[inline]
fn csm_cascade_radius(split_near: f32, split_far: f32, max_distance: f32) -> f32 {
    let span = (split_far - split_near).max(1.0);
    let radius = (split_far * 0.72).max(span * 0.95).max(8.0);
    radius.min(max_distance.max(8.0))
}

#[inline]
fn csm_tile_viewport_scissor(index: u32, cascade_count: u32, resolution: u32) -> (Viewport, RectI32) {
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


#[inline]
fn shadow_rt_extent_key(extent: Extent2D) -> u32 {
    let w = extent.width.min(0xFFFF);
    let h = extent.height.min(0xFFFF);
    (w << 16) | h
}

#[inline]
fn ensure_shadow_rt(
    this: &mut RuntimeRenderController,
    r: &mut dyn RenderApi,
    requested_resolution: u32,
    cascade_count: u32,
) -> EngineResult<Option<(RenderTargetId, TextureId)>> {
    let resolution = requested_resolution.clamp(
        super::super::render_quality::SHADOW_RESOLUTION_MIN,
        super::super::render_quality::SHADOW_RESOLUTION_MAX,
    );
    let cascades = cascade_count.clamp(1, 4);
    let columns = if cascades <= 1 { 1 } else { 2 };
    let rows = ((cascades + columns - 1) / columns).max(1);
    let atlas_extent = Extent2D::new(
        resolution.saturating_mul(columns),
        resolution.saturating_mul(rows),
    );
    let atlas_key = shadow_rt_extent_key(atlas_extent);
    let recreate =
        this.shadows.render_target.is_none() || this.shadows.render_target_resolution != atlas_key;

    if recreate {
        if let Some(old) = this.shadows.render_target.take() {
            this.retire_render_target(old);
        }
        this.shadows.render_target_resolution = 0;
        this.invalidate_shadow_cache();
        let rt = r.create_render_target(
            RenderTargetDesc::new(
                atlas_extent,
                super::super::render_quality::SHADOW_MAP_COLOR_FORMAT,
            )
                .with_depth(TextureFormat::Depth32Float)
                .with_label(if cascades > 1 {
                    format!(
                        "game_sun_csm_atlas_{}x{}_cascades_{}",
                        atlas_extent.width, atlas_extent.height, cascades
                    )
                } else {
                    format!("game_sun_shadow_map_{resolution}")
                }),
        )?;
        this.shadows.render_target = Some(rt);
        this.shadows.render_target_resolution = atlas_key;
    }

    let Some(rt) = this.shadows.render_target else {
        return Ok(None);
    };
    let tex = r.render_target_color_texture_id(rt)?;
    Ok(Some((rt, tex)))
}

#[inline]
pub fn retire_shadow_rt(this: &mut RuntimeRenderController) {
    if let Some(old) = this.shadows.render_target.take() {
        this.retire_render_target(old);
    }
    this.shadows.render_target_resolution = 0;
    this.invalidate_shadow_cache();
}

#[inline]
pub fn warn_unsupported_point_shadow_once(this: &mut RuntimeRenderController) {
    if this.shadows.unsupported_point_warning_emitted {
        return;
    }
    this.shadows.unsupported_point_warning_emitted = true;
    log::warn!(
        "render shadows: PointLight is shadow-capable, but point cube-map shadows are not implemented by this Vulkan path yet; falling back to unshadowed point lighting"
    );
}

#[inline]
pub fn warn_unsupported_spot_shadow_once(this: &mut RuntimeRenderController) {
    if this.shadows.unsupported_spot_warning_emitted {
        return;
    }
    this.shadows.unsupported_spot_warning_emitted = true;
    log::warn!(
        "render shadows: Spot shadow maps are planned, but no SpotLight component/backend path is implemented yet; falling back to unshadowed lighting"
    );
}
