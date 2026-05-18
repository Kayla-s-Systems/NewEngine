#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::{
    Extent2D, RenderApi, RenderTargetDesc, RenderTargetId, TextureFormat, TextureId,
};
use newengine_core::EngineResult;
use newengine_lighting::{ShadowMethod, ShadowSettings};
use newengine_math::{Mat4, Vec3};

use super::lights;
use super::light_extraction::LightExtractionCtx;
use super::scene::BoundsSnap;
use crate::render_controller::RuntimeRenderController;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShadowLightKind {
    Directional,
    Point,
    Spot,
}

impl ShadowLightKind {
    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Directional => "sun_directional",
            Self::Point => "point",
            Self::Spot => "spot",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ShadowCasterCull {
    pub light_view: Mat4,
    pub half_extent_xy: f32,
    pub near: f32,
    pub far: f32,
}

impl ShadowCasterCull {
    #[inline]
    pub fn directional(light_view: Mat4, half_extent_xy: f32, near: f32, far: f32) -> Self {
        Self {
            light_view,
            half_extent_xy: half_extent_xy.max(0.001),
            near: near.max(0.001),
            far: far.max(near.max(0.001) + 0.001),
        }
    }

    #[inline]
    pub fn contains_sphere(self, center_ws: Vec3, radius_ws: f32) -> bool {
        let radius_ws = radius_ws.abs().max(0.001);
        let p = self.light_view.transform_point3(center_ws);
        if p.x.abs() > self.half_extent_xy + radius_ws {
            return false;
        }
        if p.y.abs() > self.half_extent_xy + radius_ws {
            return false;
        }
        // Right-handed look_at shadow view looks down -Z; visible range is [-far, -near].
        p.z <= -self.near + radius_ws && p.z >= -self.far - radius_ws
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ShadowFrame {
    pub texture: TextureId,
    pub light_mvp: Mat4,
    /// x: enabled, y: receiver depth bias, z: contact strength, w: PCF softness.
    pub params: [f32; 4],
    /// x: normal bias in shadow-depth units, y: cascade count, z/w: reserved for atlas/cascade metadata.
    pub extra: [f32; 4],
}

impl ShadowFrame {
    #[inline]
    pub fn disabled(fallback: TextureId) -> Self {
        Self {
            texture: fallback,
            light_mvp: Mat4::IDENTITY,
            params: [0.0, 0.0, 0.0, 0.0],
            extra: [0.0, 0.0, 0.0, 0.0],
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LightShadowPlan {
    pub light_kind: Option<ShadowLightKind>,
    pub supported: bool,
    pub target: Option<RenderTargetId>,
    pub resolution: u32,
    pub frame: ShadowFrame,
    pub caster_cull: Option<ShadowCasterCull>,
}

impl LightShadowPlan {
    #[inline]
    pub fn disabled(fallback: TextureId) -> Self {
        Self {
            light_kind: None,
            supported: false,
            target: None,
            resolution: 1,
            frame: ShadowFrame::disabled(fallback),
            caster_cull: None,
        }
    }

    #[inline]
    pub fn unsupported(kind: ShadowLightKind, fallback: TextureId, resolution: u32) -> Self {
        Self {
            light_kind: Some(kind),
            supported: false,
            target: None,
            resolution: resolution.max(1),
            frame: ShadowFrame::disabled(fallback),
            caster_cull: None,
        }
    }

    #[inline]
    pub fn directional(
        target: RenderTargetId,
        texture: TextureId,
        resolution: u32,
        light_mvp: Mat4,
        params: [f32; 4],
        extra: [f32; 4],
        caster_cull: Option<ShadowCasterCull>,
    ) -> Self {
        Self {
            light_kind: Some(ShadowLightKind::Directional),
            supported: true,
            target: Some(target),
            resolution: resolution.max(1),
            frame: ShadowFrame {
                texture,
                light_mvp,
                params,
                extra,
            },
            caster_cull,
        }
    }

    #[inline]
    pub fn is_active(self) -> bool {
        self.supported && self.target.is_some() && self.frame.params[0] > 0.0
    }

    #[inline]
    pub fn render_target(self) -> Option<RenderTargetId> {
        if self.is_active() { self.target } else { None }
    }

    #[inline]
    pub fn extent(self) -> Extent2D {
        let cascades = self.cascade_count();
        if cascades <= 1 {
            return Extent2D::new(self.resolution, self.resolution);
        }
        let columns = if cascades <= 4 { 2 } else { 4 };
        let rows = ((cascades + columns - 1) / columns).max(1);
        Extent2D::new(
            self.resolution.saturating_mul(columns),
            self.resolution.saturating_mul(rows),
        )
    }

    #[inline]
    pub fn cascade_count(self) -> u32 {
        self.frame.extra[1].round().clamp(1.0, 8.0) as u32
    }
}

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

    let frame_index = this.frame.frame_index;
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
