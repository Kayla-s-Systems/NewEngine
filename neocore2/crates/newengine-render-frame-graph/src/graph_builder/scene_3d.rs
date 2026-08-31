use newengine_render_api::{
    RenderGraphPassDomain, RenderGraphResourceDesc, RenderGraphResourceId,
    RenderGraphResourceLifetime, RenderGraphResourceSemantic, RenderGraphResourceUsage,
    TextureFormat,
};

use crate::{DrawListKind, StandardRenderPhase};

use super::{
    FrameGraphBuilder, RG_GBUFFER_ALBEDO, RG_GBUFFER_DEPTH, RG_GBUFFER_MATERIAL, RG_GBUFFER_NORMAL,
    RG_SHADOW_MAP, RG_VIEWPORT_DEPTH,
};

const RG_VFX_PARTICLE_STATE: RenderGraphResourceId = RenderGraphResourceId(17_500);
const VFX_PARTICLE_STATE_BYTES: u64 = 262_144 * 96;
pub(super) const RG_HAIR_STRAND_STATE: RenderGraphResourceId = RenderGraphResourceId(17_600);
const HAIR_STRAND_STATE_BYTES: u64 = 40_632_320;

impl FrameGraphBuilder {
    #[inline]
    pub fn viewport_gbuffer_or_forward(mut self, deferred: bool) -> Self {
        if deferred {
            self = self.gbuffer();
        } else {
            self = self.forward_opaque();
        }
        self
    }

    #[inline]
    pub fn tessellation_prepare(mut self) -> Self {
        self.add_phase_pass(StandardRenderPhase::TessellationPrepare, |pass| pass);
        self
    }

    #[inline]
    pub fn particle_simulation(mut self) -> Self {
        if !self.has_resource(RG_VFX_PARTICLE_STATE) {
            self.graph.resources.push(RenderGraphResourceDesc {
                id: RG_VFX_PARTICLE_STATE,
                label: Some("vfx_particle_state".to_owned()),
                semantic: RenderGraphResourceSemantic::Custom,
                usage: RenderGraphResourceUsage::StorageBuffer,
                lifetime: RenderGraphResourceLifetime::Persistent,
                extent: None,
                format: None,
                sample_count: 1,
                byte_size: Some(VFX_PARTICLE_STATE_BYTES),
                external: None,
            });
        }
        self.add_phase_pass(StandardRenderPhase::ParticleSimulation, |pass| {
            let mut pass = pass
                .with_domain(RenderGraphPassDomain::Render3d)
                .with_culling(false)
                .writes(
                    RG_VFX_PARTICLE_STATE,
                    RenderGraphResourceUsage::StorageBuffer,
                );
            pass.queue = newengine_render_api::RenderGraphQueueKind::Compute;
            pass
        });
        self
    }

    #[inline]
    pub fn hair_simulation(mut self) -> Self {
        if !self.has_resource(RG_HAIR_STRAND_STATE) {
            self.graph.resources.push(RenderGraphResourceDesc {
                id: RG_HAIR_STRAND_STATE,
                label: Some("hair_strand_state".to_owned()),
                semantic: RenderGraphResourceSemantic::Custom,
                usage: RenderGraphResourceUsage::StorageBuffer,
                lifetime: RenderGraphResourceLifetime::Persistent,
                extent: None,
                format: None,
                sample_count: 1,
                byte_size: Some(HAIR_STRAND_STATE_BYTES),
                external: None,
            });
        }
        self.add_phase_pass(StandardRenderPhase::HairSimulation, |pass| {
            let mut pass = pass
                .with_domain(RenderGraphPassDomain::Render3d)
                .with_culling(false)
                .writes(
                    RG_HAIR_STRAND_STATE,
                    RenderGraphResourceUsage::StorageBuffer,
                );
            pass.queue = newengine_render_api::RenderGraphQueueKind::Compute;
            pass
        });
        self
    }

    #[inline]
    pub fn transparent(mut self) -> Self {
        let viewport_color = self.viewport_color_resource();
        let scene_depth = self.scene_depth_resource();
        let has_hair = self.has_resource(RG_HAIR_STRAND_STATE);
        let has_shadow = self.has_resource(RG_SHADOW_MAP);
        self.add_phase_pass(StandardRenderPhase::Transparent, |pass| {
            let pass = pass.with_domain(RenderGraphPassDomain::Render3d).reads(
                RG_VFX_PARTICLE_STATE,
                RenderGraphResourceUsage::StorageBuffer,
            );
            let pass = if let Some(scene_depth) = scene_depth {
                pass.reads(scene_depth, RenderGraphResourceUsage::DepthAttachment)
            } else {
                pass
            };
            let pass = pass
                .writes(viewport_color, RenderGraphResourceUsage::ColorAttachment)
                .draw_list(DrawListKind::Transparent);
            let pass = if has_hair {
                pass.reads(
                    RG_HAIR_STRAND_STATE,
                    RenderGraphResourceUsage::StorageBuffer,
                )
            } else {
                pass
            };
            if has_shadow {
                pass.reads(RG_SHADOW_MAP, RenderGraphResourceUsage::SampledTexture)
            } else {
                pass
            }
        });
        self
    }

    #[inline]
    pub fn water(mut self) -> Self {
        let viewport_color = self.viewport_color_resource();
        let scene_depth = self.scene_depth_resource();
        self.add_phase_pass(StandardRenderPhase::Water, |pass| {
            let pass = pass.with_domain(RenderGraphPassDomain::Render3d);
            let pass = if let Some(scene_depth) = scene_depth {
                pass.reads(scene_depth, RenderGraphResourceUsage::DepthAttachment)
            } else {
                pass
            };
            pass.writes(viewport_color, RenderGraphResourceUsage::ColorAttachment)
        });
        self
    }

    pub fn depth_prepass(mut self) -> Self {
        self.add_phase_pass(StandardRenderPhase::DepthPrepass, |pass| {
            pass.with_domain(RenderGraphPassDomain::Render3d)
                .writes(RG_VIEWPORT_DEPTH, RenderGraphResourceUsage::DepthAttachment)
        });
        self
    }

    pub fn gbuffer(mut self) -> Self {
        self.graph.resources.push(
            RenderGraphResourceDesc::transient_texture(
                RG_GBUFFER_DEPTH,
                "gbuffer_depth",
                RenderGraphResourceUsage::DepthAttachment,
                self.target.viewport_extent,
                self.target.depth_format,
            )
            .with_semantic(RenderGraphResourceSemantic::GBufferDepth),
        );
        self.graph.resources.push(
            RenderGraphResourceDesc::transient_texture(
                RG_GBUFFER_ALBEDO,
                "gbuffer_albedo",
                RenderGraphResourceUsage::ColorAttachment,
                self.target.viewport_extent,
                TextureFormat::Rgba8Unorm,
            )
            .with_semantic(RenderGraphResourceSemantic::GBufferAlbedo),
        );
        self.graph.resources.push(
            RenderGraphResourceDesc::transient_texture(
                RG_GBUFFER_NORMAL,
                "gbuffer_normal",
                RenderGraphResourceUsage::ColorAttachment,
                self.target.viewport_extent,
                TextureFormat::Rgba16Float,
            )
            .with_semantic(RenderGraphResourceSemantic::GBufferNormal),
        );
        self.graph.resources.push(
            RenderGraphResourceDesc::transient_texture(
                RG_GBUFFER_MATERIAL,
                "gbuffer_material",
                RenderGraphResourceUsage::ColorAttachment,
                self.target.viewport_extent,
                TextureFormat::Rgba8Unorm,
            )
            .with_semantic(RenderGraphResourceSemantic::GBufferMaterial),
        );
        self.add_phase_pass(StandardRenderPhase::ViewportGBuffer, |pass| {
            pass.with_domain(RenderGraphPassDomain::Render3d)
                .writes(RG_GBUFFER_DEPTH, RenderGraphResourceUsage::DepthAttachment)
                .writes(RG_GBUFFER_ALBEDO, RenderGraphResourceUsage::ColorAttachment)
                .writes(RG_GBUFFER_NORMAL, RenderGraphResourceUsage::ColorAttachment)
                .writes(
                    RG_GBUFFER_MATERIAL,
                    RenderGraphResourceUsage::ColorAttachment,
                )
                .draw_list(DrawListKind::OpaqueForward)
        });
        self
    }

    pub fn forward_opaque(mut self) -> Self {
        let has_shadow = self.has_resource(RG_SHADOW_MAP);
        let viewport_color = self.viewport_color_resource();
        let deferred_depth = self
            .has_resource(RG_GBUFFER_DEPTH)
            .then_some(RG_GBUFFER_DEPTH);
        self.add_phase_pass(StandardRenderPhase::ViewportForward, |pass| {
            let pass = pass
                .with_domain(RenderGraphPassDomain::Render3d)
                .writes(viewport_color, RenderGraphResourceUsage::ColorAttachment)
                .draw_list(DrawListKind::OpaqueForward);
            let pass = if let Some(depth) = deferred_depth {
                // Deferred forward-only roles are an overlay on the lighting resolve.
                // Borrow GBuffer depth; never manufacture a second scene-depth owner.
                pass.reads(depth, RenderGraphResourceUsage::DepthAttachment)
            } else {
                pass.writes(RG_VIEWPORT_DEPTH, RenderGraphResourceUsage::DepthAttachment)
            };
            if has_shadow {
                pass.reads(RG_SHADOW_MAP, RenderGraphResourceUsage::SampledTexture)
            } else {
                pass
            }
        });
        self
    }
}
