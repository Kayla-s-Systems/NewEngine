#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;

use newengine_core::ThreadPoolHandle;
use newengine_ecs::World;
use newengine_materials::MaterialRegistry;
use newengine_primitives::PrimitiveRegistry;

use newengine_runtime_provider_api::{
    validate_provider_contract, RuntimeProviderDescriptor, I_WORLD_RUNTIME_PROVIDER_V1,
    PROVIDER_CONTRACT_V1,
};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WorldRuntimeFrame {
    pub frame_index: u64,
    pub dt: f32,
    pub runtime_active: bool,
    pub streaming_enabled: bool,
    pub environment_cycle_enabled: bool,
}

pub trait WorldRuntimeProvider: Send + Sync {
    fn id(&self) -> &'static str;

    #[inline]
    fn descriptor(&self) -> RuntimeProviderDescriptor {
        RuntimeProviderDescriptor::world_runtime(self.id())
    }

    /// Progress launch-blocking authored-world assembly while the loading gate is active.
    fn tick_prelaunch(
        &self,
        _world: &mut World,
        _primitives: &mut PrimitiveRegistry,
        _materials: &MaterialRegistry,
        _thread_pool: Option<&ThreadPoolHandle>,
        _frame_index: u64,
    ) {
    }

    /// Progress normal world-owned streaming/environment work for one engine frame.
    fn tick_frame(
        &self,
        _world: &mut World,
        _primitives: &mut PrimitiveRegistry,
        _materials: &MaterialRegistry,
        _thread_pool: Option<&ThreadPoolHandle>,
        _frame: WorldRuntimeFrame,
    ) {
    }
}

#[derive(Default)]
pub struct WorldRuntimeProviderRegistry {
    providers: Vec<Arc<dyn WorldRuntimeProvider>>,
}

impl WorldRuntimeProviderRegistry {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_provider(&mut self, provider: Arc<dyn WorldRuntimeProvider>) {
        let descriptor = provider.descriptor();
        if let Err(error) = validate_provider_contract(
            descriptor,
            I_WORLD_RUNTIME_PROVIDER_V1,
            PROVIDER_CONTRACT_V1,
        ) {
            newengine_ulog_api::ulog::warn!("world runtime provider rejected: {}", error);
            return;
        }
        if let Some(existing) = self
            .providers
            .iter_mut()
            .find(|existing| existing.id() == descriptor.id)
        {
            *existing = provider;
            return;
        }
        self.providers.push(provider);
    }

    pub fn tick_prelaunch(
        &self,
        world: &mut World,
        primitives: &mut PrimitiveRegistry,
        materials: &MaterialRegistry,
        thread_pool: Option<&ThreadPoolHandle>,
        frame_index: u64,
    ) {
        for provider in &self.providers {
            provider.tick_prelaunch(world, primitives, materials, thread_pool, frame_index);
        }
    }

    pub fn tick_frame(
        &self,
        world: &mut World,
        primitives: &mut PrimitiveRegistry,
        materials: &MaterialRegistry,
        thread_pool: Option<&ThreadPoolHandle>,
        frame: WorldRuntimeFrame,
    ) {
        for provider in &self.providers {
            provider.tick_frame(world, primitives, materials, thread_pool, frame);
        }
    }

    pub fn descriptors(&self) -> Vec<RuntimeProviderDescriptor> {
        self.providers
            .iter()
            .map(|provider| provider.descriptor())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestProvider;
    impl WorldRuntimeProvider for TestProvider {
        fn id(&self) -> &'static str {
            "test.world"
        }
    }

    #[test]
    fn registry_accepts_versioned_world_provider() {
        let mut registry = WorldRuntimeProviderRegistry::new();
        registry.register_provider(Arc::new(TestProvider));
        assert_eq!(registry.descriptors().len(), 1);
        assert_eq!(
            registry.descriptors()[0].interface_id,
            I_WORLD_RUNTIME_PROVIDER_V1
        );
    }
}
