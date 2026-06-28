#![forbid(unsafe_op_in_unsafe_fn)]

//! Profile-owned render feature provider API.
//!
//! This API is the seam between reusable engine runtime and product/profile
//! render feature packs. Runtime owns lowering and backend submission; feature
//! crates own draw-list extraction and light/shadow policy.

mod lights;
mod shadows;

pub use lights::*;
pub use shadows::*;

use newengine_core::render::{Extent2D, RenderDrawListKind};
use newengine_core::EngineResult;
use newengine_math::{Mat4, Vec3};
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

#[derive(Clone, Copy)]
pub struct SceneExtractionCtx<'a> {
    pub scene: &'a newengine_scene::Scene,
    pub lit: newengine_material_domain_api::LitPipeline,
    pub viewproj: Mat4,
    pub camera_position: Vec3,
    pub camera_forward: Vec3,
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
    fn record_procedural_terrain_gbuffer(&mut self, _ctx: &SceneExtractionCtx<'_>) -> EngineResult<()> {
        Ok(())
    }
    fn record_primitive_mesh_shadow(&mut self, ctx: &SceneExtractionCtx<'_>) -> EngineResult<()>;
    fn record_primitive_mesh_forward(&mut self, ctx: &SceneExtractionCtx<'_>) -> EngineResult<()>;
    fn record_primitive_mesh_gbuffer(&mut self, _ctx: &SceneExtractionCtx<'_>) -> EngineResult<()> {
        Ok(())
    }
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

#[inline]
pub const fn shadow_and_opaque_list(active: bool) -> &'static [RenderDrawListKind] {
    if active { SHADOW_AND_OPAQUE } else { OPAQUE_FORWARD }
}

#[inline]
pub const fn ui_list(active: bool) -> &'static [RenderDrawListKind] {
    if active { UI_LIST } else { EMPTY_LISTS }
}
