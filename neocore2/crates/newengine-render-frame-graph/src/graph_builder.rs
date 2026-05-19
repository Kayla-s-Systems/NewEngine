use newengine_render_api::{
    Extent2D, RenderGraphDesc, RenderGraphPassDesc, RenderGraphResourceDesc,
    RenderGraphPassDomain, RenderGraphResourceId, RenderGraphResourceSemantic, RenderGraphResourceUsage, RenderTargetId, TextureFormat,
};

use crate::{
    DrawListDesc, DrawListKind, FramePlanExecutionMode, RenderFramePlan, RenderFrameRecipe,
    RenderPhaseDesc, RuntimeRecipeBuildParams, StandardRenderPhase,
};

pub const RG_SURFACE_COLOR: RenderGraphResourceId = RenderGraphResourceId(1);
pub const RG_VIEWPORT_COLOR: RenderGraphResourceId = RenderGraphResourceId(2);
pub const RG_VIEWPORT_DEPTH: RenderGraphResourceId = RenderGraphResourceId(3);
/// Linear scene color target. World shaders write lighting here without display encoding.
pub const RG_SCENE_HDR_COLOR: RenderGraphResourceId = RenderGraphResourceId(4);
pub const RG_SHADOW_MAP: RenderGraphResourceId = RenderGraphResourceId(10);
pub const RG_GBUFFER_ALBEDO: RenderGraphResourceId = RenderGraphResourceId(20);
pub const RG_GBUFFER_NORMAL: RenderGraphResourceId = RenderGraphResourceId(21);
pub const RG_GBUFFER_MATERIAL: RenderGraphResourceId = RenderGraphResourceId(22);
pub const RG_GBUFFER_DEPTH: RenderGraphResourceId = RenderGraphResourceId(23);
pub const RG_LIT_COLOR: RenderGraphResourceId = RenderGraphResourceId(30);

#[derive(Debug, Clone, Copy)]
pub struct FrameGraphTargetDesc {
    pub surface_extent: Extent2D,
    pub viewport_extent: Extent2D,
    pub viewport_is_surface: bool,
    pub viewport_render_target: Option<RenderTargetId>,
    pub shadow_render_target: Option<RenderTargetId>,
    /// Final display/swapchain format. Keep LDR unless the platform exposes HDR swapchains.
    pub color_format: TextureFormat,
    /// Linear scene color format used by HDR-capable world/material shaders.
    pub scene_color_format: TextureFormat,
    pub depth_format: TextureFormat,
    pub hdr_scene_enabled: bool,
}

impl FrameGraphTargetDesc {
    #[inline]
    pub fn new(surface_extent: Extent2D, viewport_extent: Extent2D, viewport_is_surface: bool) -> Self {
        Self {
            surface_extent,
            viewport_extent,
            viewport_is_surface,
            viewport_render_target: None,
            shadow_render_target: None,
            color_format: TextureFormat::Bgra8Unorm,
            scene_color_format: TextureFormat::Rgba16Float,
            depth_format: TextureFormat::Depth32Float,
            hdr_scene_enabled: true,
        }
    }

    #[inline]
    pub fn with_viewport_render_target(mut self, target: Option<RenderTargetId>) -> Self {
        self.viewport_render_target = target;
        self
    }

    #[inline]
    pub fn with_shadow_render_target(mut self, target: Option<RenderTargetId>) -> Self {
        self.shadow_render_target = target;
        self
    }

    #[inline]
    pub fn with_hdr_scene(mut self, enabled: bool) -> Self {
        self.hdr_scene_enabled = enabled;
        self
    }

    #[inline]
    pub fn with_scene_color_format(mut self, format: TextureFormat) -> Self {
        self.scene_color_format = format;
        self
    }
}

#[derive(Debug, Clone)]
pub struct FrameGraphBuilder {
    graph: RenderGraphDesc,
    phases: Vec<RenderPhaseDesc>,
    draw_lists: Vec<DrawListDesc>,
    target: FrameGraphTargetDesc,
    execution_mode: FramePlanExecutionMode,
    next_custom_pass: u64,
}

impl FrameGraphBuilder {
    #[inline]
    pub fn new(label: impl Into<String>, frame_index: u64, target: FrameGraphTargetDesc) -> Self {
        let mut graph = RenderGraphDesc::new(label);
        graph.frame_index = frame_index;

        let mut this = Self {
            graph,
            phases: vec![RenderPhaseDesc::standard(StandardRenderPhase::BeginFrame)],
            draw_lists: Vec::new(),
            target,
            execution_mode: FramePlanExecutionMode::ImmediateCallbacks,
            next_custom_pass: 10_000,
        };
        this.add_standard_external_resources();
        this
    }

    #[inline]
    pub fn execution_mode(mut self, mode: FramePlanExecutionMode) -> Self {
        self.execution_mode = mode;
        self
    }

    #[inline]
    pub fn draw_list(mut self, list: DrawListDesc) -> Self {
        if let Some(existing) = self.draw_lists.iter_mut().find(|it| it.kind == list.kind) {
            *existing = list;
        } else {
            self.draw_lists.push(list);
        }
        self
    }

    #[inline]
    pub fn draw_lists(mut self, lists: impl IntoIterator<Item = DrawListDesc>) -> Self {
        for list in lists {
            self = self.draw_list(list);
        }
        self
    }


    pub fn apply_runtime_recipe(
        mut self,
        recipe: &RenderFrameRecipe,
        params: RuntimeRecipeBuildParams,
    ) -> Self {
        for phase in recipe.enabled_phases() {
            self = match phase {
                StandardRenderPhase::BeginFrame | StandardRenderPhase::EndFrame => self,
                StandardRenderPhase::ShadowMap => self.shadow_map(true, params.shadow_resolution),
                StandardRenderPhase::ShadowCascadeMap => self.shadow_cascade_map(true, params.shadow_resolution, params.shadow_cascade_count),
                StandardRenderPhase::TessellationPrepare => self.tessellation_prepare(),
                StandardRenderPhase::DepthPrepass => self.depth_prepass(),
                StandardRenderPhase::ViewportGBuffer => self.gbuffer(),
                StandardRenderPhase::DeferredLighting => self.deferred_lighting(),
                StandardRenderPhase::ViewportForward => self.forward_opaque(),
                StandardRenderPhase::Transparent => self.transparent(),
                StandardRenderPhase::Water => self.water(),
                StandardRenderPhase::PostFx => self.postfx(true),
                StandardRenderPhase::BloomExtract => self.bloom_extract(),
                StandardRenderPhase::BloomBlur => self.bloom_blur(),
                StandardRenderPhase::TaaResolve => self.taa_resolve(),
                StandardRenderPhase::MsaaResolve => self.msaa_resolve(),
                StandardRenderPhase::UiBackdropBlur => self.ui_backdrop_blur(true),
                StandardRenderPhase::UiComposite => self.ui_composite(true),
                StandardRenderPhase::DebugOverlay => self.debug_overlay(true),
            };
        }
        self
    }

    #[inline]
    pub fn shadow_map(mut self, enabled: bool, resolution: u32) -> Self {
        if !enabled {
            return self;
        }
        let resolution = resolution.clamp(256, 8192);
        let shadow_extent = Extent2D::new(resolution, resolution);
        let shadow_resource = if let Some(rt) = self.target.shadow_render_target {
            RenderGraphResourceDesc::external_render_target(
                RG_SHADOW_MAP,
                "shadow_map",
                rt,
                RenderGraphResourceUsage::DepthAttachment,
                shadow_extent,
                self.target.depth_format,
            )
            .with_semantic(RenderGraphResourceSemantic::ShadowMap)
        } else {
            RenderGraphResourceDesc::transient_texture(
                RG_SHADOW_MAP,
                "shadow_map",
                RenderGraphResourceUsage::DepthAttachment,
                shadow_extent,
                self.target.depth_format,
            )
            .with_semantic(RenderGraphResourceSemantic::ShadowMap)
        };
        self.graph.resources.push(shadow_resource);
        self.add_phase_pass(StandardRenderPhase::ShadowMap, |pass| {
            pass.with_domain(RenderGraphPassDomain::Render3d).writes(RG_SHADOW_MAP, RenderGraphResourceUsage::DepthAttachment)
                .draw_list(DrawListKind::ShadowCasters)
        });
        self
    }

    #[inline]
    pub fn shadow_cascade_map(mut self, enabled: bool, resolution: u32, cascade_count: u32) -> Self {
        if !enabled {
            return self;
        }
        let resolution = resolution.clamp(256, 8192);
        let cascades = cascade_count.clamp(1, 8);
        let atlas_cols = if cascades <= 1 { 1 } else if cascades <= 4 { 2 } else { 4 };
        let atlas_rows = ((cascades + atlas_cols - 1) / atlas_cols).max(1);
        let shadow_extent = Extent2D::new(resolution.saturating_mul(atlas_cols), resolution.saturating_mul(atlas_rows));
        let shadow_resource = if let Some(rt) = self.target.shadow_render_target {
            RenderGraphResourceDesc::external_render_target(
                RG_SHADOW_MAP,
                "shadow_cascade_atlas",
                rt,
                RenderGraphResourceUsage::DepthAttachment,
                shadow_extent,
                self.target.depth_format,
            )
            .with_semantic(RenderGraphResourceSemantic::ShadowMap)
        } else {
            RenderGraphResourceDesc::transient_texture(
                RG_SHADOW_MAP,
                "shadow_cascade_atlas",
                RenderGraphResourceUsage::DepthAttachment,
                shadow_extent,
                self.target.depth_format,
            )
            .with_semantic(RenderGraphResourceSemantic::ShadowMap)
        };
        if !self.has_resource(RG_SHADOW_MAP) {
            self.graph.resources.push(shadow_resource);
        }
        self.add_phase_pass(StandardRenderPhase::ShadowCascadeMap, |pass| {
            pass.with_domain(RenderGraphPassDomain::Render3d).writes(RG_SHADOW_MAP, RenderGraphResourceUsage::DepthAttachment)
                .draw_list(DrawListKind::ShadowCasters)
        });
        self
    }

    #[inline]
    pub fn tessellation_prepare(mut self) -> Self {
        self.add_phase_pass(StandardRenderPhase::TessellationPrepare, |pass| pass);
        self
    }

    #[inline]
    pub fn viewport_gbuffer_or_forward(mut self, deferred: bool) -> Self {
        if deferred {
            self = self.depth_prepass();
            self = self.gbuffer();
        } else {
            self = self.forward_opaque();
        }
        self
    }

    #[inline]
    pub fn lighting(mut self, deferred: bool) -> Self {
        if deferred {
            self = self.deferred_lighting();
        }
        self
    }

    #[inline]
    pub fn deferred_lighting(mut self) -> Self {
        self.graph.resources.push(RenderGraphResourceDesc::transient_texture(
            RG_LIT_COLOR,
            "lit_color",
            RenderGraphResourceUsage::ColorAttachment,
            self.target.viewport_extent,
            self.target.scene_color_format,
        )
        .with_semantic(RenderGraphResourceSemantic::LitColor));
        self.add_phase_pass(StandardRenderPhase::DeferredLighting, |pass| {
            pass.with_domain(RenderGraphPassDomain::Render3d).reads(RG_GBUFFER_ALBEDO, RenderGraphResourceUsage::SampledTexture)
                .reads(RG_GBUFFER_NORMAL, RenderGraphResourceUsage::SampledTexture)
                .reads(RG_GBUFFER_MATERIAL, RenderGraphResourceUsage::SampledTexture)
                .reads(RG_GBUFFER_DEPTH, RenderGraphResourceUsage::SampledTexture)
                .writes(RG_LIT_COLOR, RenderGraphResourceUsage::ColorAttachment)
                // Forward-compatible bridge: until native MRT GBuffer material shaders land,
                // replay opaque commands in the lighting pass so enabling deferred graph does
                // not produce a blank frame. The pass/target is still distinct and observable.
                .draw_list(DrawListKind::OpaqueForward)
        });
        self
    }

    #[inline]
    pub fn transparent(mut self) -> Self {
        let viewport_color = self.viewport_color_resource();
        self.add_phase_pass(StandardRenderPhase::Transparent, |pass| {
            pass.with_domain(RenderGraphPassDomain::Render3d).reads(RG_VIEWPORT_DEPTH, RenderGraphResourceUsage::DepthAttachment)
                .writes(viewport_color, RenderGraphResourceUsage::ColorAttachment)
                .draw_list(DrawListKind::Transparent)
        });
        self
    }

    #[inline]
    pub fn water(mut self) -> Self {
        let viewport_color = self.viewport_color_resource();
        self.add_phase_pass(StandardRenderPhase::Water, |pass| {
            pass.with_domain(RenderGraphPassDomain::Render3d).reads(RG_VIEWPORT_DEPTH, RenderGraphResourceUsage::DepthAttachment)
                .writes(viewport_color, RenderGraphResourceUsage::ColorAttachment)
        });
        self
    }

    #[inline]
    pub fn postfx(mut self, enabled: bool) -> Self {
        if !enabled {
            return self;
        }

        let input = if self.has_resource(RG_LIT_COLOR) {
            RG_LIT_COLOR
        } else {
            RG_SCENE_HDR_COLOR
        };
        self.add_phase_pass(StandardRenderPhase::PostFx, |pass| {
            pass.with_domain(RenderGraphPassDomain::PostProcess).reads(input, RenderGraphResourceUsage::SampledTexture)
                .writes(RG_SURFACE_COLOR, RenderGraphResourceUsage::ColorAttachment)
        });
        self
    }

    #[inline]
    pub fn bloom_extract(mut self) -> Self {
        let input = if self.has_resource(RG_LIT_COLOR) { RG_LIT_COLOR } else { RG_SCENE_HDR_COLOR };
        self.add_phase_pass(StandardRenderPhase::BloomExtract, |pass| {
            pass.with_domain(RenderGraphPassDomain::PostProcess).reads(input, RenderGraphResourceUsage::SampledTexture)
                .writes(RG_SURFACE_COLOR, RenderGraphResourceUsage::ColorAttachment)
        });
        self
    }

    #[inline]
    pub fn bloom_blur(mut self) -> Self {
        self.add_phase_pass(StandardRenderPhase::BloomBlur, |pass| {
            pass.with_domain(RenderGraphPassDomain::PostProcess).reads(RG_SURFACE_COLOR, RenderGraphResourceUsage::SampledTexture)
                .writes(RG_SURFACE_COLOR, RenderGraphResourceUsage::ColorAttachment)
        });
        self
    }

    #[inline]
    pub fn taa_resolve(mut self) -> Self {
        self.add_phase_pass(StandardRenderPhase::TaaResolve, |pass| {
            pass.with_domain(RenderGraphPassDomain::PostProcess).reads(RG_SURFACE_COLOR, RenderGraphResourceUsage::SampledTexture)
                .writes(RG_SURFACE_COLOR, RenderGraphResourceUsage::ColorAttachment)
        });
        self
    }

    #[inline]
    pub fn msaa_resolve(mut self) -> Self {
        self.add_phase_pass(StandardRenderPhase::MsaaResolve, |pass| {
            pass.with_domain(RenderGraphPassDomain::PostProcess).reads(RG_SURFACE_COLOR, RenderGraphResourceUsage::SampledTexture)
                .writes(RG_SURFACE_COLOR, RenderGraphResourceUsage::ColorAttachment)
        });
        self
    }

    #[inline]
    pub fn ui_backdrop_blur(mut self, enabled: bool) -> Self {
        if !enabled {
            return self;
        }
        let input = if self.has_resource(RG_LIT_COLOR) { RG_LIT_COLOR } else { RG_SCENE_HDR_COLOR };
        self.add_phase_pass(StandardRenderPhase::UiBackdropBlur, |pass| {
            pass.with_domain(RenderGraphPassDomain::Render2d)
                .reads(input, RenderGraphResourceUsage::SampledTexture)
                .writes(RG_SURFACE_COLOR, RenderGraphResourceUsage::ColorAttachment)
        });
        self
    }

    #[inline]
    pub fn ui_composite(mut self, enabled: bool) -> Self {
        if !enabled {
            return self;
        }
        self.add_phase_pass(StandardRenderPhase::UiComposite, |pass| {
            pass.with_domain(RenderGraphPassDomain::Render2d).writes(RG_SURFACE_COLOR, RenderGraphResourceUsage::ColorAttachment)
                .draw_list(DrawListKind::Ui)
        });
        self
    }

    #[inline]
    pub fn debug_overlay(mut self, enabled: bool) -> Self {
        if enabled {
            self.add_phase_pass(StandardRenderPhase::DebugOverlay, |pass| {
                pass.with_domain(RenderGraphPassDomain::Render2d).reads(RG_SURFACE_COLOR, RenderGraphResourceUsage::ColorAttachment)
                    .writes(RG_SURFACE_COLOR, RenderGraphResourceUsage::ColorAttachment)
                    .draw_list(DrawListKind::Debug)
            });
        }
        self
    }

    #[inline]
    pub fn submit(mut self) -> RenderFramePlan {
        self.phases.push(RenderPhaseDesc::standard(StandardRenderPhase::EndFrame));
        let mut plan = RenderFramePlan::new(self.graph);
        plan.phases = self.phases;
        plan.draw_lists = self.draw_lists;
        plan.execution_mode = self.execution_mode;
        plan
    }

    fn add_standard_external_resources(&mut self) {
        self.graph.resources.push(RenderGraphResourceDesc::external_swapchain(
            RG_SURFACE_COLOR,
            "swapchain_surface_color",
            RenderGraphResourceUsage::ColorAttachment,
            self.target.surface_extent,
            self.target.color_format,
        )
        .with_semantic(RenderGraphResourceSemantic::SurfaceColor));

        if self.target.hdr_scene_enabled {
            self.graph.resources.push(RenderGraphResourceDesc::transient_texture(
                RG_SCENE_HDR_COLOR,
                "scene_hdr_color",
                RenderGraphResourceUsage::ColorAttachment,
                self.target.viewport_extent,
                self.target.scene_color_format,
            )
            .with_semantic(RenderGraphResourceSemantic::SceneHdrColor));

            // HDR scene rendering is offscreen even when the viewport is the native surface.
            // The world pass must therefore own a matching depth attachment in the same
            // native render scope as scene_hdr_color. Binding RG_VIEWPORT_DEPTH to the
            // swapchain/external depth here makes the forward material pipelines use a
            // color+depth render pass while the actual offscreen target is color-only,
            // which can silently produce an empty scene before postFX composition.
            self.graph.resources.push(RenderGraphResourceDesc::transient_texture(
                RG_VIEWPORT_DEPTH,
                "scene_hdr_depth",
                RenderGraphResourceUsage::DepthAttachment,
                self.target.viewport_extent,
                self.target.depth_format,
            )
            .with_semantic(RenderGraphResourceSemantic::ViewportDepth));
        }

        if self.target.viewport_is_surface {
            self.graph.resources.push(RenderGraphResourceDesc::external_swapchain(
                RG_VIEWPORT_COLOR,
                "viewport_surface_color",
                RenderGraphResourceUsage::ColorAttachment,
                self.target.surface_extent,
                self.target.color_format,
            )
            .with_semantic(RenderGraphResourceSemantic::ViewportColor));
            if !self.target.hdr_scene_enabled {
                self.graph.resources.push(RenderGraphResourceDesc::external_swapchain(
                    RG_VIEWPORT_DEPTH,
                    "viewport_surface_depth",
                    RenderGraphResourceUsage::DepthAttachment,
                    self.target.surface_extent,
                    self.target.depth_format,
                )
                .with_semantic(RenderGraphResourceSemantic::ViewportDepth));
            }
        } else if let Some(rt) = self.target.viewport_render_target {
            self.graph.resources.push(RenderGraphResourceDesc::external_render_target(
                RG_VIEWPORT_COLOR,
                "viewport_render_target_color",
                rt,
                RenderGraphResourceUsage::ColorAttachment,
                self.target.viewport_extent,
                self.target.color_format,
            )
            .with_semantic(RenderGraphResourceSemantic::ViewportColor));
            if !self.target.hdr_scene_enabled {
                self.graph.resources.push(RenderGraphResourceDesc::external_render_target(
                    RG_VIEWPORT_DEPTH,
                    "viewport_render_target_depth",
                    rt,
                    RenderGraphResourceUsage::DepthAttachment,
                    self.target.viewport_extent,
                    self.target.depth_format,
                )
                .with_semantic(RenderGraphResourceSemantic::ViewportDepth));
            }
        } else {
            self.graph.resources.push(RenderGraphResourceDesc::external(
                RG_VIEWPORT_COLOR,
                "viewport_render_target_color",
                RenderGraphResourceUsage::ColorAttachment,
            )
            .with_semantic(RenderGraphResourceSemantic::ViewportColor));
            if !self.target.hdr_scene_enabled {
                self.graph.resources.push(RenderGraphResourceDesc::external(
                    RG_VIEWPORT_DEPTH,
                    "viewport_depth",
                    RenderGraphResourceUsage::DepthAttachment,
                )
                .with_semantic(RenderGraphResourceSemantic::ViewportDepth));
            }
        }
    }

    pub fn depth_prepass(mut self) -> Self {
        self.graph.resources.push(RenderGraphResourceDesc::transient_texture(
            RG_GBUFFER_DEPTH,
            "gbuffer_depth",
            RenderGraphResourceUsage::DepthAttachment,
            self.target.viewport_extent,
            self.target.depth_format,
        )
        .with_semantic(RenderGraphResourceSemantic::GBufferDepth));
        self.add_phase_pass(StandardRenderPhase::DepthPrepass, |pass| {
            pass.with_domain(RenderGraphPassDomain::Render3d).writes(RG_GBUFFER_DEPTH, RenderGraphResourceUsage::DepthAttachment)
        });
        self
    }

    pub fn gbuffer(mut self) -> Self {
        self.graph.resources.push(RenderGraphResourceDesc::transient_texture(
            RG_GBUFFER_ALBEDO,
            "gbuffer_albedo",
            RenderGraphResourceUsage::ColorAttachment,
            self.target.viewport_extent,
            self.target.color_format,
        )
        .with_semantic(RenderGraphResourceSemantic::GBufferAlbedo));
        self.graph.resources.push(RenderGraphResourceDesc::transient_texture(
            RG_GBUFFER_NORMAL,
            "gbuffer_normal",
            RenderGraphResourceUsage::ColorAttachment,
            self.target.viewport_extent,
            self.target.color_format,
        )
        .with_semantic(RenderGraphResourceSemantic::GBufferNormal));
        self.graph.resources.push(RenderGraphResourceDesc::transient_texture(
            RG_GBUFFER_MATERIAL,
            "gbuffer_material",
            RenderGraphResourceUsage::ColorAttachment,
            self.target.viewport_extent,
            TextureFormat::Rgba8Unorm,
        )
        .with_semantic(RenderGraphResourceSemantic::GBufferMaterial));
        self.add_phase_pass(StandardRenderPhase::ViewportGBuffer, |pass| {
            pass.with_domain(RenderGraphPassDomain::Render3d).reads(RG_GBUFFER_DEPTH, RenderGraphResourceUsage::DepthAttachment)
                .writes(RG_GBUFFER_ALBEDO, RenderGraphResourceUsage::ColorAttachment)
                .writes(RG_GBUFFER_NORMAL, RenderGraphResourceUsage::ColorAttachment)
                .writes(RG_GBUFFER_MATERIAL, RenderGraphResourceUsage::ColorAttachment)
        });
        self
    }

    pub fn forward_opaque(mut self) -> Self {
        let has_shadow = self.has_resource(RG_SHADOW_MAP);
        let viewport_color = self.viewport_color_resource();
        self.add_phase_pass(StandardRenderPhase::ViewportForward, |pass| {
            let pass = pass.with_domain(RenderGraphPassDomain::Render3d)
                .writes(viewport_color, RenderGraphResourceUsage::ColorAttachment)
                .writes(RG_VIEWPORT_DEPTH, RenderGraphResourceUsage::DepthAttachment)
                .draw_list(DrawListKind::OpaqueForward);
            if has_shadow {
                pass.reads(RG_SHADOW_MAP, RenderGraphResourceUsage::SampledTexture)
            } else {
                pass
            }
        });
        self
    }

    #[inline]
    fn viewport_color_resource(&mut self) -> RenderGraphResourceId {
        if self.target.hdr_scene_enabled {
            return RG_SCENE_HDR_COLOR;
        }
        if self.target.viewport_is_surface {
            RG_SURFACE_COLOR
        } else {
            RG_VIEWPORT_COLOR
        }
    }

    fn add_phase_pass(
        &mut self,
        phase: StandardRenderPhase,
        build: impl FnOnce(RenderGraphPassDesc) -> RenderGraphPassDesc,
    ) {
        let Some(kind) = phase.pass_kind() else {
            return;
        };
        let id = phase.stable_pass_id().unwrap_or_else(|| {
            let id = self.next_custom_pass;
            self.next_custom_pass = self.next_custom_pass.saturating_add(1);
            newengine_render_api::RenderGraphPassId(id)
        });
        let pass = build(RenderGraphPassDesc::new(id, phase.label(), kind));
        self.graph.passes.push(pass);
        self.phases.push(RenderPhaseDesc::standard(phase));
    }

    #[inline]
    fn has_resource(&self, id: RenderGraphResourceId) -> bool {
        self.graph.resources.iter().any(|resource| resource.id == id)
    }
}
