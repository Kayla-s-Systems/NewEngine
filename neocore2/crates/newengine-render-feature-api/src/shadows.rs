#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::{Extent2D, RectI32, RenderTargetId, TextureId, Viewport};
use newengine_core::EngineResult;
use newengine_lighting::ShadowSettings;
use newengine_math::{Mat4, Vec3};

use crate::{
    BoundsSnap, LightSceneSnapshot, LIGHT_PROVIDER_CAP_EXTRACTION, LIGHT_PROVIDER_TAG_FEATURE,
};

pub const MAX_DIRECTIONAL_SHADOW_CASCADES: usize = 4;
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
        p.z <= -self.near + radius_ws && p.z >= -self.far - radius_ws
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ShadowCascadeFrame {
    pub light_mvp: Mat4,
    pub viewport: Viewport,
    pub scissor: RectI32,
    pub split_near: f32,
    pub split_far: f32,
    pub texel_world_size: f32,
    pub caster_cull: ShadowCasterCull,
}

impl ShadowCascadeFrame {
    #[inline]
    pub fn disabled() -> Self {
        Self {
            light_mvp: Mat4::IDENTITY,
            viewport: Viewport {
                x: 0.0,
                y: 0.0,
                w: 1.0,
                h: 1.0,
                min_depth: 0.0,
                max_depth: 1.0,
            },
            scissor: RectI32::new(0, 0, 1, 1),
            split_near: 0.0,
            split_far: 0.0,
            texel_world_size: 1.0,
            caster_cull: ShadowCasterCull::directional(Mat4::IDENTITY, 1.0, 0.1, 1.0),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ShadowFrame {
    pub texture: TextureId,
    pub light_mvp: Mat4,
    pub cascade_light_mvp: [Mat4; MAX_DIRECTIONAL_SHADOW_CASCADES],
    pub cascade_splits: [f32; MAX_DIRECTIONAL_SHADOW_CASCADES],
    pub cascade_count: u32,
    pub cascades: [ShadowCascadeFrame; MAX_DIRECTIONAL_SHADOW_CASCADES],
    pub params: [f32; 4],
    pub extra: [f32; 4],
}

impl ShadowFrame {
    #[inline]
    pub fn disabled(fallback: TextureId) -> Self {
        Self {
            texture: fallback,
            light_mvp: Mat4::IDENTITY,
            cascade_light_mvp: [Mat4::IDENTITY; MAX_DIRECTIONAL_SHADOW_CASCADES],
            cascade_splits: [0.0; MAX_DIRECTIONAL_SHADOW_CASCADES],
            cascade_count: 1,
            cascades: [ShadowCascadeFrame::disabled(); MAX_DIRECTIONAL_SHADOW_CASCADES],
            params: [0.0, 0.0, 0.0, 0.0],
            extra: [0.0, 1.0, 0.0, 0.0],
        }
    }

    #[inline]
    pub fn single(
        texture: TextureId,
        light_mvp: Mat4,
        params: [f32; 4],
        extra: [f32; 4],
        caster_cull: Option<ShadowCasterCull>,
    ) -> Self {
        let cull = caster_cull
            .unwrap_or_else(|| ShadowCasterCull::directional(Mat4::IDENTITY, 1.0, 0.1, 1.0));
        let split_far = extra[3].max(0.0);
        let cascade = ShadowCascadeFrame {
            light_mvp,
            viewport: Viewport {
                x: 0.0,
                y: 0.0,
                w: 1.0,
                h: 1.0,
                min_depth: 0.0,
                max_depth: 1.0,
            },
            scissor: RectI32::new(0, 0, 1, 1),
            split_near: 0.0,
            split_far,
            texel_world_size: 1.0,
            caster_cull: cull,
        };
        let mut cascade_light_mvp = [light_mvp; MAX_DIRECTIONAL_SHADOW_CASCADES];
        cascade_light_mvp[0] = light_mvp;
        let mut cascade_splits = [split_far; MAX_DIRECTIONAL_SHADOW_CASCADES];
        cascade_splits[0] = split_far;
        let mut cascades = [ShadowCascadeFrame::disabled(); MAX_DIRECTIONAL_SHADOW_CASCADES];
        cascades[0] = cascade;
        Self {
            texture,
            light_mvp,
            cascade_light_mvp,
            cascade_splits,
            cascade_count: 1,
            cascades,
            params,
            extra: [extra[0], 1.0, extra[2], extra[3]],
        }
    }

    #[inline]
    pub fn cascaded(
        texture: TextureId,
        cascade_count: u32,
        cascades: [ShadowCascadeFrame; MAX_DIRECTIONAL_SHADOW_CASCADES],
        params: [f32; 4],
        extra: [f32; 4],
    ) -> Self {
        let count = cascade_count.clamp(1, MAX_DIRECTIONAL_SHADOW_CASCADES as u32);
        let first = cascades[0];
        let mut cascade_light_mvp = [first.light_mvp; MAX_DIRECTIONAL_SHADOW_CASCADES];
        let mut cascade_splits = [first.split_far; MAX_DIRECTIONAL_SHADOW_CASCADES];
        for i in 0..count as usize {
            cascade_light_mvp[i] = cascades[i].light_mvp;
            cascade_splits[i] = cascades[i].split_far;
        }
        Self {
            texture,
            light_mvp: first.light_mvp,
            cascade_light_mvp,
            cascade_splits,
            cascade_count: count,
            cascades,
            params,
            extra: [extra[0], count as f32, extra[2], extra[3]],
        }
    }

    #[inline]
    pub fn cascade(self, index: usize) -> ShadowCascadeFrame {
        let max = self
            .cascade_count
            .saturating_sub(1)
            .min((MAX_DIRECTIONAL_SHADOW_CASCADES - 1) as u32) as usize;
        self.cascades[index.min(max)]
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
            frame: ShadowFrame::single(texture, light_mvp, params, extra, caster_cull),
            caster_cull,
        }
    }

    #[inline]
    pub fn directional_cascaded(
        target: RenderTargetId,
        texture: TextureId,
        resolution: u32,
        cascade_count: u32,
        cascades: [ShadowCascadeFrame; MAX_DIRECTIONAL_SHADOW_CASCADES],
        params: [f32; 4],
        extra: [f32; 4],
        caster_cull: Option<ShadowCasterCull>,
    ) -> Self {
        Self {
            light_kind: Some(ShadowLightKind::Directional),
            supported: true,
            target: Some(target),
            resolution: resolution.max(1),
            frame: ShadowFrame::cascaded(texture, cascade_count, cascades, params, extra),
            caster_cull,
        }
    }

    #[inline]
    pub fn is_active(self) -> bool {
        self.supported && self.target.is_some() && self.frame.params[0] > 0.0
    }

    #[inline]
    pub fn render_target(self) -> Option<RenderTargetId> {
        if self.is_active() {
            self.target
        } else {
            None
        }
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
        self.frame
            .cascade_count
            .clamp(1, MAX_DIRECTIONAL_SHADOW_CASCADES as u32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LightExtractionProviderMetadata {
    pub id: &'static str,
    pub label: &'static str,
    pub tags: &'static [&'static str],
    pub capabilities: &'static [&'static str],
}

impl LightExtractionProviderMetadata {
    #[inline]
    pub fn feature(id: &'static str, label: &'static str) -> Self {
        Self {
            id,
            label,
            tags: &[LIGHT_PROVIDER_TAG_FEATURE],
            capabilities: &[LIGHT_PROVIDER_CAP_EXTRACTION],
        }
    }
}

pub struct LightExtractionCtx<'a> {
    pub lights: &'a LightSceneSnapshot,
    pub bounds: BoundsSnap,
    pub lit: newengine_material_domain_api::LitPipeline,
    pub settings: ShadowSettings,
    pub frame_index: u64,
    pub viewproj: Mat4,
    pub camera_position: [f32; 3],
    pub viewport_extent: Extent2D,
    pub surface_extent: Extent2D,
}

impl<'a> LightExtractionCtx<'a> {
    #[allow(clippy::too_many_arguments)]
    #[inline]
    pub fn new(
        lights: &'a LightSceneSnapshot,
        bounds: BoundsSnap,
        lit: newengine_material_domain_api::LitPipeline,
        settings: ShadowSettings,
        frame_index: u64,
        viewproj: Mat4,
        camera_position: [f32; 3],
        viewport_extent: Extent2D,
        surface_extent: Extent2D,
    ) -> Self {
        Self {
            lights,
            bounds,
            lit,
            settings,
            frame_index,
            viewproj,
            camera_position,
            viewport_extent,
            surface_extent,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum LightExtractionCommand {
    DirectionalShadow,
    Unsupported(ShadowLightKind),
    Disabled,
}

pub trait LightExtractionProvider: Send + Sync {
    fn id(&self) -> &'static str;

    fn metadata(&self) -> LightExtractionProviderMetadata {
        LightExtractionProviderMetadata::feature(self.id(), self.id())
    }

    fn supports(&self, ctx: &LightExtractionCtx<'_>) -> bool;

    fn extract(&self, ctx: &LightExtractionCtx<'_>)
        -> EngineResult<Option<LightExtractionCommand>>;
}
