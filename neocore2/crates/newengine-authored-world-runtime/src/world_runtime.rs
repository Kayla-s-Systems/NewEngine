#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;

use newengine_core::ThreadPoolHandle;
use newengine_ecs::World;
use newengine_materials::MaterialRegistry;
use newengine_primitives::PrimitiveRegistry;
use newengine_world_runtime_api::{WorldRuntimeFrame, WorldRuntimeProvider};

pub trait AuthoredWorldStreamingRuntimeAdapter: Send + Sync {
    fn tick_prelaunch(
        &self,
        world: &mut World,
        primitives: &mut PrimitiveRegistry,
        materials: &MaterialRegistry,
        thread_pool: Option<&ThreadPoolHandle>,
        frame_index: u64,
    );

    fn tick_frame(
        &self,
        world: &mut World,
        primitives: &mut PrimitiveRegistry,
        materials: &MaterialRegistry,
        thread_pool: Option<&ThreadPoolHandle>,
        frame: WorldRuntimeFrame,
    );
}

#[derive(Clone)]
pub struct AuthoredWorldStreamingRuntimeBinding(pub Arc<dyn AuthoredWorldStreamingRuntimeAdapter>);

pub fn install_authored_world_streaming_runtime_adapter(
    world: &mut World,
    adapter: Arc<dyn AuthoredWorldStreamingRuntimeAdapter>,
) {
    world.insert_resource(AuthoredWorldStreamingRuntimeBinding(adapter));
}

struct AuthoredSceneStreamingRuntimeAdapter;

#[inline]
fn emit_authored_streaming_profile(frame_index: u64, total_ms: f32) {
    if total_ms < 4.0 && !frame_index.is_multiple_of(120) {
        return;
    }
    let payload = serde_json::json!({
        "schema": "newengine.diagnostics.profiler.sample.v1",
        "category": "world.runtime.contribution",
        "source": "newengine-authored-world-runtime",
        "name": "engine.authored-world.streaming-runtime",
        "lane": "world-runtime",
        "priority": "interactive",
        "dependency_group": format!("world.runtime.frame.{frame_index}"),
        "frame_index": frame_index,
        "elapsed_ms": total_ms,
        "budget_ms": 4.0,
        "slow": total_ms >= 4.0,
    });
    if let Ok(bytes) = serde_json::to_vec(&payload) {
        let _ = newengine_plugin_host::emit_plugin_event(
            "newengine.diagnostics.profiler.sample.v1",
            &bytes,
        );
    }
}

impl AuthoredWorldStreamingRuntimeAdapter for AuthoredSceneStreamingRuntimeAdapter {
    fn tick_prelaunch(
        &self,
        world: &mut World,
        primitives: &mut PrimitiveRegistry,
        materials: &MaterialRegistry,
        thread_pool: Option<&ThreadPoolHandle>,
        _frame_index: u64,
    ) {
        crate::tick_authored_map_streaming(world, primitives, materials, thread_pool);
        crate::tick_authored_static_world_prefabs(world, primitives, materials, thread_pool);
    }

    fn tick_frame(
        &self,
        world: &mut World,
        primitives: &mut PrimitiveRegistry,
        materials: &MaterialRegistry,
        thread_pool: Option<&ThreadPoolHandle>,
        frame: WorldRuntimeFrame,
    ) {
        let started = std::time::Instant::now();
        if frame.runtime_active && frame.streaming_enabled {
            crate::tick_authored_map_streaming(world, primitives, materials, thread_pool);
        }
        crate::tick_authored_static_world_prefabs(world, primitives, materials, thread_pool);
        emit_authored_streaming_profile(
            frame.frame_index,
            started.elapsed().as_secs_f32() * 1000.0,
        );
    }
}

pub fn install_default_authored_world_streaming_runtime_adapter(world: &mut World) {
    install_authored_world_streaming_runtime_adapter(
        world,
        Arc::new(AuthoredSceneStreamingRuntimeAdapter),
    );
}

pub struct AuthoredWorldStreamingWorldRuntimeProvider;

impl AuthoredWorldStreamingWorldRuntimeProvider {
    #[inline]
    pub fn shared() -> Arc<dyn WorldRuntimeProvider> {
        Arc::new(Self)
    }
}

impl WorldRuntimeProvider for AuthoredWorldStreamingWorldRuntimeProvider {
    fn id(&self) -> &'static str {
        "engine.authored-world.streaming-runtime"
    }

    fn tick_prelaunch(
        &self,
        world: &mut World,
        primitives: &mut PrimitiveRegistry,
        materials: &MaterialRegistry,
        thread_pool: Option<&ThreadPoolHandle>,
        frame_index: u64,
    ) {
        let binding = world
            .resource::<AuthoredWorldStreamingRuntimeBinding>()
            .cloned();
        if let Some(binding) = binding {
            binding
                .0
                .tick_prelaunch(world, primitives, materials, thread_pool, frame_index);
        }
    }

    fn tick_frame(
        &self,
        world: &mut World,
        primitives: &mut PrimitiveRegistry,
        materials: &MaterialRegistry,
        thread_pool: Option<&ThreadPoolHandle>,
        frame: WorldRuntimeFrame,
    ) {
        let binding = world
            .resource::<AuthoredWorldStreamingRuntimeBinding>()
            .cloned();
        if let Some(binding) = binding {
            binding
                .0
                .tick_frame(world, primitives, materials, thread_pool, frame);
        }
    }
}
