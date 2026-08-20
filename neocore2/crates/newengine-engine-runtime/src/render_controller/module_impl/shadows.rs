#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::{Extent2D, RenderApi};
use newengine_core::EngineResult;
use newengine_lighting::{ShadowFilter, ShadowMethod, ShadowSettings};
use newengine_math::{Mat4, Vec3};
pub(crate) use newengine_render_feature_api::{
    BoundsSnap, LightExtractionCommand, LightExtractionCtx, LightShadowPlan, LocalShadowFrame,
    LocalShadowPlan, ShadowCascadeFrame, ShadowCasterCull, ShadowFrame, ShadowLightKind,
    MAX_DIRECTIONAL_SHADOW_CASCADES,
};

use super::lights;
use crate::render_controller::RuntimeRenderController;

mod fit;
mod local;
mod targets;

use fit::{
    csm_cascade_radius, csm_split_distances, csm_tile_viewport_scissor, directional_shadow_center,
    directional_shadow_stable_fit_with_padding, snapped_directional_shadow_center,
    DirectionalShadowFit,
};
pub(super) use local::build_local_shadow_plan;
use targets::{
    ensure_shadow_rt, retire_shadow_rt, warn_unsupported_point_shadow_once,
    warn_unsupported_spot_shadow_once,
};

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

    let mut registry =
        super::light_extraction::LightExtractionProviderRegistry::from_runtime_providers(
            this.features
                .light_extraction_providers
                .runtime_provider_arcs(),
        );
    if let Some(snapshot) = plugin_snapshot {
        registry.sync_plugin_capabilities(snapshot);
    }

    let trace_frame = super::trace_policy::should_trace_frame(this.frame.frame_index);
    if trace_frame && newengine_ulog_api::ulog::debug_enabled() {
        newengine_ulog_api::ulog::debug!(
            "render light extraction providers: {}",
            registry.labels().join(",")
        );
    }

    let light_snapshot = super::lights::collect_light_scene_snapshot(world);
    let ctx = LightExtractionCtx::new(
        &light_snapshot,
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
            viewproj,
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
    viewproj: Mat4,
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
                viewproj,
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
    viewproj: Mat4,
    camera_position: [f32; 3],
    camera_forward: Vec3,
) -> EngineResult<Option<LightShadowPlan>> {
    let Some(dir_light) = lights::primary_directional_light(world) else {
        return Ok(None);
    };

    let cascade_count = if matches!(settings.method, ShadowMethod::CascadedShadowMaps) {
        settings
            .cascade_count
            .clamp(2, MAX_DIRECTIONAL_SHADOW_CASCADES as u32)
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
    let up = if dir.dot(Vec3::Y).abs() > 0.92 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    let max_distance = settings.max_distance.max(16.0);
    let params = [
        1.0,
        settings.bias,
        settings
            .contact_strength
            .clamp(0.0, super::super::render_quality::SHADOW_STRENGTH_MAX),
        settings
            .softness
            .clamp(0.0, super::super::render_quality::SHADOW_SOFTNESS_MAX),
    ];
    let extra = [
        settings.normal_bias.clamp(0.0, 0.5),
        cascade_count as f32,
        settings.resolution as f32,
        max_distance,
    ];
    let pcss = settings.pcss.sanitized();
    let filter_mode = match settings.filter {
        ShadowFilter::Hard => 0.0,
        ShadowFilter::Pcf => 1.0,
        ShadowFilter::Pcss => 2.0,
    };
    let pcss0 = [
        filter_mode,
        pcss.light_angular_radius_tangent(),
        pcss.blocker_search_radius_texels,
        pcss.max_filter_radius_texels,
    ];
    let pcss1 = [
        pcss.blocker_samples as f32,
        pcss.filter_samples as f32,
        pcss.min_filter_radius_texels,
        pcss.stable_kernel_cell_texels,
    ];

    // The cascade projection must reserve at least the receiver kernel footprint.
    // Otherwise a wide PCSS search/filter kernel reaches the edge of a perfectly
    // stable atlas tile, clamps its taps, and produces a false bright/soft strip.
    let kernel_guard_texels = match settings.filter {
        ShadowFilter::Hard => 2.0,
        ShadowFilter::Pcf => settings.softness.max(pcss.min_filter_radius_texels).ceil() + 2.0,
        ShadowFilter::Pcss => {
            pcss.blocker_search_radius_texels
                .max(pcss.max_filter_radius_texels)
                .ceil()
                + 2.0
        }
    }
    .clamp(2.0, 16.0);

    if cascade_count <= 1 {
        let fallback_radius = bounds.radius.max(4.0).min(max_distance.max(4.0));
        let fallback_center = directional_shadow_center(bounds, camera_position, fallback_radius);
        let texel_world = fallback_radius * 2.0 / settings.resolution.max(1) as f32;
        let stable_half = fallback_radius + (texel_world * kernel_guard_texels).max(0.02);
        let center = snapped_directional_shadow_center(
            fallback_center,
            dir,
            up,
            stable_half,
            stable_half,
            settings.resolution,
        );
        let depth_radius = stable_half.max(4.0);
        let eye = center - dir * (depth_radius * 1.90);
        let view = Mat4::look_at_rh(eye, center, up);
        let near = 0.1;
        let far = depth_radius * 4.20;
        let proj = Mat4::orthographic_rh(
            -stable_half,
            stable_half,
            -stable_half,
            stable_half,
            near,
            far,
        );
        let caster_cull = Some(ShadowCasterCull::directional(view, stable_half, near, far));
        return Ok(Some(
            LightShadowPlan::directional(
                rt,
                shadow_texture,
                settings.resolution,
                proj * view,
                params,
                extra,
                caster_cull,
            )
            .with_pcss(pcss0, pcss1),
        ));
    }

    let splits = csm_split_distances(0.5, max_distance, cascade_count);
    let mut cascades = [ShadowCascadeFrame::disabled(); MAX_DIRECTIONAL_SHADOW_CASCADES];
    let mut union_cull = None;
    for i in 0..cascade_count as usize {
        let split_near = if i == 0 { 0.5 } else { splits[i - 1] };
        let split_far = splits[i].max(split_near + 0.1);
        let segment_mid = (split_near + split_far) * 0.5;
        let fallback_radius = csm_cascade_radius(split_near, split_far, max_distance);
        let fallback_center = camera + camera_forward * segment_mid;
        let fallback_texel_world = fallback_radius * 2.0 / settings.resolution.max(1) as f32;
        let fallback_guard = (fallback_texel_world * kernel_guard_texels).max(0.02);
        let fallback_half = fallback_radius + fallback_guard;

        // Rotation-invariant sphere fit + texel snapping prevents cascade breathing.
        // Padding is filter-aware, matching the PCF/PCSS receiver footprint rather
        // than assuming a fixed two-texel kernel.
        let fit = directional_shadow_stable_fit_with_padding(
            viewproj,
            camera,
            camera_forward,
            split_near,
            split_far,
            settings.resolution,
            kernel_guard_texels,
        )
        .unwrap_or(DirectionalShadowFit {
            center: fallback_center,
            half_x: fallback_half,
            half_y: fallback_half,
            depth_radius: fallback_radius,
        });
        let snapped_center = snapped_directional_shadow_center(
            fit.center,
            dir,
            up,
            fit.half_x,
            fit.half_y,
            settings.resolution,
        );
        let depth_radius = fit.depth_radius.max(fit.half_x.max(fit.half_y)).max(4.0);
        let eye = snapped_center - dir * (depth_radius * 1.95);
        let view = Mat4::look_at_rh(eye, snapped_center, up);
        let near = 0.1;
        let far = depth_radius * 4.35;
        let proj =
            Mat4::orthographic_rh(-fit.half_x, fit.half_x, -fit.half_y, fit.half_y, near, far);
        let cull = ShadowCasterCull::directional(view, fit.half_x.max(fit.half_y), near, far);
        union_cull = Some(cull);
        let (viewport, scissor) =
            csm_tile_viewport_scissor(i as u32, cascade_count, settings.resolution);
        cascades[i] = ShadowCascadeFrame {
            light_mvp: proj * view,
            viewport,
            scissor,
            split_near,
            split_far,
            texel_world_size: ((fit.half_x.max(fit.half_y) * 2.0)
                / settings.resolution.max(1) as f32)
                .max(1.0e-6),
            caster_cull: cull,
        };
    }

    Ok(Some(
        LightShadowPlan::directional_cascaded(
            rt,
            shadow_texture,
            settings.resolution,
            cascade_count,
            cascades,
            params,
            extra,
            union_cull,
        )
        .with_pcss(pcss0, pcss1),
    ))
}
