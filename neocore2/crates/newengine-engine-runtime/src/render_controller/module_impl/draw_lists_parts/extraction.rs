use std::collections::BTreeSet;

use newengine_core::render::{
    Extent2D, RectI32, RenderApi, RenderDrawListKind, RenderGraphPassKind, Viewport,
};
use newengine_core::EngineResult;
use newengine_render_feature_api::{
    RenderDrawListProvider, RuntimeVisibilityPlan, SceneExtractionCtx,
};
use newengine_render_frame_graph::DrawListDesc;

use crate::render_controller::RuntimeRenderController;

#[derive(Clone, Debug)]
pub(crate) struct RuntimeDrawListSet {
    lists: Vec<RuntimeDrawList>,
}

impl RuntimeDrawListSet {
    pub(crate) fn extract(
        visibility: RuntimeVisibilityPlan,
        ctx: &SceneExtractionCtx<'_>,
        providers: &[&dyn RenderDrawListProvider],
    ) -> Self {
        let mut this = Self {
            lists: Vec::with_capacity(5),
        };
        for provider in providers {
            for &kind in provider.provided_draw_lists(ctx) {
                if visibility.allows(kind) {
                    this.push(kind);
                }
            }
        }
        this
    }

    #[inline]
    pub(crate) fn descriptors(&self) -> Vec<DrawListDesc> {
        self.lists
            .iter()
            .map(|list| DrawListDesc::standard(list.kind))
            .collect()
    }

    #[inline]
    pub(super) fn kinds(&self) -> BTreeSet<RenderDrawListKind> {
        self.lists.iter().map(|list| list.kind).collect()
    }

    #[inline]
    pub(super) fn contains(&self, kind: RenderDrawListKind) -> bool {
        self.lists.iter().any(|list| list.kind == kind)
    }

    pub(crate) fn record_pass_state(
        &self,
        ctx: &SceneExtractionCtx<'_>,
        out: &mut DrawListBuildCtx<'_>,
    ) -> EngineResult<()> {
        if self.contains(RenderDrawListKind::ShadowCasters)
            && ctx.render_shadow_map
            && ctx.shadow_frame.cascade_count <= 1
        {
            let extent = ctx.shadow_plan.extent();
            let _ = out.record(RenderDrawListKind::ShadowCasters, move |_this, r| {
                r.set_viewport(Viewport::full(extent))?;
                r.set_scissor(RectI32::new(
                    0,
                    0,
                    extent.width as i32,
                    extent.height as i32,
                ))?;
                Ok(())
            })?;
        }

        if self.contains(RenderDrawListKind::OpaqueForward) {
            let extent = ctx.viewport_extent;
            let _ = out.record(RenderDrawListKind::OpaqueForward, move |_this, r| {
                r.set_viewport(Viewport::full(extent))?;
                r.set_scissor(RectI32::new(
                    0,
                    0,
                    extent.width as i32,
                    extent.height as i32,
                ))?;
                Ok(())
            })?;
        }

        Ok(())
    }

    #[inline]
    pub(super) fn push(&mut self, kind: RenderDrawListKind) {
        if !self.contains(kind) {
            self.lists.push(RuntimeDrawList::new(kind));
        }
    }
}

pub(crate) struct DrawListBuildCtx<'a> {
    controller: &'a mut RuntimeRenderController,
    render: &'a mut dyn RenderApi,
    lists: &'a RuntimeDrawListSet,
}

impl<'a> DrawListBuildCtx<'a> {
    #[inline]
    pub(in crate::render_controller::module_impl) fn new(
        controller: &'a mut RuntimeRenderController,
        render: &'a mut dyn RenderApi,
        lists: &'a RuntimeDrawListSet,
    ) -> Self {
        Self {
            controller,
            render,
            lists,
        }
    }

    pub(crate) fn record<T>(
        &mut self,
        kind: RenderDrawListKind,
        record: impl FnOnce(&mut RuntimeRenderController, &mut dyn RenderApi) -> EngineResult<T>,
    ) -> EngineResult<Option<T>> {
        if !self.lists.contains(kind) {
            return Ok(None);
        }

        let controller = &mut *self.controller;
        let render = &mut *self.render;
        let value = super::super::record_draw_list(render, kind, |r| record(controller, r))?;
        Ok(Some(value))
    }

    pub(crate) fn record_shadow_phase<T>(
        &mut self,
        phase: RenderGraphPassKind,
        record: impl FnOnce(&mut RuntimeRenderController, &mut dyn RenderApi) -> EngineResult<T>,
    ) -> EngineResult<Option<T>> {
        if !self.lists.contains(RenderDrawListKind::ShadowCasters) {
            return Ok(None);
        }

        let controller = &mut *self.controller;
        let render = &mut *self.render;
        let value = super::super::record_render_phase(render, phase, |r| record(controller, r))?;
        Ok(Some(value))
    }
}

impl<'a> newengine_render_feature_api::DrawListBuildCtx for DrawListBuildCtx<'a> {
    fn record_procedural_terrain_shadow(
        &mut self,
        ctx: &SceneExtractionCtx<'_>,
    ) -> EngineResult<()> {
        if ctx.shadow_frame.cascade_count > 1 {
            for cascade_index in 0..ctx.shadow_frame.cascade_count as usize {
                let cascade = ctx.shadow_frame.cascade(cascade_index);
                let _ =
                    self.record_shadow_phase(RenderGraphPassKind::ShadowCascadeMap, |this, r| {
                        r.set_viewport(cascade.viewport)?;
                        r.set_scissor(cascade.scissor)?;
                        this.set_shadow_caster_cull(Some(cascade.caster_cull));
                        super::super::passes::draw_procedural_terrain_shadow(
                            this,
                            r,
                            ctx.scene,
                            ctx.lit,
                            cascade.light_mvp,
                            &ctx.lights,
                            ctx.runtime,
                        )
                    })?;
            }
            return Ok(());
        }

        let _ = self.record(RenderDrawListKind::ShadowCasters, |this, r| {
            super::super::passes::draw_procedural_terrain_shadow(
                this,
                r,
                ctx.scene,
                ctx.lit,
                ctx.shadow_frame.light_mvp,
                &ctx.lights,
                ctx.runtime,
            )
        })?;
        Ok(())
    }

    fn record_procedural_terrain_forward(
        &mut self,
        ctx: &SceneExtractionCtx<'_>,
    ) -> EngineResult<()> {
        let _ = self.record(RenderDrawListKind::OpaqueForward, |this, r| {
            super::super::passes::draw_procedural_terrain(
                this,
                r,
                ctx.scene,
                ctx.lit,
                ctx.viewproj,
                &ctx.lights,
                ctx.shadow_frame.texture,
                ctx.runtime,
                ctx.camera_position,
                ctx.camera_forward,
            )
        })?;
        Ok(())
    }

    fn record_procedural_terrain_gbuffer(
        &mut self,
        ctx: &SceneExtractionCtx<'_>,
    ) -> EngineResult<()> {
        let _ = self.record_shadow_phase(RenderGraphPassKind::GBuffer, |this, r| {
            r.set_viewport(Viewport::full(ctx.viewport_extent))?;
            r.set_scissor(RectI32::new(
                0,
                0,
                ctx.viewport_extent.width as i32,
                ctx.viewport_extent.height as i32,
            ))?;
            super::super::passes::draw_procedural_terrain_gbuffer(
                this,
                r,
                ctx.scene,
                ctx.lit,
                ctx.viewproj,
                &ctx.lights,
                ctx.runtime,
                ctx.camera_position,
                ctx.camera_forward,
            )
        })?;
        Ok(())
    }

    fn record_primitive_mesh_shadow(&mut self, ctx: &SceneExtractionCtx<'_>) -> EngineResult<()> {
        if ctx.shadow_frame.cascade_count > 1 {
            for cascade_index in 0..ctx.shadow_frame.cascade_count as usize {
                let cascade = ctx.shadow_frame.cascade(cascade_index);
                let _ =
                    self.record_shadow_phase(RenderGraphPassKind::ShadowCascadeMap, |this, r| {
                        r.set_viewport(cascade.viewport)?;
                        r.set_scissor(cascade.scissor)?;
                        this.set_shadow_caster_cull(Some(cascade.caster_cull));
                        super::super::passes::draw_primitives_shadow(
                            this,
                            r,
                            ctx.scene,
                            ctx.lit,
                            cascade.light_mvp,
                            &ctx.lights,
                            ctx.runtime,
                            ctx.camera_position,
                        )
                    })?;
            }
            return Ok(());
        }

        let _ = self.record(RenderDrawListKind::ShadowCasters, |this, r| {
            super::super::passes::draw_primitives_shadow(
                this,
                r,
                ctx.scene,
                ctx.lit,
                ctx.shadow_frame.light_mvp,
                &ctx.lights,
                ctx.runtime,
                ctx.camera_position,
            )
        })?;
        Ok(())
    }

    fn record_primitive_mesh_gbuffer(&mut self, ctx: &SceneExtractionCtx<'_>) -> EngineResult<()> {
        let _ = self.record_shadow_phase(RenderGraphPassKind::GBuffer, |this, r| {
            r.set_viewport(Viewport::full(ctx.viewport_extent))?;
            r.set_scissor(RectI32::new(
                0,
                0,
                ctx.viewport_extent.width as i32,
                ctx.viewport_extent.height as i32,
            ))?;
            super::super::passes::draw_primitives_gbuffer(
                this,
                r,
                ctx.scene,
                ctx.lit,
                ctx.viewproj,
                &ctx.lights,
                ctx.runtime,
                ctx.camera_position,
                ctx.camera_forward,
                ctx.deferred,
            )
        })?;
        Ok(())
    }

    fn record_primitive_mesh_forward(&mut self, ctx: &SceneExtractionCtx<'_>) -> EngineResult<()> {
        let _ = self.record(RenderDrawListKind::OpaqueForward, |this, r| {
            super::super::passes::draw_primitives(
                this,
                r,
                ctx.scene,
                ctx.lit,
                ctx.viewproj,
                &ctx.lights,
                ctx.shadow_frame.texture,
                ctx.runtime,
                ctx.camera_position,
                ctx.camera_forward,
                ctx.deferred,
            )
        })?;
        Ok(())
    }

    fn record_asset_preview(
        &mut self,
        ctx: &SceneExtractionCtx<'_>,
        bundle: &newengine_model_domain_api::ModelAssetBundle,
        view: newengine_render_feature_api::AssetPreviewView,
    ) -> EngineResult<()> {
        let _ = self.record(RenderDrawListKind::OpaqueForward, |this, r| {
            super::super::passes::draw_asset_preview_bundle(
                this,
                r,
                bundle,
                ctx.lit,
                ctx.viewport_extent,
                view,
            )
        })?;
        Ok(())
    }

    fn record_ui(&mut self, ctx: &SceneExtractionCtx<'_>) -> EngineResult<()> {
        let Some(ui) = ctx.ui else {
            return Ok(());
        };
        let extent: Extent2D = ctx.surface_extent;
        let _ = self.record(RenderDrawListKind::Ui, |_this, r| {
            r.set_viewport(Viewport::full(extent))?;
            r.set_scissor(RectI32::new(
                0,
                0,
                extent.width as i32,
                extent.height as i32,
            ))?;
            r.set_ui_draw_list(ui.clone());
            Ok(())
        })?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct RuntimeDrawList {
    kind: RenderDrawListKind,
}

impl RuntimeDrawList {
    #[inline]
    fn new(kind: RenderDrawListKind) -> Self {
        Self { kind }
    }
}
