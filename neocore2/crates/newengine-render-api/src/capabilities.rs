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
    Shadows,
    ShadowCasterCulling,
    ShadowAtlas,
    PcssShadows,
    CascadedShadowMaps,
    HdrSceneColor,
    Bloom,
    Fxaa,
    Msaa,
    PostEffects,
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
            max_color_attachments: 1,
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
                RenderFeature::Shadows,
                RenderFeature::ShadowCasterCulling,
                RenderFeature::ShadowAtlas,
                RenderFeature::PcssShadows,
                RenderFeature::CascadedShadowMaps,
                RenderFeature::HdrSceneColor,
                RenderFeature::Bloom,
                RenderFeature::Fxaa,
                RenderFeature::PostEffects,
                RenderFeature::UiComposite,
            ],
            limits: RenderLimits::default(),
        }
    }

    #[inline]
    pub fn headless_default() -> Self {
        Self {
            backend_class: RenderBackendClass::Headless,
            threading: RenderThreadingModel::SingleThreaded,
            features: Vec::new(),
            limits: RenderLimits::default(),
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
        assert!(caps.supports(RenderFeature::PcssShadows));
        assert!(caps.supports(RenderFeature::ShadowCasterCulling));
        assert!(!caps.supports(RenderFeature::Msaa));
    }
}
