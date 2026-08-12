#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;

use newengine_core::ThreadPoolHandle;
use newengine_ecs::World;
use newengine_engine_runtime::{WorldRuntimeFrame, WorldRuntimeProvider};
use newengine_materials::MaterialRegistry;
use newengine_primitives::PrimitiveRegistry;

/// GameReady implementation of the generic world-runtime scheduling contract.
pub(crate) struct GameReadyWorldRuntimeProvider;

impl GameReadyWorldRuntimeProvider {
    #[inline]
    pub(crate) fn shared() -> Arc<Self> {
        Arc::new(Self)
    }
}

impl WorldRuntimeProvider for GameReadyWorldRuntimeProvider {
    fn id(&self) -> &'static str {
        "app.game-ready.world-runtime"
    }

    fn tick_prelaunch(
        &self,
        world: &mut World,
        primitives: &mut PrimitiveRegistry,
        materials: &MaterialRegistry,
        thread_pool: Option<&ThreadPoolHandle>,
        _frame_index: u64,
    ) {
        newengine_game_ready_world::tick_prelaunch(world, primitives, materials, thread_pool);
    }

    fn tick_frame(
        &self,
        world: &mut World,
        primitives: &mut PrimitiveRegistry,
        materials: &MaterialRegistry,
        thread_pool: Option<&ThreadPoolHandle>,
        frame: WorldRuntimeFrame,
    ) {
        newengine_game_ready_world::tick_frame(world, primitives, materials, thread_pool, frame);
    }
}
