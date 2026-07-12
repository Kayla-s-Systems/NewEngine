use std::sync::Arc;

use crate::bookkeeping::WorldRuntimeBookkeeping;

pub const WORLD_GATEWAY_OWNER: &str = "newengine-world-runtime.world-gateway";
pub const WORLD_FOUNDATION_PROVIDER_ROUTE: &str = "engine.world.foundation";
pub(crate) const WORLD_SNAPSHOT_SCHEMA_V1: &str = "newengine.world.snapshot.v1";

#[derive(Clone)]
pub struct EngineWorldGatewayService {
    pub(crate) scene: Arc<newengine_scene_runtime::SceneBridge>,
    pub(crate) state: Arc<parking_lot::Mutex<WorldRuntimeBookkeeping>>,
}

impl EngineWorldGatewayService {
    #[inline]
    pub fn new(scene: Arc<newengine_scene_runtime::SceneBridge>) -> Self {
        Self {
            scene,
            state: Arc::new(parking_lot::Mutex::new(WorldRuntimeBookkeeping::default())),
        }
    }
}
