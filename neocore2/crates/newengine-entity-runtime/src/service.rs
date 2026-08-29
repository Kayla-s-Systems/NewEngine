use std::sync::Arc;

use newengine_entity_api::{EntityHandle, EntityRecord};

use crate::archetype::{
    default_entity_archetype_registry, EntityArchetypeRegistry, EntityRuntimeMetadata,
};

pub const ENTITY_GATEWAY_OWNER: &str = "newengine-entity-runtime.entity-gateway";
pub(crate) const MAX_ENTITY_SPAWN_PER_CALL: usize = 4096;

#[derive(Clone)]
pub struct EngineEntityGatewayService {
    pub(crate) scene: Arc<newengine_scene_runtime::SceneBridge>,
    pub(crate) archetypes: Arc<EntityArchetypeRegistry>,
}

impl EngineEntityGatewayService {
    #[inline]
    pub fn new(scene: Arc<newengine_scene_runtime::SceneBridge>) -> Self {
        Self {
            scene,
            archetypes: default_entity_archetype_registry(),
        }
    }

    #[inline]
    pub fn with_archetypes(
        scene: Arc<newengine_scene_runtime::SceneBridge>,
        archetypes: Arc<EntityArchetypeRegistry>,
    ) -> Self {
        Self { scene, archetypes }
    }

    #[inline]
    pub(crate) fn handle(id: newengine_ecs::EntityId) -> EntityHandle {
        EntityHandle::new(id.stable_u64())
    }

    pub(crate) fn live_record(
        world: &newengine_ecs::World,
        id: newengine_ecs::EntityId,
    ) -> EntityRecord {
        let handle = Self::handle(id);
        let metadata = world.get::<EntityRuntimeMetadata>(id);
        EntityRecord {
            handle,
            lifecycle: "alive".to_owned(),
            tags: metadata.map(|meta| meta.tags.clone()).unwrap_or_default(),
            owner: metadata.and_then(|meta| meta.owner.clone()),
            archetype: metadata.map(|meta| meta.archetype.clone()),
            debug_identity: metadata
                .map(|meta| format!("{}:{}", meta.archetype, handle.stable_id))
                .unwrap_or_else(|| format!("entity:{}", handle.stable_id)),
        }
    }

    pub(crate) fn find_entity_by_handle(
        world: &newengine_ecs::World,
        handle: EntityHandle,
    ) -> Option<newengine_ecs::EntityId> {
        world
            .iter_entities()
            .find(|id| id.stable_u64() == handle.stable_id)
    }
}
