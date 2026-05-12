#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::{
    Extent2D, RenderApi, RenderTargetDesc, RenderTargetId, TextureFormat, TextureId,
};
use newengine_core::EngineResult;
use newengine_lighting::{ShadowMethod, ShadowSettings};
use newengine_math::{Mat4, Vec3};

use super::lights;
use super::light_extraction::LightExtractionCtx;
use super::light_providers::standard_runtime_light_extraction_provider_registry;
use super::scene::BoundsSnap;
use super::RuntimeRenderController;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ShadowLightKind {
    Directional,
    Point,
    Spot,
}

impl ShadowLightKind {
    #[inline]
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Directional => "directional",
            Self::Point => "point",
            Self::Spot => "spot",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ShadowFrame {
    pub texture: TextureId,
    pub light_mvp: Mat4,
    pub params: [f32; 4],
}

impl ShadowFrame {
    #[inline]
    pub(super) fn disabled(fallback: TextureId) -> Self {
        Self {
            texture: fallback,
            light_mvp: Mat4::IDENTITY,
            params: [0.0, 0.0, 0.0, 0.0],
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct LightShadowPlan {
    pub(super) light_kind: Option<ShadowLightKind>,
    pub(super) supported: bool,
    pub(super) target: Option<RenderTargetId>,
    pub(super) resolution: u32,
    pub(super) frame: ShadowFrame,
}

impl LightShadowPlan {
    #[inline]
    pub(super) fn disabled(fallback: TextureId) -> Self {
        Self {
            light_kind: None,
            supported: false,
            target: None,
            resolution: 1,
            frame: ShadowFrame::disabled(fallback),
        }
    }

    #[inline]
    pub(super) fn unsupported(kind: ShadowLightKind, fallback: TextureId, resolution: u32) -> Self {
        Self {
            light_kind: Some(kind),
            supported: false,
            target: None,
            resolution: resolution.max(1),
            frame: ShadowFrame::disabled(fallback),
        }
    }

    #[inline]
    pub(super) fn directional(target: RenderTargetId, texture: TextureId, resolution: u32, light_mvp: Mat4, params: [f32; 4]) -> Self {
        Self {
            light_kind: Some(ShadowLightKind::Directional),
            supported: true,
            target: Some(target),
            resolution: resolution.max(1),
            frame: ShadowFrame {
                texture,
                light_mvp,
                params,
            },
        }
    }

    #[inline]
    pub(super) fn is_active(self) -> bool {
        self.supported && self.target.is_some() && self.frame.params[0] > 0.0
    }

    #[inline]
    pub(super) fn render_target(self) -> Option<RenderTargetId> {
        if self.is_active() { self.target } else { None }
    }

    #[inline]
    pub(super) fn extent(self) -> Extent2D {
        Extent2D::new(self.resolution, self.resolution)
    }
}

#[inline]
pub(super) fn build_light_shadow_plan(
    this: &mut RuntimeRenderController,
    r: &mut dyn RenderApi,
    scene: &newengine_scene::Scene,
    bounds: BoundsSnap,
    lit: super::super::gpu::LitPipeline,
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

    let mut registry = standard_runtime_light_extraction_provider_registry();
    if let Some(snapshot) = plugin_snapshot {
        registry.sync_plugin_capabilities(snapshot);
    }

    let trace_frame = super::trace_policy::should_trace_frame(this.frame_index);
    if trace_frame && log::log_enabled!(log::Level::Debug) {
        log::debug!(
            "render light extraction providers: {}",
            registry.labels().join(",")
        );
    }

    let frame_index = this.frame_index;
    let mut ctx = LightExtractionCtx::new(
        this,
        r,
        world,
        bounds,
        lit,
        settings,
        frame_index,
        viewproj,
        camera_position,
        viewport_extent,
        surface_extent,
    );
    if let Some(plan) = registry.extract_shadow_plan(&mut ctx)? {
        return Ok(plan);
    }

    retire_shadow_rt(&mut *ctx.controller);
    Ok(LightShadowPlan::disabled(ctx.lit.white_texture))
}

#[inline]
pub(super) fn try_build_directional_shadow_plan(
    this: &mut RuntimeRenderController,
    r: &mut dyn RenderApi,
    world: &newengine_ecs::World,
    bounds: BoundsSnap,
    _lit: super::super::gpu::LitPipeline,
    settings: ShadowSettings,
) -> EngineResult<Option<LightShadowPlan>> {
    let Some(dir_light) = lights::primary_directional_light(world) else {
        return Ok(None);
    };

    let Some((rt, shadow_texture)) = ensure_shadow_rt(this, r, settings.resolution)? else {
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
    let proj = Mat4::orthographic_rh(-radius, radius, -radius, radius, 0.1, radius * 4.0);
    let light_mvp = proj * view;
    let params = [
        1.0,
        settings.bias,
        settings.contact_strength.clamp(0.0, super::super::render_quality::SHADOW_STRENGTH_MAX),
        settings.softness.clamp(0.0, super::super::render_quality::SHADOW_SOFTNESS_MAX),
    ];

    Ok(Some(LightShadowPlan::directional(
        rt,
        shadow_texture,
        settings.resolution,
        light_mvp,
        params,
    )))
}

#[inline]
fn ensure_shadow_rt(
    this: &mut RuntimeRenderController,
    r: &mut dyn RenderApi,
    requested_resolution: u32,
) -> EngineResult<Option<(RenderTargetId, TextureId)>> {
    let resolution = requested_resolution.clamp(
        super::super::render_quality::SHADOW_RESOLUTION_MIN,
        super::super::render_quality::SHADOW_RESOLUTION_MAX,
    );
    let recreate = this.shadow_rt.is_none() || this.shadow_rt_resolution != resolution;

    if recreate {
        if let Some(old) = this.shadow_rt.take() {
            this.retire_render_target(old);
        }
        this.shadow_rt_resolution = 0;
        this.invalidate_shadow_cache();
        let rt = r.create_render_target(
            RenderTargetDesc::new(
                Extent2D::new(resolution, resolution),
                super::super::render_quality::SHADOW_MAP_COLOR_FORMAT,
            )
                .with_depth(TextureFormat::Depth32Float)
                .with_label(format!("editor_shadow_map_{resolution}")),
        )?;
        this.shadow_rt = Some(rt);
        this.shadow_rt_resolution = resolution;
    }

    let Some(rt) = this.shadow_rt else {
        return Ok(None);
    };
    let tex = r.render_target_color_texture_id(rt)?;
    Ok(Some((rt, tex)))
}

#[inline]
pub(super) fn retire_shadow_rt(this: &mut RuntimeRenderController) {
    if let Some(old) = this.shadow_rt.take() {
        this.retire_render_target(old);
    }
    this.shadow_rt_resolution = 0;
    this.invalidate_shadow_cache();
}

#[inline]
pub(super) fn warn_unsupported_point_shadow_once(this: &mut RuntimeRenderController) {
    if this.unsupported_point_shadow_warning_emitted {
        return;
    }
    this.unsupported_point_shadow_warning_emitted = true;
    log::warn!(
        "render shadows: PointLight is shadow-capable, but point cube-map shadows are not implemented by this Vulkan path yet; falling back to unshadowed point lighting"
    );
}

#[inline]
pub(super) fn warn_unsupported_spot_shadow_once(this: &mut RuntimeRenderController) {
    if this.unsupported_spot_shadow_warning_emitted {
        return;
    }
    this.unsupported_spot_shadow_warning_emitted = true;
    log::warn!(
        "render shadows: Spot shadow maps are planned, but no SpotLight component/backend path is implemented yet; falling back to unshadowed lighting"
    );
}
