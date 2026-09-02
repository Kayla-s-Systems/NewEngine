#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;

use newengine_core::ThreadPoolHandle;
use newengine_ecs::World;
use newengine_materials::MaterialRegistry;
use newengine_primitives::PrimitiveRegistry;
use newengine_world_runtime_api::{WorldRuntimeFrame, WorldRuntimeProvider};

pub trait WorldEnvironmentRuntimeAdapter: Send + Sync {
    fn tick_admission_prelaunch(
        &self,
        world: &mut World,
        primitives: &mut PrimitiveRegistry,
        materials: &MaterialRegistry,
        thread_pool: Option<&ThreadPoolHandle>,
        frame_index: u64,
    );

    fn tick_admission_frame(
        &self,
        world: &mut World,
        primitives: &mut PrimitiveRegistry,
        materials: &MaterialRegistry,
        thread_pool: Option<&ThreadPoolHandle>,
        frame: WorldRuntimeFrame,
    );

    fn tick_environment_frame(
        &self,
        world: &mut World,
        primitives: &mut PrimitiveRegistry,
        materials: &MaterialRegistry,
        thread_pool: Option<&ThreadPoolHandle>,
        frame: WorldRuntimeFrame,
    );
}

#[derive(Clone)]
pub struct WorldEnvironmentRuntimeBinding(pub Arc<dyn WorldEnvironmentRuntimeAdapter>);

pub fn install_world_environment_runtime_adapter(
    world: &mut World,
    adapter: Arc<dyn WorldEnvironmentRuntimeAdapter>,
) {
    world.insert_resource(WorldEnvironmentRuntimeBinding(adapter));
}

#[inline]
fn should_emit_environment_runtime_profile(frame_index: u64, total_ms: f32) -> bool {
    total_ms >= 4.0 || frame_index.is_multiple_of(120)
}

fn emit_environment_runtime_profile(provider_id: &'static str, frame_index: u64, total_ms: f32) {
    if !should_emit_environment_runtime_profile(frame_index, total_ms) {
        return;
    }
    let payload = serde_json::json!({
        "schema": "newengine.diagnostics.profiler.sample.v1",
        "category": "world.runtime.contribution",
        "source": "newengine-world-environment-runtime",
        "name": provider_id,
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

struct AuthoredEnvironmentRuntimeAdapter;

impl WorldEnvironmentRuntimeAdapter for AuthoredEnvironmentRuntimeAdapter {
    fn tick_admission_prelaunch(
        &self,
        world: &mut World,
        primitives: &mut PrimitiveRegistry,
        materials: &MaterialRegistry,
        _thread_pool: Option<&ThreadPoolHandle>,
        _frame_index: u64,
    ) {
        crate::authored_foliage::tick_deferred_foliage_prefabs(world, primitives, materials);
    }

    fn tick_admission_frame(
        &self,
        world: &mut World,
        primitives: &mut PrimitiveRegistry,
        materials: &MaterialRegistry,
        _thread_pool: Option<&ThreadPoolHandle>,
        frame: WorldRuntimeFrame,
    ) {
        let started = std::time::Instant::now();
        crate::authored_foliage::tick_deferred_foliage_prefabs(world, primitives, materials);
        emit_environment_runtime_profile(
            "engine.world.environment-admission",
            frame.frame_index,
            started.elapsed().as_secs_f32() * 1000.0,
        );
    }

    fn tick_environment_frame(
        &self,
        world: &mut World,
        _primitives: &mut PrimitiveRegistry,
        materials: &MaterialRegistry,
        thread_pool: Option<&ThreadPoolHandle>,
        frame: WorldRuntimeFrame,
    ) {
        let started = std::time::Instant::now();
        if frame.runtime_active && frame.streaming_enabled {
            crate::terrain_streaming::tick_authored_streaming_terrain(
                world,
                materials,
                thread_pool,
            );
        }
        if frame.environment_cycle_enabled {
            crate::authored_sky::tick_game_ready_sky_cycle(world, frame.dt);
        }
        crate::shadow_validation::tick_shadow_validation(world, frame.dt);
        emit_environment_runtime_profile(
            "engine.world.environment-runtime",
            frame.frame_index,
            started.elapsed().as_secs_f32() * 1000.0,
        );
    }
}

/// Installs the concrete authored-environment implementation owned by this domain runtime.
/// Product/game modules select the environment runtime but do not provide its internal tick adapter.
pub fn install_authored_environment_runtime_adapter(world: &mut World) {
    install_world_environment_runtime_adapter(world, Arc::new(AuthoredEnvironmentRuntimeAdapter));
}

pub struct WorldEnvironmentAdmissionWorldRuntimeProvider;

impl WorldEnvironmentAdmissionWorldRuntimeProvider {
    #[inline]
    pub fn shared() -> Arc<dyn WorldRuntimeProvider> {
        Arc::new(Self)
    }
}

impl WorldRuntimeProvider for WorldEnvironmentAdmissionWorldRuntimeProvider {
    fn id(&self) -> &'static str {
        "engine.world.environment-admission"
    }

    fn tick_prelaunch(
        &self,
        world: &mut World,
        primitives: &mut PrimitiveRegistry,
        materials: &MaterialRegistry,
        thread_pool: Option<&ThreadPoolHandle>,
        frame_index: u64,
    ) {
        let binding = world.resource::<WorldEnvironmentRuntimeBinding>().cloned();
        if let Some(binding) = binding {
            binding.0.tick_admission_prelaunch(
                world,
                primitives,
                materials,
                thread_pool,
                frame_index,
            );
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
        let binding = world.resource::<WorldEnvironmentRuntimeBinding>().cloned();
        if let Some(binding) = binding {
            binding
                .0
                .tick_admission_frame(world, primitives, materials, thread_pool, frame);
        }
    }
}

pub struct WorldEnvironmentSimulationWorldRuntimeProvider;

impl WorldEnvironmentSimulationWorldRuntimeProvider {
    #[inline]
    pub fn shared() -> Arc<dyn WorldRuntimeProvider> {
        Arc::new(Self)
    }
}

impl WorldRuntimeProvider for WorldEnvironmentSimulationWorldRuntimeProvider {
    fn id(&self) -> &'static str {
        "engine.world.environment-runtime"
    }

    fn tick_frame(
        &self,
        world: &mut World,
        primitives: &mut PrimitiveRegistry,
        materials: &MaterialRegistry,
        thread_pool: Option<&ThreadPoolHandle>,
        frame: WorldRuntimeFrame,
    ) {
        let binding = world.resource::<WorldEnvironmentRuntimeBinding>().cloned();
        if let Some(binding) = binding {
            binding
                .0
                .tick_environment_frame(world, primitives, materials, thread_pool, frame);
        }
    }
}
