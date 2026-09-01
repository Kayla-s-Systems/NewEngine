#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use newengine_ecs::{EntityId, World};
use newengine_physics_api::{PhysicsQueryDto, PhysicsQueryHitDto};

use newengine_runtime_provider_api::{
    validate_provider_contract, RuntimeProviderDescriptor, I_GAMEPLAY_PHYSICS_QUERY_PROVIDER_V1,
    PROVIDER_CONTRACT_V1,
};

// Compatibility re-export; the contract is owned outside the composition root.
pub use newengine_physics_world_api::GameplayPhysicsQueryProvider;

#[derive(Default)]
pub struct GameplayPhysicsQueryProviderRegistry {
    providers: Vec<Arc<dyn GameplayPhysicsQueryProvider>>,
}

impl GameplayPhysicsQueryProviderRegistry {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_provider(&mut self, provider: Arc<dyn GameplayPhysicsQueryProvider>) {
        let descriptor = RuntimeProviderDescriptor::gameplay_physics_queries(provider.id());
        if let Err(error) = validate_provider_contract(
            descriptor,
            I_GAMEPLAY_PHYSICS_QUERY_PROVIDER_V1,
            PROVIDER_CONTRACT_V1,
        ) {
            newengine_ulog_api::ulog::warn!("gameplay physics-query provider rejected: {}", error);
            return;
        }
        if let Some(existing) = self
            .providers
            .iter_mut()
            .find(|existing| existing.id() == provider.id())
        {
            *existing = provider;
            return;
        }
        self.providers.push(provider);
    }

    pub fn descriptors(&self) -> Vec<RuntimeProviderDescriptor> {
        self.providers
            .iter()
            .map(|provider| RuntimeProviderDescriptor::gameplay_physics_queries(provider.id()))
            .collect()
    }

    pub fn collect_queries(&self, world: &World) -> Vec<PhysicsQueryDto> {
        let mut queries = Vec::new();
        for provider in &self.providers {
            queries.extend(provider.collect_queries(world));
        }
        queries
    }

    pub fn resolve_query_hits(
        &self,
        world: &mut World,
        fixed_tick: u64,
        hits: &[PhysicsQueryHitDto],
        key_to_entity: &BTreeMap<u64, EntityId>,
    ) -> BTreeSet<u64> {
        let mut consumed = BTreeSet::new();
        for provider in &self.providers {
            consumed.extend(provider.resolve_query_hits(world, fixed_tick, hits, key_to_entity));
        }
        consumed
    }
}
