#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::HashMap;

use newengine_core::render::RenderApi;
use newengine_core::{EngineError, EngineResult as CoreResult};
use newengine_material_domain_api::{
    MaterialGpuPipeline, MaterialGpuPipelineKey, MaterialGpuPipelineProvider,
    MaterialPipelineBuildProfile,
};

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

        provider.require_pipeline(profile, r)
    }
}
