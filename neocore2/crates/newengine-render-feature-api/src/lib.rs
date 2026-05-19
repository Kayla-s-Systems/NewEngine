#![forbid(unsafe_op_in_unsafe_fn)]

//! Profile-owned render feature provider API.
//!
//! This API is the seam between reusable engine runtime and product/profile
//! render feature packs. Runtime owns lowering and backend submission; feature
//! crates own draw-list extraction and light/shadow policy.

use newengine_core::render::{Extent2D, RenderDrawListKind, RenderTargetId, TextureId};
use newengine_core::EngineResult;
use newengine_lighting::{AmbientLight, DirectionalLight, PointLight, ShadowSettings};
use newengine_math::{Mat4, Vec3};
use newengine_transform::GlobalTransform;
use newengine_ui::draw::UiDrawList;

pub const PROVIDER_TAG_FEATURE: &str = "feature";
pub const PROVIDER_CAP_DRAW_LISTS: &str = newengine_plugin_api::CAPABILITY_ID_RENDER_DRAW_LIST_PROVIDER;
pub const LIGHT_PROVIDER_TAG_FEATURE: &str = "feature";
pub const LIGHT_PROVIDER_CAP_EXTRACTION: &str = newengine_plugin_api::CAPABILITY_ID_RENDER_LIGHT_EXTRACTION_PROVIDER;

const EMPTY_LISTS: &[RenderDrawListKind] = &[];
const OPAQUE_FORWARD: &[RenderDrawListKind] = &[RenderDrawListKind::OpaqueForward];
const SHADOW_AND_OPAQUE: &[RenderDrawListKind] = &[
    RenderDrawListKind::ShadowCasters,
    RenderDrawListKind::OpaqueForward,
];
const UI_LIST: &[RenderDrawListKind] = &[RenderDrawListKind::Ui];

#[derive(Clone, Copy, Debug)]
pub struct RuntimeVisibilityPlan {
    pub shadow_casters: bool,
    pub opaque_forward: bool,
    pub transparent: bool,
    pub ui: bool,
    pub debug: bool,
}

impl RuntimeVisibilityPlan {
    #[inline]
    pub fn standard(shadow_casters: bool, ui: bool, debug: bool) -> Self {
        Self {
            shadow_casters,
            opaque_forward: true,
            transparent: false,
            ui,
            debug,
        }
    }

    #[inline]
    pub fn allows(&self, kind: RenderDrawListKind) -> bool {
        match kind {
            RenderDrawListKind::ShadowCasters => self.shadow_casters,
            RenderDrawListKind::OpaqueForward => self.opaque_forward,
            RenderDrawListKind::Transparent => self.transparent,
            RenderDrawListKind::Ui => self.ui,
            RenderDrawListKind::Debug => self.debug,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BoundsSnap {
    pub center: Vec3,
    pub radius: f32,
}

const MAX_POINT_LIGHTS: usize = 4;

#[derive(Clone, Copy, Debug)]
pub struct PackedLights {
    pub ambient: [f32; 4],
    pub dir_dir_intensity: [f32; 4],
    pub dir_color: [f32; 4],
    pub point_pos_range: [[f32; 4]; MAX_POINT_LIGHTS],
    pub point_color_intensity: [[f32; 4]; MAX_POINT_LIGHTS],
    pub point_count_pad: [f32; 4],
    pub shadow_light_mvp: Mat4,
    pub shadow_params: [f32; 4],
    pub shadow_extra: [f32; 4],
}

impl Default for PackedLights {
    #[inline]
    fn default() -> Self {
        Self {
            ambient: [0.0, 0.0, 0.0, 0.0],
            dir_dir_intensity: [0.0, -1.0, 0.0, 0.0],
            dir_color: [1.0, 1.0, 1.0, 0.0],
            point_pos_range: [[0.0; 4]; MAX_POINT_LIGHTS],
            point_color_intensity: [[0.0; 4]; MAX_POINT_LIGHTS],
            point_count_pad: [0.0; 4],
            shadow_light_mvp: Mat4::IDENTITY,
            shadow_params: [0.0; 4],
            shadow_extra: [0.0; 4],
        }
    }
}

impl PackedLights {
    pub const UBO_SIZE: usize = 480;

    #[inline]
    pub fn from_world(world: &newengine_ecs::World) -> Self {
        let amb = world.resource::<AmbientLight>().copied().unwrap_or_default();
        let ambient = [amb.color[0], amb.color[1], amb.color[2], amb.intensity];

        let dir = primary_directional_light(world).unwrap_or_default();
        let dir_dir_intensity = [
            dir.direction_ws[0],
            dir.direction_ws[1],
            dir.direction_ws[2],
            dir.intensity,
        ];
        let dir_color = [dir.color[0], dir.color[1], dir.color[2], 0.0];

        let mut pts: Vec<(u64, [f32; 4], [f32; 4])> = Vec::new();
        for (e, pl, gt) in world.query2::<PointLight, GlobalTransform>() {
            let m = gt.0;
            let pos = [m.w_axis.x, m.w_axis.y, m.w_axis.z, pl.range.max(1e-3)];
            let col = [pl.color[0], pl.color[1], pl.color[2], pl.intensity.max(0.0)];
            pts.push((e.stable_u64(), pos, col));
        }
        pts.sort_by(|a, b| a.0.cmp(&b.0));

        if pts.len() > MAX_POINT_LIGHTS {
            log::warn!(
                "render: point lights truncated: requested={} max={} (deterministic keep=min stable id)",
                pts.len(),
                MAX_POINT_LIGHTS
            );
        }

        let mut out = Self {
            ambient,
            dir_dir_intensity,
            dir_color,
            ..Self::default()
        };
        let n = pts.len().min(MAX_POINT_LIGHTS);
        for i in 0..n {
            out.point_pos_range[i] = pts[i].1;
            out.point_color_intensity[i] = pts[i].2;
        }
        out.point_count_pad = [n as f32, 0.0, 0.0, 0.0];
        out
    }

    #[inline]
    pub fn with_camera_position(mut self, camera_position: [f32; 3]) -> Self {
        self.point_count_pad[1] = camera_position[0];
        self.point_count_pad[2] = camera_position[1];
        self.point_count_pad[3] = camera_position[2];
        self
    }

    #[inline]
    pub fn with_shadow(mut self, light_mvp: Mat4, params: [f32; 4], extra: [f32; 4]) -> Self {
        self.shadow_light_mvp = light_mvp;
        self.shadow_params = params;
        self.shadow_extra = extra;
        self
    }

    #[inline]
    pub fn write_into(&self, bytes: &mut [u8; Self::UBO_SIZE]) {
        let mut off = 160;
        fn write_vec4(dst: &mut [u8], off: &mut usize, v: [f32; 4]) {
            for i in 0..4 {
                let o = *off + i * 4;
                dst[o..o + 4].copy_from_slice(&v[i].to_ne_bytes());
            }
            *off += 16;
        }

        write_vec4(bytes, &mut off, self.ambient);
        write_vec4(bytes, &mut off, self.dir_dir_intensity);
        write_vec4(bytes, &mut off, self.dir_color);
        for i in 0..MAX_POINT_LIGHTS {
            write_vec4(bytes, &mut off, self.point_pos_range[i]);
            write_vec4(bytes, &mut off, self.point_color_intensity[i]);
        }
        write_vec4(bytes, &mut off, self.point_count_pad);
    }
}

#[inline]
pub fn primary_directional_light(world: &newengine_ecs::World) -> Option<DirectionalLight> {
    let mut best_dir: Option<(u64, DirectionalLight)> = None;
    for (e, l) in world.query::<DirectionalLight>() {
        let k = e.stable_u64();
        if best_dir.map(|(bk, _)| k < bk).unwrap_or(true) {
            best_dir = Some((k, *l));
        }
    }
    best_dir.map(|(_, l)| l)
}

#[inline]
pub fn primary_point_light(world: &newengine_ecs::World) -> Option<(PointLight, Vec3)> {
    let mut best: Option<(u64, PointLight, Vec3)> = None;
    for (e, l, gt) in world.query2::<PointLight, GlobalTransform>() {
        let k = e.stable_u64();
        let m = gt.0;
        let pos = Vec3::new(m.w_axis.x, m.w_axis.y, m.w_axis.z);
        if best.map(|(bk, _, _)| k < bk).unwrap_or(true) {
            best = Some((k, *l, pos));
        }
    }
    best.map(|(_, l, pos)| (l, pos))
}

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
pub struct ShadowFrame {
    pub texture: TextureId,
    pub light_mvp: Mat4,
    pub params: [f32; 4],
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
            frame: ShadowFrame { texture, light_mvp, params, extra },
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

#[derive(Clone, Copy)]
pub struct SceneExtractionCtx<'a> {
    pub scene: &'a newengine_scene::Scene,
    pub lit: newengine_material_domain_api::LitPipeline,
    pub viewproj: Mat4,
    pub camera_position: Vec3,
    pub bounds: BoundsSnap,
    pub lights: PackedLights,
    pub shadow_plan: LightShadowPlan,
    pub shadow_frame: ShadowFrame,
    pub render_shadow_map: bool,
    pub viewport_extent: Extent2D,
    pub surface_extent: Extent2D,
    pub runtime: bool,
    pub debug_overlays: bool,
    pub ui: Option<&'a UiDrawList>,
}

impl<'a> SceneExtractionCtx<'a> {
    #[inline]
    pub fn visibility(&self) -> RuntimeVisibilityPlan {
        RuntimeVisibilityPlan::standard(
            self.render_shadow_map,
            self.ui.is_some(),
            self.debug_overlays,
        )
    }
}

pub trait DrawListBuildCtx {
    fn record_procedural_terrain_shadow(&mut self, ctx: &SceneExtractionCtx<'_>) -> EngineResult<()>;
    fn record_procedural_terrain_forward(&mut self, ctx: &SceneExtractionCtx<'_>) -> EngineResult<()>;
    fn record_primitive_mesh_shadow(&mut self, ctx: &SceneExtractionCtx<'_>) -> EngineResult<()>;
    fn record_primitive_mesh_forward(&mut self, ctx: &SceneExtractionCtx<'_>) -> EngineResult<()>;
    fn record_ui(&mut self, ctx: &SceneExtractionCtx<'_>) -> EngineResult<()>;
}

#[derive(Clone, Copy, Debug)]
pub struct RenderDrawListProviderMetadata {
    pub id: &'static str,
    pub label: &'static str,
    pub tags: &'static [&'static str],
    pub capabilities: &'static [&'static str],
}

impl RenderDrawListProviderMetadata {
    #[inline]
    pub fn feature(id: &'static str, label: &'static str) -> Self {
        Self {
            id,
            label,
            tags: &[PROVIDER_TAG_FEATURE],
            capabilities: &[PROVIDER_CAP_DRAW_LISTS],
        }
    }
}

pub trait RenderDrawListProvider: Send + Sync {
    fn id(&self) -> &'static str;

    fn metadata(&self) -> RenderDrawListProviderMetadata {
        RenderDrawListProviderMetadata::feature(self.id(), self.id())
    }

    fn provided_draw_lists(&self, ctx: &SceneExtractionCtx<'_>) -> &'static [RenderDrawListKind];

    fn extract(&self, ctx: &SceneExtractionCtx<'_>, out: &mut dyn DrawListBuildCtx) -> EngineResult<()>;
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
    pub world: &'a newengine_ecs::World,
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
        world: &'a newengine_ecs::World,
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
            world,
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

    fn extract(&self, ctx: &LightExtractionCtx<'_>) -> EngineResult<Option<LightExtractionCommand>>;
}

#[inline]
pub const fn shadow_and_opaque_list(active: bool) -> &'static [RenderDrawListKind] {
    if active { SHADOW_AND_OPAQUE } else { OPAQUE_FORWARD }
}

#[inline]
pub const fn ui_list(active: bool) -> &'static [RenderDrawListKind] {
    if active { UI_LIST } else { EMPTY_LISTS }
}
