use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderBackendClass {
    Headless,
    Raster,
    RayTracing,
    Hybrid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderThreadingModel {
    SingleThreaded,
    RenderThread,
    MultiQueue,
}

/// Coarse hardware capability tier exposed by a render backend.
///
/// This is intentionally not a per-GPU model profile. Backends classify the
/// selected adapter by capabilities and broad family, then the host/profile
/// decides which optional render features are allowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderHardwareTier {
    Headless,
    LegacyGtx,
    Gtx,
    Rtx,
    Unknown,
}

impl Default for RenderHardwareTier {
    #[inline]
    fn default() -> Self {
        Self::Unknown
    }
}

impl RenderHardwareTier {
    #[inline]
    pub const fn profile_id(self) -> &'static str {
        match self {
            Self::Headless => "newengine.render.runtime.tier.headless",
            Self::LegacyGtx => "newengine.render.runtime.tier.legacy_gtx",
            Self::Gtx => "newengine.render.runtime.tier.gtx",
            Self::Rtx => "newengine.render.runtime.tier.rtx",
            Self::Unknown => "newengine.render.runtime.tier.auto",
        }
    }

    #[inline]
    pub const fn is_conservative(self) -> bool {
        matches!(self, Self::LegacyGtx | Self::Gtx)
    }
}

impl Default for RenderThreadingModel {
    #[inline]
    fn default() -> Self {
        Self::SingleThreaded
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RenderFeature {
    Swapchain,
    OffscreenTargets,
    DepthTargets,
    TextureSampling,
    StorageBuffers,
    StorageTextures,
    AsyncUploads,
    PersistentTransferUploads,
    FenceTrackedUploads,
    TimelineSemaphoreReady,
    PipelineCache,
    RuntimeShaderBake,
    ShaderDiskCache,
    RenderGraph,
    TransientResourceLifetime,
    DeferredGBuffer,
    DeferredLighting,
    TiledLighting,
    ClusteredLighting,
    Shadows,
    ShadowCasterCulling,
    ShadowAtlas,
    PcssShadows,
    CascadedShadowMaps,
    HdrSceneColor,
    Bloom,
    Fxaa,
    Msaa,
    Taa,
    Tessellation,
    EffectStack,
    PostEffects,
    PostFxExposureHistory,
    Ssao,
    AdaptiveDof,
    LensArtefacts,
    Mlaa,
    PostScan,
    OcclusionCulling,
    HiZOcclusion,
    PvsVisibility,
    ZoneCulling,
    Reflections,
    PlanarReflections,
    MirrorReflections,
    WaterReflections,
    WaterRendering,
    Vegetation,
    GrassRendering,
    TreeImposters,
    UiComposite,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderLimits {
    pub max_texture_dimension_2d: u32,
    pub max_color_attachments: u32,
    pub max_bind_groups: u32,
    pub max_sampled_textures_per_stage: u32,
    pub max_uniform_buffer_range: u64,
    pub max_storage_buffer_range: u64,
}

impl Default for RenderLimits {
    #[inline]
    fn default() -> Self {
        Self {
            max_texture_dimension_2d: 4096,
            max_color_attachments: 4,
            max_bind_groups: 4,
            max_sampled_textures_per_stage: 16,
            max_uniform_buffer_range: 64 * 1024,
            max_storage_buffer_range: 128 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderBackendCapabilities {
    pub backend_class: RenderBackendClass,
    pub threading: RenderThreadingModel,
    pub features: Vec<RenderFeature>,
    pub limits: RenderLimits,
    #[serde(default)]
    pub hardware_tier: RenderHardwareTier,
}

impl RenderBackendCapabilities {
    #[inline]
    pub fn raster_default() -> Self {
        Self {
            backend_class: RenderBackendClass::Raster,
            threading: RenderThreadingModel::SingleThreaded,
            features: vec![
                RenderFeature::Swapchain,
                RenderFeature::OffscreenTargets,
                RenderFeature::DepthTargets,
                RenderFeature::TextureSampling,
                RenderFeature::StorageBuffers,
                RenderFeature::AsyncUploads,
                RenderFeature::PersistentTransferUploads,
                RenderFeature::FenceTrackedUploads,
                RenderFeature::PipelineCache,
                RenderFeature::RuntimeShaderBake,
                RenderFeature::ShaderDiskCache,
                RenderFeature::RenderGraph,
                RenderFeature::TransientResourceLifetime,
                RenderFeature::DeferredGBuffer,
                RenderFeature::DeferredLighting,
                RenderFeature::TiledLighting,
                RenderFeature::ClusteredLighting,
                RenderFeature::Shadows,
                RenderFeature::ShadowCasterCulling,
                RenderFeature::ShadowAtlas,
                RenderFeature::PcssShadows,
                RenderFeature::CascadedShadowMaps,
                RenderFeature::HdrSceneColor,
                RenderFeature::Bloom,
                RenderFeature::Fxaa,
                RenderFeature::Taa,
                RenderFeature::Tessellation,
                RenderFeature::EffectStack,
                RenderFeature::PostEffects,
                RenderFeature::PostFxExposureHistory,
                RenderFeature::Ssao,
                RenderFeature::AdaptiveDof,
                RenderFeature::LensArtefacts,
                RenderFeature::Mlaa,
                RenderFeature::PostScan,
                RenderFeature::OcclusionCulling,
                RenderFeature::HiZOcclusion,
                RenderFeature::PvsVisibility,
                RenderFeature::ZoneCulling,
                RenderFeature::Reflections,
                RenderFeature::PlanarReflections,
                RenderFeature::MirrorReflections,
                RenderFeature::WaterReflections,
                RenderFeature::WaterRendering,
                RenderFeature::Vegetation,
                RenderFeature::GrassRendering,
                RenderFeature::TreeImposters,
                RenderFeature::UiComposite,
            ],
            limits: RenderLimits::default(),
            hardware_tier: RenderHardwareTier::Unknown,
        }
    }

    #[inline]
    pub fn headless_default() -> Self {
        Self {
            backend_class: RenderBackendClass::Headless,
            threading: RenderThreadingModel::SingleThreaded,
            features: Vec::new(),
            limits: RenderLimits::default(),
            hardware_tier: RenderHardwareTier::Headless,
        }
    }

    #[inline]
    pub fn supports(&self, feature: RenderFeature) -> bool {
        self.features.contains(&feature)
    }
}

impl Default for RenderBackendCapabilities {
    #[inline]
    fn default() -> Self {
        Self::raster_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raster_default_advertises_implemented_postfx_and_shadow_features() {
        let caps = RenderBackendCapabilities::raster_default();
        assert!(caps.supports(RenderFeature::HdrSceneColor));
        assert!(caps.supports(RenderFeature::Bloom));
        assert!(caps.supports(RenderFeature::Fxaa));
        assert!(caps.supports(RenderFeature::Taa));
        assert!(caps.supports(RenderFeature::Tessellation));
        assert!(caps.supports(RenderFeature::EffectStack));
        assert!(caps.supports(RenderFeature::DeferredGBuffer));
        assert!(caps.supports(RenderFeature::DeferredLighting));
        assert!(caps.supports(RenderFeature::TiledLighting));
        assert!(caps.supports(RenderFeature::ClusteredLighting));
        assert!(caps.supports(RenderFeature::PcssShadows));
        assert!(caps.supports(RenderFeature::ShadowCasterCulling));
        assert!(caps.supports(RenderFeature::Ssao));
        assert!(caps.supports(RenderFeature::AdaptiveDof));
        assert!(caps.supports(RenderFeature::LensArtefacts));
        assert!(caps.supports(RenderFeature::HiZOcclusion));
        assert!(caps.supports(RenderFeature::PvsVisibility));
        assert!(caps.supports(RenderFeature::WaterRendering));
        assert!(caps.supports(RenderFeature::TreeImposters));
        assert!(!caps.supports(RenderFeature::Msaa));
    }
}
