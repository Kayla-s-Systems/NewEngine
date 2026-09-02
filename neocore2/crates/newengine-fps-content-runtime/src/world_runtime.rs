#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;

use newengine_core::ThreadPoolHandle;
use newengine_ecs::World;
use newengine_materials::MaterialRegistry;
use newengine_primitives::PrimitiveRegistry;
use newengine_world_runtime_api::{WorldRuntimeFrame, WorldRuntimeProvider};

pub trait FpsContentWorldRuntimeAdapter: Send + Sync {
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
pub struct FpsContentWorldRuntimeBinding(pub Arc<dyn FpsContentWorldRuntimeAdapter>);

pub fn install_fps_content_world_runtime_adapter(
    world: &mut World,
    adapter: Arc<dyn FpsContentWorldRuntimeAdapter>,
) {
    world.insert_resource(FpsContentWorldRuntimeBinding(adapter));
}

#[inline]
fn should_emit_content_profile(frame_index: u64, total_ms: f32) -> bool {
    total_ms >= 4.0 || frame_index.is_multiple_of(120)
}

fn emit_content_profile(frame_index: u64, total_ms: f32) {
    if !should_emit_content_profile(frame_index, total_ms) {
        return;
    }
    let payload = serde_json::json!({
        "schema": "newengine.diagnostics.profiler.sample.v1",
        "category": "world.runtime.contribution",
        "source": "newengine-fps-content-runtime",
        "name": "engine.fps.content-runtime",
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

struct FpsContentRuntime;

impl FpsContentWorldRuntimeAdapter for FpsContentRuntime {
    fn tick_prelaunch(
        &self,
        world: &mut World,
        primitives: &mut PrimitiveRegistry,
        materials: &MaterialRegistry,
        _thread_pool: Option<&ThreadPoolHandle>,
        _frame_index: u64,
    ) {
        crate::mission::tick_deferred_item_pickups(world, primitives, materials);
        crate::mission::tick_runtime_world_item_visuals(world, primitives, materials);
    }

    fn tick_frame(
        &self,
        world: &mut World,
        primitives: &mut PrimitiveRegistry,
        materials: &MaterialRegistry,
        _thread_pool: Option<&ThreadPoolHandle>,
        frame: WorldRuntimeFrame,
    ) {
        let started = std::time::Instant::now();
        crate::mission::tick_deferred_item_pickups(world, primitives, materials);
        crate::mission::tick_runtime_world_item_visuals(world, primitives, materials);
        emit_content_profile(frame.frame_index, started.elapsed().as_secs_f32() * 1000.0);
    }
}

/// Install the domain-owned FPS content runtime implementation.
pub fn install_fps_content_world_runtime(world: &mut World) {
    install_fps_content_world_runtime_adapter(world, Arc::new(FpsContentRuntime));
}

pub struct FpsContentWorldRuntimeProvider;

impl FpsContentWorldRuntimeProvider {
    #[inline]
    pub fn shared() -> Arc<dyn WorldRuntimeProvider> {
        Arc::new(Self)
    }
}

impl WorldRuntimeProvider for FpsContentWorldRuntimeProvider {
    fn id(&self) -> &'static str {
        "engine.fps.content-runtime"
    }

    fn tick_prelaunch(
        &self,
        world: &mut World,
        primitives: &mut PrimitiveRegistry,
        materials: &MaterialRegistry,
        thread_pool: Option<&ThreadPoolHandle>,
        frame_index: u64,
    ) {
        let binding = world.resource::<FpsContentWorldRuntimeBinding>().cloned();
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
        let binding = world.resource::<FpsContentWorldRuntimeBinding>().cloned();
        if let Some(binding) = binding {
            binding
                .0
                .tick_frame(world, primitives, materials, thread_pool, frame);
        }
    }
}
