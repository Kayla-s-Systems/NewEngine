use newengine_ecs::World;
use newengine_sim::{
    default_schedule, CommandBuffer, SimFrame, SimSchedule, SimStage, SimSystemBatchExecutor,
    SimSystemBatchResult, SimSystemCommandBatch, SimSystemJob, SimulationJobBatch,
    SimulationJobTelemetry,
};
#[cfg(test)]
use newengine_sim::{SimReadBatchExecutor, SimReadBatchReport, SimReadSnapshot};

use super::content::GameplayContentProviderRegistry;
use super::execution::{GameplayExecutionPhase, GameplayFrame, GameplaySystemProviderRegistry};
use super::physics::step_service_physics;
use super::physics_queries::GameplayPhysicsQueryProviderRegistry;
use newengine_core::physics::PhysicsApiRef;
use newengine_core::{TaskLane, TaskPriority, TaskRequest, ThreadPoolHandle};
use parking_lot::{Condvar, Mutex};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{
    atomic::{AtomicU64, Ordering as AtomicOrdering},
    Arc,
};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Default)]
pub struct SimulationScheduleTiming {
    pub input_ms: f32,
    pub controllers_ms: f32,
    pub apply_intents_ms: f32,
    pub content_install_ms: f32,
    pub capability_ensure_ms: f32,
    pub before_physics_ms: f32,
    pub physics_ms: f32,
    pub after_physics_ms: f32,
    pub derived_ms: f32,
    pub after_derived_ms: f32,
    pub capability_dispatch_ms: f32,
    pub capability_requested: usize,
    pub capability_executed: usize,
    pub capability_missing: usize,
    pub capability_failed: usize,
    pub animation_state_ms: f32,
    pub total_ms: f32,
}

/// Legacy metadata-only simulation read boundary retained for diagnostics/tests.
/// Real system execution uses `EngineJobsSimSystemExecutor` below and returns typed
/// command buffers for deterministic owner-thread commit.
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

struct EngineJobsSimSystemExecutor<'a> {
    thread_pool: &'a ThreadPoolHandle,
}

impl SimSystemBatchExecutor for EngineJobsSimSystemExecutor<'_> {
    fn run_system_batch(
        &self,
        batch: &SimulationJobBatch,
        world: Arc<World>,
        frame: SimFrame,
        systems: Vec<SimSystemJob>,
    ) -> SimSystemBatchResult {
        // Controller batches are intentionally small and latency-sensitive. Sending one or two
        // trivial controller systems through the shared engine worker queue and immediately
        // waiting for their body-ready barrier turns unrelated worker saturation into simulation
        // latency (the profiler observed 20..121 ms controller stalls). Keep those tiny batches on
        // the owner thread; wider batches still use engine.threading where independent work can
        // amortize scheduling and synchronization overhead.
        if should_inline_controller_batch(batch, &systems) {
            return run_inline_sim_system_batch(batch, world.as_ref(), frame, systems);
        }

        const BODY_JOIN_BUDGET: Duration = Duration::from_millis(8);
        const BODY_WAIT_SLICE: Duration = Duration::from_millis(1);
        const STALL_REPORT_INTERVAL: Duration = Duration::from_millis(250);

        let wall_started = Instant::now();
        let worker_cpu_ns = Arc::new(AtomicU64::new(0));
        let results = Arc::new((
            Mutex::new(Vec::<Option<(SimSystemCommandBatch, bool)>>::new()),
            Condvar::new(),
        ));
        results.0.lock().resize_with(systems.len(), || None);
        let mut tickets = Vec::with_capacity(systems.len());

        for (slot, system) in systems.into_iter().enumerate() {
            let world = Arc::clone(&world);
            let results = Arc::clone(&results);
            let worker_cpu_ns = Arc::clone(&worker_cpu_ns);
            let task_id = format!("{}.system.{}", batch.task_id, system.system_index);
            let system_name = system.name;
            let request = TaskRequest::new("simulation.system")
                .with_task_id(task_id.clone())
                .with_lane(TaskLane::Simulation)
                .with_priority(TaskPriority::Critical)
                .with_source("newengine-engine-runtime.sim")
                .with_owner("newengine-engine-runtime")
                .with_category("simulation-system")
                .with_frame_id(frame.fixed_tick)
                .with_dependency_group(batch.event_dependency_group())
                .with_task_domain(newengine_task_api::task_domain::ENGINE_SIMULATION)
                .with_task_pass(batch.stage.as_str())
                .cancellable(false);

            let ticket = self.thread_pool.submit_request(request, move || {
                let started = Instant::now();
                let mut commands = CommandBuffer::new();
                let body_result = catch_unwind(AssertUnwindSafe(|| {
                    (system.function)(world.as_ref(), frame, &mut commands);
                }));
                let elapsed = started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
                worker_cpu_ns.fetch_add(elapsed, AtomicOrdering::AcqRel);

                // Body-ready, not hierarchical TaskTicket completion, is the simulation
                // commit barrier. Release the World lease before publishing readiness so
                // Arc::try_unwrap on the owner thread is safe even if task-core bookkeeping
                // is still finalizing this task.
                drop(world);
                let panicked = body_result.is_err();
                if panicked {
                    // A failed system must not leak a partially-authored command buffer into
                    // the owner commit phase. The task remains completed from task-core's
                    // perspective; the body-result channel carries the precise failure state.
                    commands = CommandBuffer::new();
                }
                {
                    let mut guard = results.0.lock();
                    guard[slot] = Some((
                        SimSystemCommandBatch::new(system.system_index, system.name, commands),
                        panicked,
                    ));
                }
                results.1.notify_all();
            });
            tickets.push((task_id, system_name, ticket));
        }

        // Winit/engine owner thread must never perform an unbounded TaskTicket::wait().
        // We wait only for simulation-body publication, in bounded condition-variable
        // slices. Once a body exceeds the normal budget, emit exact task/system status.
        let mut next_stall_report = wall_started + BODY_JOIN_BUDGET;
        let mut guard = results.0.lock();
        loop {
            let stalled_slots = guard
                .iter()
                .enumerate()
                .filter_map(|(slot, result)| result.is_none().then_some(slot))
                .collect::<Vec<_>>();
            if stalled_slots.is_empty() {
                break;
            }

            let now = Instant::now();
            if now >= next_stall_report {
                drop(guard);
                let elapsed_ms = wall_started.elapsed().as_secs_f64() * 1_000.0;
                for slot in stalled_slots {
                    let (task_id, system_name, ticket) = &tickets[slot];
                    let status = ticket.status();
                    newengine_ulog_api::ulog::warn!(
                        "simulation body stall: batch='{}' task_id='{}' system='{}' stage='{}' frame={} elapsed_ms={:.2} phase={:?} lane='{}' pending_simulation={} policy='body-ready barrier; hierarchical TaskTicket join forbidden on engine thread'",
                        batch.task_id,
                        task_id,
                        system_name,
                        batch.stage.as_str(),
                        frame.fixed_tick,
                        elapsed_ms,
                        status.phase,
                        status.lane.as_str(),
                        self.thread_pool.pending_for_lane(TaskLane::Simulation),
                    );
                }
                next_stall_report = Instant::now() + STALL_REPORT_INTERVAL;
                guard = results.0.lock();
                continue;
            }

            let wait_for = BODY_WAIT_SLICE.min(next_stall_report.saturating_duration_since(now));
            results.1.wait_for(&mut guard, wait_for);
        }

        let worker_wall_time_ns =
            wall_started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
        let worker_cpu_time_ns = worker_cpu_ns.load(AtomicOrdering::Acquire);
        let commands = core::mem::take(&mut *guard)
            .into_iter()
            .enumerate()
            .map(|(slot, result)| {
                let (commands, panicked) = result
                    .expect("simulation body-ready barrier completed without a command-buffer result");
                if panicked {
                    let (task_id, system_name, _) = &tickets[slot];
                    newengine_ulog_api::ulog::error!(
                        "simulation system body panicked: batch='{}' task_id='{}' system='{}' stage='{}' frame={}; partial command buffer will remain deterministically ordered",
                        batch.task_id,
                        task_id,
                        system_name,
                        batch.stage.as_str(),
                        frame.fixed_tick,
                    );
                }
                commands
            })
            .collect();
        drop(guard);

        // Deliberately no wait(): lifecycle bookkeeping may finish independently after
        // body-ready publication and can no longer freeze the Winit thread.
        drop(tickets);

        SimSystemBatchResult::new(commands, worker_wall_time_ns, worker_cpu_time_ns)
    }
}

fn should_inline_controller_batch(batch: &SimulationJobBatch, systems: &[SimSystemJob]) -> bool {
    if batch.stage != SimStage::Controllers {
        return false;
    }
    // A singleton batch has no parallelism to exploit, so queueing it can only add latency.
    if systems.len() <= 1 {
        return true;
    }
    // The stock motor/orbit pair is deliberately tiny. Keep custom two-system batches eligible
    // for true parallel execution because they may contain materially expensive project work.
    systems.len() == 2
        && systems
            .iter()
            .any(|system| system.name == "character_motor")
        && systems.iter().any(|system| system.name == "orbit_camera")
}

fn run_inline_sim_system_batch(
    batch: &SimulationJobBatch,
    world: &World,
    frame: SimFrame,
    systems: Vec<SimSystemJob>,
) -> SimSystemBatchResult {
    let wall_started = Instant::now();
    let mut worker_cpu_ns = 0u64;
    let mut commands = Vec::with_capacity(systems.len());

    for system in systems {
        let started = Instant::now();
        let mut command_buffer = CommandBuffer::new();
        let body_result = catch_unwind(AssertUnwindSafe(|| {
            (system.function)(world, frame, &mut command_buffer);
        }));
        worker_cpu_ns = worker_cpu_ns
            .saturating_add(started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64);
        if body_result.is_err() {
            command_buffer = CommandBuffer::new();
            newengine_ulog_api::ulog::error!(
                "simulation inline system body panicked: batch='{}' system='{}' stage='{}' frame={}; command buffer discarded",
                batch.task_id,
                system.name,
                batch.stage.as_str(),
                frame.fixed_tick,
            );
        }
        commands.push(SimSystemCommandBatch::new(
            system.system_index,
            system.name,
            command_buffer,
        ));
    }

    let worker_wall_time_ns = wall_started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
    SimSystemBatchResult::new(commands, worker_wall_time_ns, worker_cpu_ns)
}

fn run_sim_stage(
    schedule: &mut SimSchedule,
    world: &mut World,
    stage: SimStage,
    frame: SimFrame,
    telemetry: Option<&SimulationJobTelemetry<'_>>,
    thread_pool: Option<&ThreadPoolHandle>,
) {
    if let Some(thread_pool) = thread_pool {
        let executor = EngineJobsSimSystemExecutor { thread_pool };
        schedule.run_stage_with_parallel_executor(world, stage, frame, telemetry, &executor);
    } else {
        schedule.run_stage_with_telemetry(world, stage, frame, telemetry);
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

#[allow(clippy::too_many_arguments)]
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

#[allow(clippy::too_many_arguments)]
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

#[allow(clippy::too_many_arguments)]
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
) -> SimulationScheduleTiming {
    let schedule_started = Instant::now();
    let mut timing = SimulationScheduleTiming::default();
    let frame = SimFrame::new(dt.max(0.0001), frame_index);
    let gameplay_frame = GameplayFrame::from(frame);

    let phase_started = Instant::now();
    run_sim_stage(
        schedule,
        world,
        SimStage::Input,
        frame,
        telemetry,
        thread_pool,
    );
    timing.input_ms = phase_started.elapsed().as_secs_f32() * 1000.0;

    let phase_started = Instant::now();
    run_sim_stage(
        schedule,
        world,
        SimStage::Controllers,
        frame,
        telemetry,
        thread_pool,
    );
    timing.controllers_ms = phase_started.elapsed().as_secs_f32() * 1000.0;

    let phase_started = Instant::now();
    schedule.run_stage_with_telemetry(world, SimStage::ApplyIntents, frame, telemetry);
    timing.apply_intents_ms = phase_started.elapsed().as_secs_f32() * 1000.0;

    // Character vitals consume semantic controller exertion at the fixed-step boundary.
    // This keeps local input, remote controllers and future AI on the same stamina contract.
    crate::gameplay::update_character_vitals(world, frame.dt);
    crate::gameplay::update_character_damage_states(world, frame.dt);

    // Profile-owned authored content is installed explicitly before gameplay execution.
    // Generic engine code never manufactures FPS items/loadouts as a fallback.
    let phase_started = Instant::now();
    gameplay_content.install_pending(world);
    timing.content_install_ms = phase_started.elapsed().as_secs_f32() * 1000.0;

    // Engine exposes capability implementations but never invokes them implicitly. Project/gameplay
    // code may enqueue requests; dispatch happens only after gameplay phases have committed.
    let phase_started = Instant::now();
    crate::gameplay::ensure_builtin_gameplay_capabilities(world);
    timing.capability_ensure_ms = phase_started.elapsed().as_secs_f32() * 1000.0;

    // Product/gameplay behavior is profile-owned. The engine only owns the stable
    // execution phase boundary and never names FPS, inventory, combat or missions.
    let phase_started = Instant::now();
    gameplay_systems.run_phase(GameplayExecutionPhase::BeforePhysics, world, gameplay_frame);
    timing.before_physics_ms = phase_started.elapsed().as_secs_f32() * 1000.0;

    let phase_started = Instant::now();
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
            run_sim_stage(
                schedule,
                world,
                SimStage::Physics,
                frame,
                telemetry,
                thread_pool,
            );
        }
    }
    timing.physics_ms = phase_started.elapsed().as_secs_f32() * 1000.0;

    let phase_started = Instant::now();
    gameplay_systems.run_phase(GameplayExecutionPhase::AfterPhysics, world, gameplay_frame);
    timing.after_physics_ms = phase_started.elapsed().as_secs_f32() * 1000.0;

    let phase_started = Instant::now();
    run_sim_stage(
        schedule,
        world,
        SimStage::Derived,
        frame,
        telemetry,
        thread_pool,
    );
    timing.derived_ms = phase_started.elapsed().as_secs_f32() * 1000.0;

    let phase_started = Instant::now();
    gameplay_systems.run_phase(GameplayExecutionPhase::AfterDerived, world, gameplay_frame);
    timing.after_derived_ms = phase_started.elapsed().as_secs_f32() * 1000.0;

    let phase_started = Instant::now();
    let capability_report = crate::gameplay::dispatch_gameplay_capabilities(world);
    timing.capability_requested = capability_report.requested;
    timing.capability_executed = capability_report.executed;
    timing.capability_missing = capability_report.missing.len();
    timing.capability_failed = capability_report.failed.len();
    for capability in capability_report.missing {
        newengine_ulog_api::ulog::warn!(
            "gameplay capability provider missing capability='{}'",
            capability
        );
    }
    for failure in capability_report.failed {
        newengine_ulog_api::ulog::warn!("gameplay capability invocation failed {}", failure);
    }
    timing.capability_dispatch_ms = phase_started.elapsed().as_secs_f32() * 1000.0;

    // Character locomotion animation state is derived from authoritative post-physics
    // motion/grounding. Possessed players and AI/NPC actors share the same semantic contract.
    let phase_started = Instant::now();
    crate::gameplay::update_character_animation_states(world, frame.dt);
    timing.animation_state_ms = phase_started.elapsed().as_secs_f32() * 1000.0;
    timing.total_ms = schedule_started.elapsed().as_secs_f32() * 1000.0;
    timing
}

#[inline]
pub fn default_sim_schedule() -> SimSchedule {
    default_schedule()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ParallelProbe {
        state: Arc<ParallelProbeState>,
    }

    struct ParallelProbeState {
        started: std::sync::Mutex<usize>,
        wake: std::sync::Condvar,
        timed_out: std::sync::atomic::AtomicBool,
    }

    fn parallel_probe_system(world: &World, _frame: SimFrame, _commands: &mut CommandBuffer) {
        use std::sync::atomic::Ordering;
        use std::time::Duration;

        let probe = world
            .resource::<ParallelProbe>()
            .expect("parallel probe missing");
        let mut started = probe
            .state
            .started
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *started += 1;
        probe.state.wake.notify_all();
        let (started, _) = probe
            .state
            .wake
            .wait_timeout_while(started, Duration::from_millis(500), |started| *started < 2)
            .unwrap_or_else(|e| e.into_inner());
        if *started < 2 {
            probe.state.timed_out.store(true, Ordering::Release);
        }
    }

    #[test]
    fn tiny_stock_controller_batches_stay_inline_but_custom_pairs_remain_parallel_eligible() {
        use newengine_sim::AccessMask;

        let batch = SimulationJobBatch::new(
            SimStage::Controllers,
            SimFrame::new(0.016, 77),
            0,
            1,
            2,
            "engine.threading",
        );
        let system = |system_index, name| SimSystemJob {
            system_index,
            order: system_index as i32,
            seq: system_index as u32,
            name,
            access: AccessMask::write(system_index as u32),
            function: parallel_probe_system,
        };

        assert!(should_inline_controller_batch(
            &batch,
            &[system(0, "character_motor")]
        ));
        assert!(should_inline_controller_batch(
            &batch,
            &[system(0, "character_motor"), system(1, "orbit_camera")]
        ));
        assert!(!should_inline_controller_batch(
            &batch,
            &[system(0, "project_ai"), system(1, "project_camera")]
        ));
    }

    #[test]
    fn engine_threading_executor_runs_independent_simulation_jobs_concurrently() {
        use newengine_core::{ThreadPoolConfig, ThreadPoolManager};
        use newengine_sim::AccessMask;
        use std::sync::atomic::Ordering;

        let state = Arc::new(ParallelProbeState {
            started: std::sync::Mutex::new(0),
            wake: std::sync::Condvar::new(),
            timed_out: std::sync::atomic::AtomicBool::new(false),
        });
        let mut world = World::new();
        world.insert_resource(ParallelProbe {
            state: Arc::clone(&state),
        });

        let mut pool = ThreadPoolManager::new(ThreadPoolConfig::fixed(2));
        let handle = pool.handle();
        let executor = EngineJobsSimSystemExecutor {
            thread_pool: &handle,
        };
        let batch = SimulationJobBatch::new(
            SimStage::Controllers,
            SimFrame::new(0.016, 77),
            0,
            1,
            2,
            "engine.threading",
        );
        let systems = vec![
            SimSystemJob {
                system_index: 0,
                order: 10,
                seq: 1,
                name: "parallel_probe_gameplay",
                access: AccessMask::write(0),
                function: parallel_probe_system,
            },
            SimSystemJob {
                system_index: 1,
                order: 20,
                seq: 2,
                name: "parallel_probe_camera",
                access: AccessMask::write(1),
                function: parallel_probe_system,
            },
        ];

        let result =
            executor.run_system_batch(&batch, Arc::new(world), SimFrame::new(0.016, 77), systems);

        assert_eq!(result.commands.len(), 2);
        assert!(result.worker_wall_time_ns > 0);
        assert!(result.worker_cpu_time_ns > 0);
        assert_eq!(*state.started.lock().unwrap_or_else(|e| e.into_inner()), 2);
        assert!(
            !state.timed_out.load(Ordering::Acquire),
            "simulation systems did not overlap on the worker pool"
        );
        assert_eq!(handle.pending_for_lane(TaskLane::Simulation), 0);

        pool.shutdown_and_join();
    }

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
