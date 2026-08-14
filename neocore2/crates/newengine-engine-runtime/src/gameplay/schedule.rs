use newengine_ecs::World;
use newengine_sim::{default_schedule, SimFrame, SimSchedule, SimStage, SimulationJobTelemetry};
#[cfg(test)]
use newengine_sim::{
    SimReadBatchExecutor, SimReadBatchReport, SimReadSnapshot, SimulationJobBatch,
};

use super::content::GameplayContentProviderRegistry;
use super::execution::{GameplayExecutionPhase, GameplayFrame, GameplaySystemProviderRegistry};
use super::physics::step_service_physics;
use super::physics_queries::GameplayPhysicsQueryProviderRegistry;
use newengine_core::physics::PhysicsApiRef;
use newengine_core::ThreadPoolHandle;

/// Engine-runtime adapter for the simulation read boundary.
///
/// The snapshot pass currently produces metadata only; the actual simulation systems
/// execute immediately afterwards on the world-owner thread. Scheduling this tiny pass
/// onto `engine.threading` and waiting for it synchronously creates a context-switch,
/// allocation and lock barrier with no parallel progress, so it stays inline until the
/// scheduler can return real worker-produced command batches.
#[cfg(test)]
struct EngineJobsSimReadExecutor;

#[cfg(test)]
impl SimReadBatchExecutor for EngineJobsSimReadExecutor {
    fn run_read_batch(
        &self,
        batch: &SimulationJobBatch,
        snapshot: SimReadSnapshot,
    ) -> SimReadBatchReport {
        SimReadBatchReport::from_snapshot(&snapshot, batch.batch_index)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhysicsIntegrationMode {
    ServiceBackend,
    EcsFallback,
}

pub fn run_schedule(
    schedule: &mut SimSchedule,
    gameplay_content: &mut GameplayContentProviderRegistry,
    gameplay_systems: &GameplaySystemProviderRegistry,
    gameplay_physics_queries: &GameplayPhysicsQueryProviderRegistry,
    world: &mut World,
    dt: f32,
    physics_api: Option<&PhysicsApiRef>,
) {
    run_schedule_with_physics_mode(
        schedule,
        gameplay_content,
        gameplay_systems,
        gameplay_physics_queries,
        world,
        dt,
        physics_api,
        PhysicsIntegrationMode::ServiceBackend,
    );
}

pub fn run_schedule_with_physics_mode(
    schedule: &mut SimSchedule,
    gameplay_content: &mut GameplayContentProviderRegistry,
    gameplay_systems: &GameplaySystemProviderRegistry,
    gameplay_physics_queries: &GameplayPhysicsQueryProviderRegistry,
    world: &mut World,
    dt: f32,
    physics_api: Option<&PhysicsApiRef>,
    physics_mode: PhysicsIntegrationMode,
) {
    run_schedule_with_physics_mode_and_telemetry(
        schedule,
        gameplay_content,
        gameplay_systems,
        gameplay_physics_queries,
        world,
        dt,
        physics_api,
        physics_mode,
        None,
    );
}

pub fn run_schedule_with_physics_mode_and_telemetry(
    schedule: &mut SimSchedule,
    gameplay_content: &mut GameplayContentProviderRegistry,
    gameplay_systems: &GameplaySystemProviderRegistry,
    gameplay_physics_queries: &GameplayPhysicsQueryProviderRegistry,
    world: &mut World,
    dt: f32,
    physics_api: Option<&PhysicsApiRef>,
    physics_mode: PhysicsIntegrationMode,
    telemetry: Option<&SimulationJobTelemetry<'_>>,
) {
    run_schedule_with_physics_mode_and_telemetry_for_frame(
        schedule,
        gameplay_content,
        gameplay_systems,
        gameplay_physics_queries,
        world,
        dt,
        0,
        physics_api,
        physics_mode,
        telemetry,
        None,
    );
}

pub fn run_schedule_with_physics_mode_and_telemetry_for_frame(
    schedule: &mut SimSchedule,
    gameplay_content: &mut GameplayContentProviderRegistry,
    gameplay_systems: &GameplaySystemProviderRegistry,
    gameplay_physics_queries: &GameplayPhysicsQueryProviderRegistry,
    world: &mut World,
    dt: f32,
    frame_index: u64,
    physics_api: Option<&PhysicsApiRef>,
    physics_mode: PhysicsIntegrationMode,
    telemetry: Option<&SimulationJobTelemetry<'_>>,
    thread_pool: Option<&ThreadPoolHandle>,
) {
    let frame = SimFrame::new(dt.max(0.0001), frame_index);
    let gameplay_frame = GameplayFrame::from(frame);
    // There is no worker-produced simulation command batch yet. Passing a synthetic
    // executor here only materializes SimReadSnapshot metadata and then consumes it
    // inline on the owner thread, adding allocations without parallel progress.
    // Keep the boundary dormant until a real async executor exists.
    let _ = thread_pool;
    let sim_executor_ref = None;

    schedule.run_stage_with_telemetry_and_executor(
        world,
        SimStage::Input,
        frame,
        telemetry,
        sim_executor_ref,
    );
    schedule.run_stage_with_telemetry_and_executor(
        world,
        SimStage::Controllers,
        frame,
        telemetry,
        sim_executor_ref,
    );
    schedule.run_stage_with_telemetry(world, SimStage::ApplyIntents, frame, telemetry);

    // Profile-owned authored content is installed explicitly before gameplay execution.
    // Generic engine code never manufactures FPS items/loadouts as a fallback.
    gameplay_content.install_pending(world);

    // Product/gameplay behavior is profile-owned. The engine only owns the stable
    // execution phase boundary and never names FPS, inventory, combat or missions.
    gameplay_systems.run_phase(GameplayExecutionPhase::BeforePhysics, world, gameplay_frame);

    match physics_mode {
        PhysicsIntegrationMode::ServiceBackend => {
            // Service-backed physics owns integration for PhysicsBodyDesc entities.
            // Do not run the default in-process SimStage::Physics velocity integrator
            // here, otherwise controlled characters are moved once by ECS and again by
            // the backend provider.
            step_service_physics(world, frame.dt, physics_api, gameplay_physics_queries);
        }
        PhysicsIntegrationMode::EcsFallback => {
            // Declarative safe-profile fallback: keep gameplay controls responsive
            // without entering the native physics provider path. This is a capability
            // downgrade, not a game-specific shortcut.
            schedule.run_stage_with_telemetry_and_executor(
                world,
                SimStage::Physics,
                frame,
                telemetry,
                sim_executor_ref,
            );
        }
    }

    gameplay_systems.run_phase(GameplayExecutionPhase::AfterPhysics, world, gameplay_frame);

    schedule.run_stage_with_telemetry_and_executor(
        world,
        SimStage::Derived,
        frame,
        telemetry,
        sim_executor_ref,
    );

    gameplay_systems.run_phase(GameplayExecutionPhase::AfterDerived, world, gameplay_frame);
}

#[inline]
pub fn default_sim_schedule() -> SimSchedule {
    default_schedule()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simulation_read_boundary_is_inline_and_preserves_batch_metadata() {
        let batch = SimulationJobBatch::new(
            SimStage::Controllers,
            SimFrame::new(0.016, 42),
            0,
            1,
            0,
            "engine.threading-inline",
        );
        let snapshot = SimReadSnapshot {
            frame: SimFrame::new(0.016, 42),
            stage: SimStage::Controllers,
            world: newengine_sim::SimWorldSnapshotHeader::default(),
            systems: Vec::new(),
            dependency_group: batch.event_dependency_group(),
        };

        let report = EngineJobsSimReadExecutor.run_read_batch(&batch, snapshot);

        assert_eq!(report.frame.fixed_tick, 42);
        assert_eq!(report.batch_index, 0);
        assert_eq!(report.stage, SimStage::Controllers);
        assert!(report.worker_safe);
    }
}
