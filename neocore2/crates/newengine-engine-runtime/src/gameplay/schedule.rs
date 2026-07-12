use newengine_ecs::World;
use newengine_sim::{
    default_schedule, SimFrame, SimReadBatchExecutor, SimReadBatchReport, SimReadSnapshot,
    SimSchedule, SimStage, SimulationJobBatch, SimulationJobTelemetry,
};

use super::combat::step_player_combat;
use super::fps_demo::step_fps_demo_gameplay;
use super::inventory::step_world_items;
use super::inventory_hud::step_inventory_commands;
use super::physics::step_service_physics;
use super::player::apply_player_fixed_commands;
use newengine_core::physics::PhysicsApiRef;
use newengine_core::{TaskLane, TaskPriority, TaskRequest, ThreadPoolHandle};

struct EngineJobsSimReadExecutor<'a> {
    jobs: &'a ThreadPoolHandle,
}

impl SimReadBatchExecutor for EngineJobsSimReadExecutor<'_> {
    fn run_read_batch(
        &self,
        batch: &SimulationJobBatch,
        snapshot: SimReadSnapshot,
        job: Box<dyn FnOnce(SimReadSnapshot) -> SimReadBatchReport + Send + 'static>,
    ) -> SimReadBatchReport {
        let fallback = SimReadBatchReport::from_snapshot(&snapshot, batch.batch_index);
        let result = std::sync::Arc::new(parking_lot::Mutex::new(None::<SimReadBatchReport>));
        let result_for_job = std::sync::Arc::clone(&result);
        let request = TaskRequest::new("simulation.read-snapshot")
            .with_source("newengine-engine-runtime.sim")
            .with_owner("newengine-engine-runtime")
            .with_category("simulation-read-batch")
            .with_lane(TaskLane::Simulation)
            .with_priority(TaskPriority::Interactive)
            .with_task_id(batch.task_id.clone())
            .with_frame_id(batch.fixed_tick)
            .with_dependency_group(batch.event_dependency_group())
            .with_task_domain(newengine_task_api::task_domain::ENGINE_SIMULATION)
            .with_task_pass(batch.stage.as_str());
        let ticket = self.jobs.submit_request(request, move || {
            *result_for_job.lock() = Some(job(snapshot));
        });
        ticket.wait();
        let mut guard = result.lock();
        guard.take().unwrap_or(fallback)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhysicsIntegrationMode {
    ServiceBackend,
    EcsFallback,
}

pub fn run_schedule(
    schedule: &mut SimSchedule,
    world: &mut World,
    dt: f32,
    physics_api: Option<&PhysicsApiRef>,
) {
    run_schedule_with_physics_mode(
        schedule,
        world,
        dt,
        physics_api,
        PhysicsIntegrationMode::ServiceBackend,
    );
}

pub fn run_schedule_with_physics_mode(
    schedule: &mut SimSchedule,
    world: &mut World,
    dt: f32,
    physics_api: Option<&PhysicsApiRef>,
    physics_mode: PhysicsIntegrationMode,
) {
    run_schedule_with_physics_mode_and_telemetry(
        schedule,
        world,
        dt,
        physics_api,
        physics_mode,
        None,
    );
}

pub fn run_schedule_with_physics_mode_and_telemetry(
    schedule: &mut SimSchedule,
    world: &mut World,
    dt: f32,
    physics_api: Option<&PhysicsApiRef>,
    physics_mode: PhysicsIntegrationMode,
    telemetry: Option<&SimulationJobTelemetry<'_>>,
) {
    run_schedule_with_physics_mode_and_telemetry_for_frame(
        schedule,
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
    world: &mut World,
    dt: f32,
    frame_index: u64,
    physics_api: Option<&PhysicsApiRef>,
    physics_mode: PhysicsIntegrationMode,
    telemetry: Option<&SimulationJobTelemetry<'_>>,
    thread_pool: Option<&ThreadPoolHandle>,
) {
    let frame = SimFrame::new(dt.max(0.0001), frame_index);
    let sim_executor = thread_pool.map(|jobs| EngineJobsSimReadExecutor { jobs });
    let sim_executor_ref = sim_executor
        .as_ref()
        .map(|executor| executor as &dyn SimReadBatchExecutor);

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
    apply_player_fixed_commands(world, frame.dt, frame.fixed_tick);
    step_inventory_commands(world, frame.fixed_tick);
    step_world_items(world, frame.dt);
    step_player_combat(world, frame.dt, frame.fixed_tick);
    match physics_mode {
        PhysicsIntegrationMode::ServiceBackend => {
            // Service-backed physics owns integration for PhysicsBodyDesc entities.
            // Do not run the default in-process SimStage::Physics velocity integrator
            // here, otherwise controlled characters are moved once by ECS and again by
            // the backend provider.
            step_service_physics(world, frame.dt, physics_api);
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
    schedule.run_stage_with_telemetry_and_executor(
        world,
        SimStage::Derived,
        frame,
        telemetry,
        sim_executor_ref,
    );

    step_fps_demo_gameplay(world, frame.dt);
}

#[inline]
pub fn default_sim_schedule() -> SimSchedule {
    default_schedule()
}
