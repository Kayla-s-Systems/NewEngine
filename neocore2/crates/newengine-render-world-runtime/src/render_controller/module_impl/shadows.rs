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

#[inline]
fn apply_startup_shadow_overrides(settings: ShadowSettings) -> ShadowSettings {
    let graphics = newengine_core::startup_launch_settings().graphics;
    apply_shadow_graphics_overrides(settings, &graphics)
}

#[inline]
fn apply_shadow_graphics_overrides(
    mut settings: ShadowSettings,
    graphics: &newengine_core::StartupGraphicsSettings,
) -> ShadowSettings {
    if !graphics.shadows_enabled
        || matches!(graphics.shadow_quality, newengine_core::ShadowQuality::Off)
    {
        settings.enabled = false;
        settings.method = ShadowMethod::None;
        return settings.sanitized();
    }

    // PreStartSettings owns the runtime shadow-quality tier. Cascade count and map
    // resolution remain independent explicit controls below; the quality tier only
    // selects reconstruction quality/sample budget so project-authored cascade layout
    // is never silently replaced by a preset.
    match graphics.shadow_quality {
        newengine_core::ShadowQuality::Off => unreachable!(),
        newengine_core::ShadowQuality::Performance => {
            settings.filter = ShadowFilter::Pcf;
            settings.softness = settings.softness.min(0.85);
            settings.pcss.blocker_samples = settings.pcss.blocker_samples.min(6);
            settings.pcss.filter_samples = settings.pcss.filter_samples.min(8);
            settings.pcss.max_filter_radius_texels =
                settings.pcss.max_filter_radius_texels.min(3.0);
        }
        newengine_core::ShadowQuality::Balanced => {
            settings.filter = ShadowFilter::Pcss;
            settings.pcss.blocker_samples = settings.pcss.blocker_samples.max(8);
            settings.pcss.filter_samples = settings.pcss.filter_samples.max(12);
            settings.pcss.max_filter_radius_texels =
                settings.pcss.max_filter_radius_texels.max(4.0);
        }
        newengine_core::ShadowQuality::Quality => {
            settings.filter = ShadowFilter::Pcss;
            settings.pcss.blocker_samples = settings.pcss.blocker_samples.max(12);
            settings.pcss.filter_samples = settings.pcss.filter_samples.max(16);
            settings.pcss.blocker_search_radius_texels =
                settings.pcss.blocker_search_radius_texels.max(3.5);
            settings.pcss.max_filter_radius_texels =
                settings.pcss.max_filter_radius_texels.max(6.0);
            settings.pcss.min_filter_radius_texels =
                settings.pcss.min_filter_radius_texels.max(0.20);
        }
        newengine_core::ShadowQuality::Cinematic => {
            settings.filter = ShadowFilter::Pcss;
            settings.pcss.blocker_samples = 16;
            settings.pcss.filter_samples = 16;
            settings.pcss.blocker_search_radius_texels =
                settings.pcss.blocker_search_radius_texels.max(4.5);
            settings.pcss.max_filter_radius_texels =
                settings.pcss.max_filter_radius_texels.max(8.0);
            settings.pcss.min_filter_radius_texels =
                settings.pcss.min_filter_radius_texels.max(0.24);
        }
    }

    // Quality is the baseline. Exact filtering/bias/PCSS controls become authoritative
    // only after a manual advanced edit; this avoids persisted defaults silently defeating
    // the selected quality tier.
    if graphics.shadow_advanced_override {
        settings.filter = match graphics.shadow_filter {
            newengine_core::ShadowFilterMode::Hard => ShadowFilter::Hard,
            newengine_core::ShadowFilterMode::Pcf => ShadowFilter::Pcf,
            newengine_core::ShadowFilterMode::Pcss => ShadowFilter::Pcss,
        };
        settings.max_distance = graphics.shadow_max_distance;
        settings.softness = graphics.shadow_softness;
        settings.bias = graphics.shadow_bias;
        settings.normal_bias = graphics.shadow_normal_bias;
        settings.contact_strength = graphics.shadow_contact_strength;
        settings.pcss.light_angular_radius_degrees = graphics.shadow_pcss_light_radius_degrees;
        settings.pcss.blocker_search_radius_texels = graphics.shadow_pcss_blocker_radius_texels;
        settings.pcss.max_filter_radius_texels = graphics.shadow_pcss_max_filter_radius_texels;
        settings.pcss.blocker_samples = graphics.shadow_pcss_blocker_samples;
        settings.pcss.filter_samples = graphics.shadow_pcss_filter_samples;
        settings.pcss.min_filter_radius_texels = graphics.shadow_pcss_min_filter_radius_texels;
        settings.pcss.stable_kernel_cell_texels = graphics.shadow_pcss_stable_kernel_texels;
    }

    if graphics.shadow_map_resolution != 0 {
        settings.resolution = graphics.shadow_map_resolution.clamp(
            super::super::render_quality::SHADOW_RESOLUTION_MIN,
            super::super::render_quality::SHADOW_RESOLUTION_MAX,
        );
    }
    if graphics.shadow_cascade_count != 0 {
        settings.cascade_count = graphics.shadow_cascade_count.clamp(1, 4);
        settings.method = if settings.cascade_count > 1 {
            ShadowMethod::CascadedShadowMaps
        } else {
            ShadowMethod::DirectionalDepthMap
        };
    }
    settings.sanitized()
}

use fit::{
    csm_split_distances, csm_tile_viewport_scissor, directional_shadow_center,
    directional_shadow_rotation_invariant_fit_with_padding, snapped_directional_shadow_center,
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
    let settings = apply_startup_shadow_overrides(
        world
            .resource::<ShadowSettings>()
            .copied()
            .unwrap_or_default()
            .sanitized(),
    );

    if !settings.enabled || matches!(settings.method, ShadowMethod::None) {
        retire_shadow_rt(this);
        return Ok(LightShadowPlan::disabled(lit.white_texture));
    }

    if let Some(snapshot) = plugin_snapshot {
        this.features
            .light_extraction_providers
            .sync_plugin_capabilities(snapshot);
    }

    let trace_frame = super::trace_policy::should_trace_frame(this.frame.frame_index);
    if trace_frame && newengine_ulog_api::ulog::debug_enabled() {
        newengine_ulog_api::ulog::debug!(
            "render light extraction providers: {}",
            this.features.light_extraction_providers.labels().join(",")
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

    if let Some(plan) = this
        .features
        .light_extraction_providers
        .extract_external_shadow_plan(&ctx)?
    {
        return Ok(plan);
    }

    let command = this
        .features
        .light_extraction_providers
        .extract_runtime_command(&ctx)?;
    if let Some(command) = command {
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

    let Some((rt, shadow_texture, shadow_resolution)) =
        ensure_shadow_rt(this, r, settings.resolution, cascade_count)?
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
        shadow_resolution as f32,
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
        let texel_world = fallback_radius * 2.0 / shadow_resolution.max(1) as f32;
        let stable_half = fallback_radius + (texel_world * kernel_guard_texels).max(0.02);
        let center = snapped_directional_shadow_center(
            fallback_center,
            dir,
            up,
            stable_half,
            stable_half,
            shadow_resolution,
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
                shadow_resolution,
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
    for i in 0..cascade_count as usize {
        let split_near = if i == 0 { 0.5 } else { splits[i - 1] };
        let split_far = splits[i].max(split_near + 0.1);
        // Directional cascades behave like camera-centered clipmaps. Rotation only
        // changes which receivers are visible; it must not translate the sun-shadow
        // projection itself. This removes angle-dependent texel-grid walking/flicker.
        let fallback_radius = (split_far * 1.85).max(8.0);
        let fallback_center = camera;
        let fallback_texel_world = fallback_radius * 2.0 / shadow_resolution.max(1) as f32;
        let fallback_guard = (fallback_texel_world * kernel_guard_texels).max(0.02);
        let fallback_half = fallback_radius + fallback_guard;
        let fit = directional_shadow_rotation_invariant_fit_with_padding(
            viewproj,
            camera,
            camera_forward,
            split_near,
            split_far,
            shadow_resolution,
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
            shadow_resolution,
        );
        let depth_radius = fit.depth_radius.max(fit.half_x.max(fit.half_y)).max(4.0);
        let eye = snapped_center - dir * (depth_radius * 1.95);
        let view = Mat4::look_at_rh(eye, snapped_center, up);
        let near = 0.1;
        let far = depth_radius * 4.35;
        let proj =
            Mat4::orthographic_rh(-fit.half_x, fit.half_x, -fit.half_y, fit.half_y, near, far);
        let cull = ShadowCasterCull::directional(view, fit.half_x.max(fit.half_y), near, far);
        let (viewport, scissor) =
            csm_tile_viewport_scissor(i as u32, cascade_count, shadow_resolution);
        cascades[i] = ShadowCascadeFrame {
            light_mvp: proj * view,
            viewport,
            scissor,
            split_near,
            split_far,
            texel_world_size: ((fit.half_x.max(fit.half_y) * 2.0)
                / shadow_resolution.max(1) as f32)
                .max(1.0e-6),
            caster_cull: cull,
        };
    }

    Ok(Some(
        LightShadowPlan::directional_cascaded(
            rt,
            shadow_texture,
            shadow_resolution,
            cascade_count,
            cascades,
            params,
            extra,
            None,
        )
        .with_pcss(pcss0, pcss1),
    ))
}

#[cfg(test)]
mod startup_shadow_override_tests {
    use super::*;

    #[test]
    fn auto_shadow_overrides_preserve_scene_values() {
        let scene = ShadowSettings {
            resolution: 1024,
            cascade_count: 2,
            method: ShadowMethod::CascadedShadowMaps,
            ..ShadowSettings::default()
        };
        let graphics = newengine_core::StartupGraphicsSettings::default();
        let resolved = apply_shadow_graphics_overrides(scene, &graphics);
        assert_eq!(resolved.resolution, 1024);
        assert_eq!(resolved.cascade_count, 2);
        assert_eq!(resolved.method, ShadowMethod::CascadedShadowMaps);
    }

    #[test]
    fn advanced_prestart_shadow_controls_override_scene_settings() {
        let scene = ShadowSettings::default();
        let graphics = newengine_core::StartupGraphicsSettings {
            shadow_advanced_override: true,
            shadow_filter: newengine_core::ShadowFilterMode::Hard,
            shadow_max_distance: 333.0,
            shadow_softness: 2.25,
            shadow_bias: 0.004,
            shadow_normal_bias: 0.031,
            shadow_contact_strength: 0.72,
            shadow_pcss_blocker_samples: 14,
            shadow_pcss_filter_samples: 15,
            ..Default::default()
        };
        let resolved = apply_shadow_graphics_overrides(scene, &graphics);
        assert_eq!(resolved.filter, ShadowFilter::Hard);
        assert_eq!(resolved.max_distance, 333.0);
        assert_eq!(resolved.softness, 2.25);
        assert_eq!(resolved.bias, 0.004);
        assert_eq!(resolved.normal_bias, 0.031);
        assert_eq!(resolved.contact_strength, 0.72);
        assert_eq!(resolved.pcss.blocker_samples, 14);
        assert_eq!(resolved.pcss.filter_samples, 15);
    }

    #[test]
    fn explicit_shadow_overrides_control_cascades_and_map_size() {
        let scene = ShadowSettings::default();
        let graphics = newengine_core::StartupGraphicsSettings {
            shadow_cascade_count: 4,
            shadow_map_resolution: 4096,
            ..Default::default()
        };
        let resolved = apply_shadow_graphics_overrides(scene, &graphics);
        assert_eq!(resolved.resolution, 4096);
        assert_eq!(resolved.cascade_count, 4);
        assert_eq!(resolved.method, ShadowMethod::CascadedShadowMaps);
    }

    #[test]
    fn prestart_shadow_quality_controls_filter_without_overriding_auto_cascades() {
        let scene = ShadowSettings {
            cascade_count: 3,
            method: ShadowMethod::CascadedShadowMaps,
            filter: ShadowFilter::Pcf,
            ..ShadowSettings::default()
        };
        let graphics = newengine_core::StartupGraphicsSettings {
            shadow_quality: newengine_core::ShadowQuality::Quality,
            shadow_cascade_count: 0,
            ..Default::default()
        };
        let resolved = apply_shadow_graphics_overrides(scene, &graphics);
        assert_eq!(resolved.cascade_count, 3);
        assert_eq!(resolved.method, ShadowMethod::CascadedShadowMaps);
        assert_eq!(resolved.filter, ShadowFilter::Pcss);
        assert!(resolved.pcss.blocker_samples >= 12);
        assert!(resolved.pcss.filter_samples >= 16);
    }

    #[test]
    fn startup_shadow_gate_disables_scene_and_local_shadow_family() {
        let scene = ShadowSettings::default();
        let graphics = newengine_core::StartupGraphicsSettings {
            shadows_enabled: false,
            ..Default::default()
        };
        let resolved = apply_shadow_graphics_overrides(scene, &graphics);
        assert!(!resolved.enabled);
        assert_eq!(resolved.method, ShadowMethod::None);
    }
}
