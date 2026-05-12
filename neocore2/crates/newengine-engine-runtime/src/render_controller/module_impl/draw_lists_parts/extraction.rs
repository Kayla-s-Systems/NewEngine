#[derive(Clone, Copy, Debug)]
pub(super) struct RuntimeVisibilityPlan {
    pub(super) shadow_casters: bool,
    pub(super) opaque_forward: bool,
    pub(super) transparent: bool,
    pub(super) ui: bool,
    pub(super) debug: bool,
}

impl RuntimeVisibilityPlan {
    #[inline]
    pub(super) fn standard(shadow_casters: bool, ui: bool, debug: bool) -> Self {
        Self {
            shadow_casters,
            opaque_forward: true,
            transparent: false,
            ui,
            debug,
        }
    }

    #[inline]
    fn allows(&self, kind: RenderDrawListKind) -> bool {
        match kind {
            RenderDrawListKind::ShadowCasters => self.shadow_casters,
            RenderDrawListKind::OpaqueForward => self.opaque_forward,
            RenderDrawListKind::Transparent => self.transparent,
            RenderDrawListKind::Ui => self.ui,
            RenderDrawListKind::Debug => self.debug,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct SceneExtractionCtx<'a> {
    pub(super) scene: &'a newengine_scene::Scene,
    pub(super) lit: super::super::gpu::LitPipeline,
    pub(super) viewproj: Mat4,
    pub(super) rig: &'a newengine_camera::CameraRig,
    pub(super) bounds: BoundsSnap,
    pub(super) lights: PackedLights,
    pub(super) shadow_plan: LightShadowPlan,
    pub(super) shadow_frame: ShadowFrame,
    pub(super) render_shadow_map: bool,
    pub(super) viewport_extent: Extent2D,
    pub(super) surface_extent: Extent2D,
    pub(super) runtime: bool,
    pub(super) editor_overlays: bool,
    pub(super) ui: Option<&'a UiDrawList>,
}

impl<'a> SceneExtractionCtx<'a> {
    #[inline]
    pub(super) fn visibility(&self) -> RuntimeVisibilityPlan {
        RuntimeVisibilityPlan::standard(
            self.render_shadow_map,
            self.ui.is_some(),
            self.editor_overlays,
        )
    }
}

pub(super) trait RenderDrawListProvider: Send + Sync {
    fn id(&self) -> &'static str;

    fn metadata(&self) -> RenderDrawListProviderMetadata {
        RenderDrawListProviderMetadata::runtime_builtin(self.id(), self.id())
    }

    fn provided_draw_lists(&self, ctx: &SceneExtractionCtx<'_>) -> &'static [RenderDrawListKind];

    fn extract(
        &self,
        ctx: &SceneExtractionCtx<'_>,
        out: &mut DrawListBuildCtx<'_>,
    ) -> EngineResult<()>;
}

#[derive(Clone, Debug)]
pub(super) struct RuntimeDrawListSet {
    lists: Vec<RuntimeDrawList>,
}

impl RuntimeDrawListSet {
    pub(super) fn extract(
        visibility: RuntimeVisibilityPlan,
        ctx: &SceneExtractionCtx<'_>,
        providers: &[&dyn RenderDrawListProvider],
    ) -> Self {
        let mut this = Self { lists: Vec::with_capacity(5) };
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
    pub(super) fn descriptors(&self) -> Vec<DrawListDesc> {
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

    pub(super) fn record_pass_state(
        &self,
        ctx: &SceneExtractionCtx<'_>,
        out: &mut DrawListBuildCtx<'_>,
    ) -> EngineResult<()> {
        if self.contains(RenderDrawListKind::ShadowCasters) && ctx.render_shadow_map {
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

pub(super) struct DrawListBuildCtx<'a> {
    controller: &'a mut RuntimeRenderController,
    render: &'a mut dyn RenderApi,
    lists: &'a RuntimeDrawListSet,
}

impl<'a> DrawListBuildCtx<'a> {
    #[inline]
    pub(super) fn new(
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

    pub(super) fn record<T>(
        &mut self,
        kind: RenderDrawListKind,
        record: impl FnOnce(&mut RuntimeRenderController, &mut dyn RenderApi) -> EngineResult<T>,
    ) -> EngineResult<Option<T>> {
        if !self.lists.contains(kind) {
            return Ok(None);
        }

        let controller = &mut *self.controller;
        let render = &mut *self.render;
        let value = super::record_draw_list(render, kind, |r| record(controller, r))?;
        Ok(Some(value))
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

#[inline]
pub(super) const fn opaque_forward_list(active: bool) -> &'static [RenderDrawListKind] {
    if active { OPAQUE_FORWARD } else { EMPTY_LISTS }
}

#[inline]
pub(super) const fn shadow_and_opaque_list(active: bool) -> &'static [RenderDrawListKind] {
    if active { SHADOW_AND_OPAQUE } else { OPAQUE_FORWARD }
}

#[inline]
pub(super) const fn ui_list(active: bool) -> &'static [RenderDrawListKind] {
    if active { UI_LIST } else { EMPTY_LISTS }
}
