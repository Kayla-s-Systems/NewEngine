use std::sync::Arc;

use newengine_entity_api::{EntityHandle, EntityRecord};

pub const ENTITY_GATEWAY_OWNER: &str = "newengine-entity-runtime.entity-gateway";
pub(crate) const MAX_ENTITY_SPAWN_PER_CALL: usize = 4096;

#[derive(Clone)]
pub struct EngineEntityGatewayService {
    pub(crate) scene: Arc<newengine_scene_runtime::SceneBridge>,
}

impl EngineEntityGatewayService {
    #[inline]
    pub fn new(scene: Arc<newengine_scene_runtime::SceneBridge>) -> Self {
        Self { scene }
    }

    #[inline]
    pub(crate) fn handle(id: newengine_ecs::EntityId) -> EntityHandle {
        EntityHandle::new(id.stable_u64())
    }

    pub(crate) fn live_record(handle: EntityHandle) -> EntityRecord {
        EntityRecord {
            handle,
            lifecycle: String::new(),
            tags: Vec::new(),
            owner: None,
            debug_identity: String::new(),
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
