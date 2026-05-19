#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::HashMap;

use newengine_core::render::RenderApi;
use newengine_core::{EngineError, EngineResult as CoreResult};
use newengine_material_domain_api::{
    MaterialDomainError, MaterialGpuPipeline, MaterialGpuPipelineKey, MaterialGpuPipelineProvider,
    MaterialPipelineBuildProfile, MaterialRenderDevice,
};

struct CoreRenderMaterialDevice<'a> {
    inner: &'a mut dyn RenderApi,
}

impl<'a> CoreRenderMaterialDevice<'a> {
    #[inline]
    fn new(inner: &'a mut dyn RenderApi) -> Self {
        Self { inner }
    }

    #[inline]
    fn map_err(e: newengine_core::EngineError) -> MaterialDomainError {
        MaterialDomainError::other(e.to_string())
    }
}

impl MaterialRenderDevice for CoreRenderMaterialDevice<'_> {
    fn create_bind_group_layout(
        &mut self,
        desc: newengine_core::render::BindGroupLayoutDesc,
    ) -> Result<newengine_core::render::BindGroupLayoutId, MaterialDomainError> {
        self.inner.create_bind_group_layout(desc).map_err(Self::map_err)
    }

    fn create_texture(
        &mut self,
        desc: newengine_core::render::TextureDesc,
    ) -> Result<newengine_core::render::TextureId, MaterialDomainError> {
        self.inner.create_texture(desc).map_err(Self::map_err)
    }

    fn create_sampler(
        &mut self,
        desc: newengine_core::render::SamplerDesc,
    ) -> Result<newengine_core::render::SamplerId, MaterialDomainError> {
        self.inner.create_sampler(desc).map_err(Self::map_err)
    }

    fn create_shader(
        &mut self,
        desc: newengine_core::render::ShaderDesc,
    ) -> Result<newengine_core::render::ShaderId, MaterialDomainError> {
        self.inner.create_shader(desc).map_err(Self::map_err)
    }

    fn create_pipeline(
        &mut self,
        desc: newengine_core::render::PipelineDesc,
    ) -> Result<newengine_core::render::PipelineId, MaterialDomainError> {
        self.inner.create_pipeline(desc).map_err(Self::map_err)
    }
}

/// GPU material registry owned by the engine runtime side of the renderer.
///
/// Reusable runtime orchestration no longer owns GameReady/FPS shader paths or
/// material presets. It only stores host-side material-domain providers that are
/// registered by the game/profile layer, then asks the selected provider to build
/// a backend-neutral pipeline bundle through `RenderApi`.
#[derive(Default)]
pub struct MaterialGpuRegistry {
    providers: HashMap<&'static str, Box<dyn MaterialGpuPipelineProvider>>,
}

impl MaterialGpuRegistry {
    pub fn register_provider(&mut self, provider: Box<dyn MaterialGpuPipelineProvider>) {
        let key = provider.key();
        let replaced = self.providers.insert(key.as_str(), provider).is_some();
        if replaced {
            log::warn!(
                "render material registry: replaced material-domain provider key='{}'",
                key.as_str()
            );
        }
    }

    pub(crate) fn require_pipeline(
        &mut self,
        key: MaterialGpuPipelineKey,
        profile: MaterialPipelineBuildProfile,
        r: &mut dyn RenderApi,
    ) -> CoreResult<MaterialGpuPipeline> {
        let Some(provider) = self.providers.get_mut(key.as_str()) else {
            return Err(EngineError::other(format!(
                "render material registry: no material-domain provider registered key='{}'",
                key.as_str()
            )));
        };

        let mut device = CoreRenderMaterialDevice::new(r);
        provider
            .require_pipeline(profile, &mut device)
            .map_err(|e| EngineError::other(format!("render material registry: {}", e)))
    }
}
