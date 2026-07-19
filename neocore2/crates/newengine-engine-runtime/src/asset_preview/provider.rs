use super::*;

pub(super) struct AssetPreviewDrawListProvider {
    pub(super) api: Arc<AssetPreviewApi>,
}

impl RenderDrawListProvider for AssetPreviewDrawListProvider {
    fn id(&self) -> &'static str {
        "engine.asset_preview.draw_lists"
    }

    fn provided_draw_lists(
        &self,
        _ctx: &SceneExtractionCtx<'_>,
    ) -> &'static [newengine_core::render::RenderDrawListKind] {
        opaque_list(self.api.render_bundle().is_some())
    }

    fn extract(
        &self,
        ctx: &SceneExtractionCtx<'_>,
        out: &mut dyn DrawListBuildCtx,
    ) -> newengine_core::EngineResult<()> {
        let Some(bundle) = self.api.render_bundle() else {
            return Ok(());
        };
        out.record_asset_preview(ctx, &bundle, self.api.camera_view())
    }
}
