#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::{
    Extent2D, RenderApi, RenderTargetDesc, RenderTargetId, TextureFormat, TextureId,
};
use newengine_core::EngineResult;
use newengine_lighting::{ShadowMethod, ShadowSettings};
use newengine_math::{Mat4, Vec3};
pub(crate) use newengine_render_feature_api::{
    BoundsSnap, LightExtractionCommand, LightExtractionCtx, LightShadowPlan, ShadowCasterCull,
    ShadowFrame, ShadowLightKind,
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
        return lower_light_extraction_command(this, r, world, bounds, lit, settings, command);
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
    command: LightExtractionCommand,
) -> EngineResult<LightShadowPlan> {
    match command {
        LightExtractionCommand::DirectionalShadow => {
            if let Some(plan) = try_build_directional_shadow_plan(this, r, world, bounds, lit, settings)? {
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
) -> EngineResult<Option<LightShadowPlan>> {
    let Some(dir_light) = lights::primary_directional_light(world) else {
        return Ok(None);
    };

    let cascade_count = if matches!(settings.method, ShadowMethod::CascadedShadowMaps) {
        settings.cascade_count.clamp(2, 4)
    } else {
        1
    };

    let Some((rt, shadow_texture)) = ensure_shadow_rt(this, r, settings.resolution, cascade_count)? else {
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

    let radius = bounds.radius.max(4.0).min(settings.max_distance.max(4.0));
    let center = bounds.center;
    let eye = center - dir * (radius * 1.75);
    let up = if dir.dot(Vec3::Y).abs() > 0.92 { Vec3::Z } else { Vec3::Y };
    let view = Mat4::look_at_rh(eye, center, up);
    let near = 0.1;
    let far = radius * 4.0;
    let proj = Mat4::orthographic_rh(-radius, radius, -radius, radius, near, far);
    let light_mvp = proj * view;
    let params = [
        1.0,
        settings.bias,
        settings.contact_strength.clamp(0.0, super::super::render_quality::SHADOW_STRENGTH_MAX),
        settings.softness.clamp(0.0, super::super::render_quality::SHADOW_SOFTNESS_MAX),
    ];
    // Convert artist/profile normal bias into shadow-depth units. The shader
    // multiplies this by receiver slope, so the default 0.015 remains close to
    // the previous hardcoded 0.00018 depth offset but is now scene-controllable.
    let extra = [settings.normal_bias.clamp(0.0, 0.5) * 0.012, cascade_count as f32, 0.0, 0.0];
    let caster_cull = Some(ShadowCasterCull::directional(view, radius, near, far));

    Ok(Some(LightShadowPlan::directional(
        rt,
        shadow_texture,
        settings.resolution,
        light_mvp,
        params,
        extra,
        caster_cull,
    )))
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
    let atlas_key = atlas_extent.width.max(atlas_extent.height);
    let recreate = this.shadows.render_target.is_none() || this.shadows.render_target_resolution != atlas_key;

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
                    format!("game_sun_csm_atlas_{}x{}_cascades_{}", atlas_extent.width, atlas_extent.height, cascades)
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
