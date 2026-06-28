#![forbid(unsafe_op_in_unsafe_fn)]

//! Renderer-neutral packet bridge from `engine.ui` to render draw-list extraction.
//!
//! The active engine.ui provider owns UI tree/layout/input/text. The renderer bridge owns only the
//! packet boundary: if the runtime frame contains a `UiDrawList`, route it to
//! `RenderDrawListKind::Ui`. Concrete backends such as Vulkan consume that
//! draw list through their backend-local UI composite implementation.

use std::sync::Arc;

use newengine_core::EngineResult;
use newengine_render_feature_api::{
    ui_list, DrawListBuildCtx, RenderDrawListProvider, RenderDrawListProviderMetadata,
    SceneExtractionCtx,
};

pub const ENGINE_UI_DRAW_LIST_BRIDGE_PROVIDER_ID: &str = "engine.ui.draw_list_bridge";

#[derive(Default)]
pub struct EngineUiDrawListBridgeProvider;

impl EngineUiDrawListBridgeProvider {
    #[inline]
    pub fn new() -> Self {
        Self
    }

    #[inline]
    pub fn shared() -> Arc<dyn RenderDrawListProvider> {
        Arc::new(Self::new())
    }
}

impl RenderDrawListProvider for EngineUiDrawListBridgeProvider {
    #[inline]
    fn id(&self) -> &'static str {
        ENGINE_UI_DRAW_LIST_BRIDGE_PROVIDER_ID
    }

    #[inline]
    fn metadata(&self) -> RenderDrawListProviderMetadata {
        RenderDrawListProviderMetadata::feature(self.id(), "engine.ui renderer bridge")
    }

    #[inline]
    fn provided_draw_lists(
        &self,
        ctx: &SceneExtractionCtx<'_>,
    ) -> &'static [newengine_core::render::RenderDrawListKind] {
        ui_list(ctx.ui.is_some())
    }

    #[inline]
    fn extract(
        &self,
        ctx: &SceneExtractionCtx<'_>,
        out: &mut dyn DrawListBuildCtx,
    ) -> EngineResult<()> {
        out.record_ui(ctx)
    }
}
