#![forbid(unsafe_op_in_unsafe_fn)]

use core::cmp::Ordering;
use std::sync::Arc;
use std::time::Instant;

use newengine_ecs::World;
use newengine_task_api::{task_domain, task_pass, EngineTaskEvent, EngineTaskPhase};
use serde::{Deserialize, Serialize};

use crate::{
    access::{AccessConflictMask, AccessDomain, AccessMask},
    commands::CommandBuffer,
    systems, SimFrame,
};

/// Simulation stages.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SimStage {
    /// Inputs are produced externally (winit/plugin) and written into components/resources.
    Input = 0,
    /// Controllers translate inputs to semantic intents only.
    Controllers = 1,
    /// A single, ordered stage applies controller intents to ECS state.
    ApplyIntents = 2,
    /// Kinematic integration / physics.
    Physics = 3,
    /// Derived world state (transforms, bounds, scene caches).
    Derived = 4,
}

impl SimStage {
    pub const COUNT: usize = 5;

    #[inline]
    pub const fn as_usize(self) -> usize {
        self as usize
    }

    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Input => task_pass::INPUT,
            Self::Controllers => task_pass::CONTROLLERS,
            Self::ApplyIntents => task_pass::APPLY_INTENTS,
            Self::Physics => task_pass::PHYSICS,
            Self::Derived => task_pass::DERIVED,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulationJobBatch {
    pub task_id: String,
    pub stage: SimStage,
    pub fixed_tick: u64,
    pub batch_index: usize,
    pub batch_count: usize,
    pub system_count: usize,
    pub executor: String,
}

impl SimulationJobBatch {
    pub fn new(
        stage: SimStage,
        frame: SimFrame,
        batch_index: usize,
        batch_count: usize,
        system_count: usize,
        executor: &'static str,
    ) -> Self {
        Self {
            task_id: format!(
                "simulation.{}.tick.{}.batch.{}",
                stage.as_str(),
                frame.fixed_tick,
                batch_index
            ),
            stage,
            fixed_tick: frame.fixed_tick,
            batch_index,
            batch_count,
            system_count,
            executor: executor.to_owned(),
        }
    }

    #[inline]
    pub fn event_dependency_group(&self) -> String {
        format!(
            "simulation.frame.{}.{}",
            self.fixed_tick,
            self.stage.as_str()
        )
    }

    pub fn event(
        &self,
        phase: EngineTaskPhase,
        status: impl Into<String>,
        detail: impl Into<String>,
        progress_01: Option<f32>,
    ) -> EngineTaskEvent {
        let mut event = EngineTaskEvent::new(
            self.task_id.clone(),
            "newengine-sim.schedule",
            "newengine-sim",
            "simulation",
            format!(
                "simulation:{}:batch:{}",
                self.stage.as_str(),
                self.batch_index
            ),
            "simulation",
            phase,
            status,
            detail,
        )
        .with_controls(false, false)
        .with_frame_id(self.fixed_tick)
        .with_dependency_group(self.event_dependency_group())
        .with_task_domain(task_domain::ENGINE_SIMULATION)
        .with_task_pass(self.stage.as_str())
        .with_priority("interactive")
        .with_executor(self.executor.clone());
        if let Some(progress) = progress_01 {
            event = event.with_progress(progress);
        }
        event
    }
}

pub struct SimulationJobTelemetry<'a> {
    publish: &'a dyn Fn(EngineTaskEvent),
}

impl<'a> SimulationJobTelemetry<'a> {
    #[inline]
    pub fn new(publish: &'a dyn Fn(EngineTaskEvent)) -> Self {
        Self { publish }
    }

    #[inline]
    pub fn publish(&self, event: EngineTaskEvent) {
        (self.publish)(event);
    }

    pub fn publish_batch(
        &self,
        batch: &SimulationJobBatch,
        phase: EngineTaskPhase,
        status: impl Into<String>,
        detail: impl Into<String>,
        progress_01: Option<f32>,
    ) {
        self.publish(batch.event(phase, status, detail, progress_01));
    }
}

/// Worker-safe summary of ECS world state captured on the world-owner thread.
///
/// This intentionally contains only serializable facts. The concrete `World`,
/// storages, resources and native component references never cross into worker
/// jobs. Domain workers consume this DTO and return command/intent batches for
/// the owner-thread apply stage.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimWorldSnapshotHeader {
    pub world_tick: u64,
    pub entity_count: usize,
    pub storage_count: usize,
    pub resource_count: usize,
    pub entities_changed_tick: u64,
}

impl SimWorldSnapshotHeader {
    #[inline]
    pub fn capture(world: &World) -> Self {
        Self {
            world_tick: world.tick(),
            entity_count: world.entity_count(),
            storage_count: world.storage_count(),
            resource_count: world.resource_count(),
            entities_changed_tick: world.entities_changed_tick(),
        }
    }
}

/// Serializable descriptor for a scheduled simulation system.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimReadSystemDescriptor {
    pub order: i32,
    pub seq: u32,
    pub name: String,
    pub access: AccessMask,
}

impl SimReadSystemDescriptor {
    #[inline]
    fn from_entry(entry: &SystemEntry) -> Self {
        Self {
            order: entry.order,
            seq: entry.seq,
            name: entry.name.to_owned(),
            access: entry.access,
        }
    }
}

/// Immutable, serializable frame snapshot for worker-safe simulation batches.
///
/// Canonical boundary:
///
/// ```text
/// world-owner capture -> SimReadSnapshot DTO -> host scheduling policy
///                       -> future worker command batches -> world-owner apply stage
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimReadSnapshot {
    pub frame: SimFrame,
    pub stage: SimStage,
    pub world: SimWorldSnapshotHeader,
    pub systems: Vec<SimReadSystemDescriptor>,
    pub dependency_group: String,
}

impl SimReadSnapshot {
    fn capture(world: &World, frame: SimFrame, stage: SimStage, systems: &[SystemEntry]) -> Self {
        Self {
            frame,
            stage,
            world: SimWorldSnapshotHeader::capture(world),
            systems: systems
                .iter()
                .map(SimReadSystemDescriptor::from_entry)
                .collect(),
            dependency_group: format!("simulation.frame.{}.{}", frame.fixed_tick, stage.as_str()),
        }
    }

    #[inline]
    pub fn system_count(&self) -> usize {
        self.systems.len()
    }

    #[inline]
    pub fn is_worker_safe(&self) -> bool {
        true
    }
}

/// Serializable command-batch header visible to jobs/profiler.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SimCommandBatchHeader {
    pub frame: SimFrame,
    pub stage: SimStage,
    pub batch_index: usize,
    pub command_count: usize,
    pub dependency_group: String,
}

/// Ordered command batch produced by simulation workers and consumed only by the
/// world-owner apply stage. The command payload itself remains typed Rust and is
/// never serialized; only the header crosses diagnostics/profiler surfaces.
pub struct SimCommandBatch {
    pub header: SimCommandBatchHeader,
    pub commands: CommandBuffer,
}

impl SimCommandBatch {
    #[inline]
    pub fn new(
        frame: SimFrame,
        stage: SimStage,
        batch_index: usize,
        commands: CommandBuffer,
        dependency_group: impl Into<String>,
    ) -> Self {
        let command_count = commands.len();
        Self {
            header: SimCommandBatchHeader {
                frame,
                stage,
                batch_index,
                command_count,
                dependency_group: dependency_group.into(),
            },
            commands,
        }
    }
}

/// Result of a worker-visible read-only simulation batch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SimReadBatchReport {
    pub frame: SimFrame,
    pub stage: SimStage,
    pub batch_index: usize,
    pub system_count: usize,
    pub worker_safe: bool,
    pub dependency_group: String,
}

impl SimReadBatchReport {
    #[inline]
    pub fn from_snapshot(snapshot: &SimReadSnapshot, batch_index: usize) -> Self {
        Self {
            frame: snapshot.frame,
            stage: snapshot.stage,
            batch_index,
            system_count: snapshot.system_count(),
            worker_safe: snapshot.is_worker_safe(),
            dependency_group: snapshot.dependency_group.clone(),
        }
    }
}

/// Host-owned policy boundary for processing immutable simulation read snapshots.
///
/// `newengine-sim` deliberately depends only on this trait, not on
/// `newengine-core`, so the simulation crate stays provider/runtime agnostic.
/// Implementations should enqueue work only when it can make independent progress;
/// a queue-and-immediate-wait round trip adds latency without parallelism.
pub trait SimReadBatchExecutor {
    fn run_read_batch(
        &self,
        batch: &SimulationJobBatch,
        snapshot: SimReadSnapshot,
    ) -> SimReadBatchReport;
}

/// Worker-executable simulation system descriptor. The function itself remains a
/// static Rust function pointer; ECS ownership is transferred into an `Arc<World>`
/// only for the duration of one conflict-free batch.
#[derive(Clone, Copy)]
pub struct SimSystemJob {
    pub system_index: usize,
    pub order: i32,
    pub seq: u32,
    pub name: &'static str,
    pub access: AccessMask,
    pub function: SystemFn,
}

/// One worker-produced command buffer. Results are committed by the world-owner
/// thread in stable system order, never by worker threads.
pub struct SimSystemCommandBatch {
    pub system_index: usize,
    pub system_name: &'static str,
    pub commands: CommandBuffer,
}

impl SimSystemCommandBatch {
    #[inline]
    pub fn new(system_index: usize, system_name: &'static str, commands: CommandBuffer) -> Self {
        Self {
            system_index,
            system_name,
            commands,
        }
    }
}

/// Timed result from a host-owned parallel simulation batch.
pub struct SimSystemBatchResult {
    pub commands: Vec<SimSystemCommandBatch>,
    /// Wall-clock interval from first submission until all jobs completed.
    pub worker_wall_time_ns: u64,
    /// Sum of time spent inside the system functions across all workers.
    pub worker_cpu_time_ns: u64,
}

impl SimSystemBatchResult {
    #[inline]
    pub fn new(
        commands: Vec<SimSystemCommandBatch>,
        worker_wall_time_ns: u64,
        worker_cpu_time_ns: u64,
    ) -> Self {
        Self {
            commands,
            worker_wall_time_ns,
            worker_cpu_time_ns,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SimAccessConflictDiagnostic {
    pub incoming_system: String,
    pub conflicting_systems: Vec<String>,
    pub mask: AccessConflictMask,
    pub named_domains: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SimBatchDiagnostics {
    pub frame: SimFrame,
    pub stage: SimStage,
    pub batch_index: usize,
    pub batch_width: usize,
    pub conflict_before: Option<SimAccessConflictDiagnostic>,
    pub worker_wall_time_ns: u64,
    pub worker_cpu_time_ns: u64,
    pub owner_commit_time_ns: u64,
    /// Approximate worker utilization: sum(worker CPU) / (wall * batch width).
    pub parallel_efficiency_01: f32,
}

/// Host-owned execution boundary for real simulation work. Implementations must
/// complete all submitted jobs before returning and must not retain `world`; this
/// allows the scheduler to reclaim sole ownership and perform deterministic commit.
pub trait SimSystemBatchExecutor {
    fn run_system_batch(
        &self,
        batch: &SimulationJobBatch,
        world: Arc<World>,
        frame: SimFrame,
        systems: Vec<SimSystemJob>,
    ) -> SimSystemBatchResult;
}

/// System function signature.
///
/// Systems must be deterministic and side-effect free outside of the provided command buffer.
pub type SystemFn = fn(&World, SimFrame, &mut CommandBuffer);

#[derive(Clone, Copy)]
struct SystemEntry {
    order: i32,
    seq: u32,
    name: &'static str,
    access: AccessMask,
    f: SystemFn,
}

/// A minimal deterministic scheduler.
///
/// - stable ordering by `(order, seq)`
/// - deterministic parallel batching by access mask
pub struct SimSchedule {
    stages: [Vec<SystemEntry>; SimStage::COUNT],
    is_sorted: [bool; SimStage::COUNT],
    next_seq: u32,
}

impl Default for SimSchedule {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl SimSchedule {
    #[inline]
    pub fn new() -> Self {
        Self {
            stages: core::array::from_fn(|_| Vec::new()),
            is_sorted: [false; SimStage::COUNT],
            next_seq: 1,
        }
    }

    #[inline]
    pub fn add_system(
        &mut self,
        stage: SimStage,
        order: i32,
        name: &'static str,
        access: AccessMask,
        f: SystemFn,
    ) {
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1).max(1);

        let idx = stage.as_usize();
        self.stages[idx].push(SystemEntry {
            order,
            seq,
            name,
            access,
            f,
        });
        self.is_sorted[idx] = false;
    }

    #[inline]
    fn sort_if_needed(&mut self) {
        for (i, v) in self.stages.iter_mut().enumerate() {
            if self.is_sorted[i] {
                continue;
            }
            v.sort_unstable_by(|a, b| match a.order.cmp(&b.order) {
                Ordering::Equal => a.seq.cmp(&b.seq),
                o => o,
            });
            self.is_sorted[i] = true;
        }
    }

    #[inline]
    pub fn run_stage(&mut self, world: &mut World, stage: SimStage, frame: SimFrame) {
        self.run_stage_with_telemetry_and_executor(world, stage, frame, None, None);
    }

    #[inline]
    pub fn run_stage_with_telemetry(
        &mut self,
        world: &mut World,
        stage: SimStage,
        frame: SimFrame,
        telemetry: Option<&SimulationJobTelemetry<'_>>,
    ) {
        self.run_stage_with_telemetry_and_executor(world, stage, frame, telemetry, None);
    }

    pub fn run_stage_with_telemetry_and_executor(
        &mut self,
        world: &mut World,
        stage: SimStage,
        frame: SimFrame,
        telemetry: Option<&SimulationJobTelemetry<'_>>,
        executor: Option<&dyn SimReadBatchExecutor>,
    ) {
        self.sort_if_needed();

        let systems = &self.stages[stage.as_usize()];
        if systems.is_empty() {
            return;
        }

        run_stage_single_thread(world, stage, systems, frame, telemetry, executor);
    }

    /// Executes conflict-free system batches through the host executor. Systems
    /// that conflict according to `AccessMask` are separated by an owner-thread
    /// commit barrier, so later conflicting systems observe earlier writes exactly
    /// as they did in the serial scheduler.
    pub fn run_stage_with_parallel_executor(
        &mut self,
        world: &mut World,
        stage: SimStage,
        frame: SimFrame,
        telemetry: Option<&SimulationJobTelemetry<'_>>,
        executor: &dyn SimSystemBatchExecutor,
    ) -> Vec<SimBatchDiagnostics> {
        self.sort_if_needed();

        let systems = &self.stages[stage.as_usize()];
        if systems.is_empty() {
            return Vec::new();
        }

        run_stage_parallel(world, stage, systems, frame, telemetry, executor)
    }

    #[inline]
    pub fn run_default_pipeline(&mut self, world: &mut World, frame: SimFrame) {
        self.run_default_pipeline_with_telemetry(world, frame, None);
    }

    pub fn run_default_pipeline_with_telemetry(
        &mut self,
        world: &mut World,
        frame: SimFrame,
        telemetry: Option<&SimulationJobTelemetry<'_>>,
    ) {
        self.run_stage_with_telemetry(world, SimStage::Input, frame, telemetry);
        self.run_stage_with_telemetry(world, SimStage::Controllers, frame, telemetry);
        self.run_stage_with_telemetry(world, SimStage::ApplyIntents, frame, telemetry);
        self.run_stage_with_telemetry(world, SimStage::Physics, frame, telemetry);
        self.run_stage_with_telemetry(world, SimStage::Derived, frame, telemetry);
    }
}

struct PlannedBatch {
    indices: Vec<usize>,
    conflict_before: Option<SimAccessConflictDiagnostic>,
}

fn named_domains(mask: u128) -> Vec<String> {
    AccessDomain::all()
        .into_iter()
        .filter(|domain| mask & domain.mask() != 0)
        .map(|domain| domain.as_str().to_owned())
        .collect()
}

fn conflict_diagnostic(
    systems: &[SystemEntry],
    current: &[usize],
    incoming_index: usize,
) -> SimAccessConflictDiagnostic {
    let incoming = systems[incoming_index];
    let mut aggregate = AccessConflictMask::default();
    let mut conflicting_systems = Vec::new();

    for &index in current {
        let existing = systems[index];
        let mask = existing.access.conflict_mask(incoming.access);
        if !mask.is_empty() {
            aggregate = aggregate.union(mask);
            conflicting_systems.push(existing.name.to_owned());
        }
    }

    SimAccessConflictDiagnostic {
        incoming_system: incoming.name.to_owned(),
        conflicting_systems,
        mask: aggregate,
        named_domains: named_domains(aggregate.blocking_mask()),
    }
}

fn plan_conflict_free_batches(systems: &[SystemEntry]) -> Vec<PlannedBatch> {
    let mut batches = Vec::<PlannedBatch>::new();
    let mut current = Vec::<usize>::new();
    let mut current_access = AccessMask::none();
    let mut conflict_before = None;

    for (index, system) in systems.iter().enumerate() {
        if !current.is_empty() && current_access.conflicts(system.access) {
            let next_conflict = conflict_diagnostic(systems, &current, index);
            batches.push(PlannedBatch {
                indices: core::mem::take(&mut current),
                conflict_before: conflict_before.take(),
            });
            current_access = AccessMask::none();
            conflict_before = Some(next_conflict);
        }
        current.push(index);
        current_access = current_access.union(system.access);
    }

    if !current.is_empty() {
        batches.push(PlannedBatch {
            indices: current,
            conflict_before,
        });
    }
    batches
}

#[inline]
fn elapsed_ns(started: Instant) -> u64 {
    started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
}

#[inline]
fn parallel_efficiency(worker_cpu_ns: u64, worker_wall_ns: u64, width: usize) -> f32 {
    if worker_wall_ns == 0 || width == 0 {
        return 0.0;
    }
    (worker_cpu_ns as f64 / (worker_wall_ns as f64 * width as f64)).clamp(0.0, 1.0) as f32
}

fn run_owner_system(
    world: &mut World,
    stage: SimStage,
    system: &SystemEntry,
    frame: SimFrame,
) -> u64 {
    let mut cb = CommandBuffer::new();
    (system.f)(world, frame, &mut cb);
    #[cfg(debug_assertions)]
    validate_commands(stage, system.name, &cb);
    let commit_started = Instant::now();
    if !cb.is_empty() {
        cb.apply_all(world);
    }
    elapsed_ns(commit_started)
}

fn run_stage_parallel(
    world: &mut World,
    stage: SimStage,
    systems: &[SystemEntry],
    frame: SimFrame,
    telemetry: Option<&SimulationJobTelemetry<'_>>,
    executor: &dyn SimSystemBatchExecutor,
) -> Vec<SimBatchDiagnostics> {
    let plans = plan_conflict_free_batches(systems);
    let batch_count = plans.len();
    let mut diagnostics = Vec::with_capacity(batch_count);

    for (batch_index, plan) in plans.into_iter().enumerate() {
        let indices = plan.indices;
        let conflict_before = plan.conflict_before;
        let conflict_detail = conflict_before.as_ref().map(|conflict| {
            format!(
                " conflict incoming='{}' blocked_by={:?} domains={:?} ww=0x{:x} wr=0x{:x} rw=0x{:x}",
                conflict.incoming_system,
                conflict.conflicting_systems,
                conflict.named_domains,
                conflict.mask.write_write,
                conflict.mask.write_read,
                conflict.mask.read_write,
            )
        }).unwrap_or_default();

        // A singleton cannot make parallel progress. Execute it directly and keep
        // the worker pool available for genuinely parallel batches.
        if indices.len() == 1 {
            let system = &systems[indices[0]];
            let batch = SimulationJobBatch::new(
                stage,
                frame,
                batch_index,
                batch_count,
                1,
                "world-owner-apply-stage",
            );
            if let Some(telemetry) = telemetry {
                telemetry.publish_batch(
                    &batch,
                    EngineTaskPhase::Running,
                    "Simulation singleton running",
                    format!(
                        "System '{}' is serialized by AccessMask boundaries.{}",
                        system.name, conflict_detail
                    ),
                    None,
                );
            }

            let owner_time_ns = run_owner_system(world, stage, system, frame);
            let diagnostic = SimBatchDiagnostics {
                frame,
                stage,
                batch_index,
                batch_width: 1,
                conflict_before,
                worker_wall_time_ns: 0,
                worker_cpu_time_ns: 0,
                owner_commit_time_ns: owner_time_ns,
                parallel_efficiency_01: 0.0,
            };
            if let Some(telemetry) = telemetry {
                telemetry.publish_batch(
                    &batch,
                    EngineTaskPhase::Completed,
                    "Simulation singleton committed",
                    format!(
                        "batch_width=1 owner_commit_ns={} worker_wall_ns=0 worker_cpu_ns=0 parallel_efficiency=0.000{}",
                        owner_time_ns, conflict_detail
                    ),
                    Some(1.0),
                );
            }
            diagnostics.push(diagnostic);
            continue;
        }

        let batch = SimulationJobBatch::new(
            stage,
            frame,
            batch_index,
            batch_count,
            indices.len(),
            "engine.threading",
        );
        if let Some(telemetry) = telemetry {
            telemetry.publish_batch(
                &batch,
                EngineTaskPhase::Scheduled,
                "Simulation parallel batch scheduled",
                format!(
                    "AccessMask admitted batch_width={} independent systems.{}",
                    indices.len(),
                    conflict_detail
                ),
                Some(0.0),
            );
        }

        // `World` is Send + Sync. Move ownership into Arc temporarily so worker
        // closures can satisfy the engine.threading 'static boundary without raw
        // pointers or scoped/unsafe lifetime extension.
        let owned_world = core::mem::take(world);
        let shared_world = Arc::new(owned_world);
        let jobs = indices
            .iter()
            .map(|&system_index| {
                let system = systems[system_index];
                SimSystemJob {
                    system_index,
                    order: system.order,
                    seq: system.seq,
                    name: system.name,
                    access: system.access,
                    function: system.f,
                }
            })
            .collect::<Vec<_>>();

        let mut result = executor.run_system_batch(&batch, Arc::clone(&shared_world), frame, jobs);

        *world = match Arc::try_unwrap(shared_world) {
            Ok(world) => world,
            Err(_) => panic!(
                "sim: parallel executor retained World after batch '{}' returned",
                batch.task_id
            ),
        };

        result
            .commands
            .sort_unstable_by_key(|commands| commands.system_index);
        assert_eq!(
            result.commands.len(),
            indices.len(),
            "sim: executor returned incomplete command batch for '{}'",
            batch.task_id
        );

        let commit_started = Instant::now();
        for (expected_index, command_batch) in
            indices.iter().copied().zip(result.commands.into_iter())
        {
            assert_eq!(
                command_batch.system_index, expected_index,
                "sim: executor returned duplicate/out-of-order system result for '{}'",
                batch.task_id
            );
            #[cfg(debug_assertions)]
            validate_commands(stage, command_batch.system_name, &command_batch.commands);
            if !command_batch.commands.is_empty() {
                command_batch.commands.apply_all(world);
            }
        }
        let owner_commit_time_ns = elapsed_ns(commit_started);
        let efficiency = parallel_efficiency(
            result.worker_cpu_time_ns,
            result.worker_wall_time_ns,
            indices.len(),
        );

        if let Some(telemetry) = telemetry {
            telemetry.publish_batch(
                &batch,
                EngineTaskPhase::Completed,
                "Simulation parallel batch committed",
                format!(
                    "batch_width={} worker_wall_ns={} worker_cpu_ns={} owner_commit_ns={} parallel_efficiency={:.3}{}",
                    indices.len(),
                    result.worker_wall_time_ns,
                    result.worker_cpu_time_ns,
                    owner_commit_time_ns,
                    efficiency,
                    conflict_detail,
                ),
                Some(1.0),
            );
        }
        diagnostics.push(SimBatchDiagnostics {
            frame,
            stage,
            batch_index,
            batch_width: indices.len(),
            conflict_before,
            worker_wall_time_ns: result.worker_wall_time_ns,
            worker_cpu_time_ns: result.worker_cpu_time_ns,
            owner_commit_time_ns,
            parallel_efficiency_01: efficiency,
        });
    }

    diagnostics
}

#[inline]
fn run_stage_single_thread(
    world: &mut World,
    stage: SimStage,
    systems: &[SystemEntry],
    frame: SimFrame,
    telemetry: Option<&SimulationJobTelemetry<'_>>,
    executor: Option<&dyn SimReadBatchExecutor>,
) {
    let batch = (telemetry.is_some() || executor.is_some()).then(|| {
        SimulationJobBatch::new(
            stage,
            frame,
            0,
            1,
            systems.len(),
            if executor.is_some() {
                "engine.threading"
            } else {
                "world-owner-apply-stage"
            },
        )
    });

    if let (Some(telemetry), Some(batch)) = (telemetry, batch.as_ref()) {
        telemetry.publish_batch(
            batch,
            EngineTaskPhase::Scheduled,
            "Simulation batch scheduled",
            format!(
                "World-owner batch dependency_group='{}' systems={} entities={} storages={} resources={}; read snapshot allocation is skipped unless a real executor consumes it.",
                batch.event_dependency_group(),
                systems.len(),
                world.entity_count(),
                world.storage_count(),
                world.resource_count(),
            ),
            Some(0.0),
        );
    }

    if let (Some(executor), Some(batch)) = (executor, batch.as_ref()) {
        let snapshot = SimReadSnapshot::capture(world, frame, stage, systems);
        let report = executor.run_read_batch(batch, snapshot);
        if let Some(telemetry) = telemetry {
            telemetry.publish_batch(batch, EngineTaskPhase::Completed, "Simulation read snapshot processed", format!("Simulation read boundary processed dependency_group='{}' systems={} worker_safe={} executor='{}'; apply stage remains world-owner.", report.dependency_group, report.system_count, report.worker_safe, batch.executor), Some(0.35));
        }
    }

    if let (Some(telemetry), Some(batch)) = (telemetry, batch.as_ref()) {
        telemetry.publish_batch(batch, EngineTaskPhase::Running, "Simulation apply-stage running", "World-owner simulation systems are executing; generated command buffers are applied on the owner thread.", None);
    }
    for s in systems {
        #[cfg(debug_assertions)]
        {
            // Keep metadata alive for debugging/profiling builds.
            let _ = (s.name, s.access);
        }
        let mut cb = CommandBuffer::new();
        (s.f)(world, frame, &mut cb);
        #[cfg(debug_assertions)]
        validate_commands(stage, s.name, &cb);
        if !cb.is_empty() {
            cb.apply_all(world);
        }
    }
    if let (Some(telemetry), Some(batch)) = (telemetry, batch.as_ref()) {
        telemetry.publish_batch(
            batch,
            EngineTaskPhase::Completed,
            "Simulation command batch applied",
            "SimCommandBatch apply-stage completed on the world owner thread.",
            Some(1.0),
        );
    }
}

// Parallel simulation is host-owned: conflict-free batches run through the
// `SimSystemBatchExecutor` boundary, whose production implementation routes every
// worker job through `engine.threading`. World mutation remains owner-thread only.

#[cfg(debug_assertions)]
fn validate_commands(stage: SimStage, system: &'static str, cb: &CommandBuffer) {
    use crate::commands::CommandTag;
    use core::any::TypeId;
    use newengine_transform_api::Transform;

    let tid = TypeId::of::<Transform>();

    for c in cb.iter() {
        match (stage, c.tag()) {
            (SimStage::Controllers, CommandTag::IntentQueueAppend) => {}
            (SimStage::Controllers, other) => {
                panic!(
                    "sim: forbidden direct world mutation in stage={:?} system='{}' (cmd={:?}). Controllers must emit IntentBuffer and enqueue it; only ApplyIntents/Physics may commit world writes.",
                    stage,
                    system,
                    other,
                );
            }
            (_, CommandTag::Insert { type_id, type_name }) if type_id == tid => {
                panic!(
                    "sim: forbidden direct Transform insert in stage={:?} system='{}' (cmd type={}). Use TransformCommandBufferExt::* helpers to emit deterministic intents instead.",
                    stage,
                    system,
                    type_name,
                );
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct AppendLog(&'static str);

    impl crate::Command for AppendLog {
        fn apply(self: Box<Self>, world: &mut World) {
            world
                .resource_mut::<Vec<&'static str>>()
                .expect("commit log resource missing")
                .push(self.0);
        }
    }

    fn log_a(_world: &World, _frame: SimFrame, commands: &mut CommandBuffer) {
        commands.push(Box::new(AppendLog("a")));
    }

    fn log_b(_world: &World, _frame: SimFrame, commands: &mut CommandBuffer) {
        commands.push(Box::new(AppendLog("b")));
    }

    struct ReverseResultExecutor;

    impl SimSystemBatchExecutor for ReverseResultExecutor {
        fn run_system_batch(
            &self,
            _batch: &SimulationJobBatch,
            world: Arc<World>,
            frame: SimFrame,
            systems: Vec<SimSystemJob>,
        ) -> SimSystemBatchResult {
            let mut results = systems
                .into_iter()
                .map(|system| {
                    let mut commands = CommandBuffer::new();
                    (system.function)(world.as_ref(), frame, &mut commands);
                    SimSystemCommandBatch::new(system.system_index, system.name, commands)
                })
                .collect::<Vec<_>>();
            results.reverse();
            SimSystemBatchResult::new(results, 100, 180)
        }
    }

    #[test]
    fn parallel_results_commit_in_stable_system_order_even_if_workers_finish_reversed() {
        let mut schedule = SimSchedule::new();
        schedule.add_system(SimStage::Derived, 10, "log_a", AccessMask::write(0), log_a);
        schedule.add_system(SimStage::Derived, 20, "log_b", AccessMask::write(1), log_b);

        let mut world = World::new();
        world.insert_resource(Vec::<&'static str>::new());
        let diagnostics = schedule.run_stage_with_parallel_executor(
            &mut world,
            SimStage::Derived,
            SimFrame::new(0.016, 9),
            None,
            &ReverseResultExecutor,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].batch_width, 2);
        assert_eq!(diagnostics[0].worker_wall_time_ns, 100);
        assert_eq!(diagnostics[0].worker_cpu_time_ns, 180);
        assert!((diagnostics[0].parallel_efficiency_01 - 0.9).abs() < 0.001);
        assert_eq!(
            world
                .resource::<Vec<&'static str>>()
                .expect("commit log resource missing"),
            &vec!["a", "b"]
        );
    }

    #[test]
    fn default_controller_stage_forms_access_mask_parallel_then_serial_batches() {
        let mut schedule = default_schedule();
        schedule.sort_if_needed();
        let systems = &schedule.stages[SimStage::Controllers.as_usize()];
        let batches = plan_conflict_free_batches(systems);

        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].indices, vec![0, 1]);
        assert_eq!(batches[1].indices, vec![2]);
        let conflict = batches[1]
            .conflict_before
            .as_ref()
            .expect("camera conflict diagnostic missing");
        assert_eq!(conflict.incoming_system, "camera_follow");
        assert_eq!(conflict.conflicting_systems, vec!["orbit_camera"]);
        assert_eq!(
            conflict.mask.write_write,
            AccessDomain::CameraControl.mask()
        );
        assert_eq!(conflict.mask.write_read, 0);
        assert_eq!(conflict.mask.read_write, 0);
        assert!(conflict
            .named_domains
            .contains(&"camera-control".to_owned()));
        assert!(!systems[0].access.conflicts(systems[1].access));
        assert!(systems[1].access.conflicts(systems[2].access));
    }
}

/// A production-lean default schedule.
///
/// You can extend it with gameplay systems without forking the engine.
#[inline]
pub fn default_schedule() -> SimSchedule {
    let mut s = SimSchedule::new();

    // Controllers emit intents only.
    s.add_system(
        SimStage::Controllers,
        10,
        "character_motor",
        AccessMask::write_domain(AccessDomain::CharacterControl)
            .union(AccessMask::read_domain(AccessDomain::CharacterInput)),
        systems::sys_character_motor,
    );
    s.add_system(
        SimStage::Controllers,
        20,
        "orbit_camera",
        AccessMask::write_domain(AccessDomain::CameraControl)
            .union(AccessMask::read_domain(AccessDomain::CameraInput))
            .union(AccessMask::read_domain(AccessDomain::CameraRig)),
        systems::sys_orbit_camera,
    );
    s.add_system(
        SimStage::Controllers,
        25,
        "camera_follow",
        AccessMask::write_domain(AccessDomain::CameraControl)
            .union(AccessMask::read_domain(AccessDomain::CameraRig))
            .union(AccessMask::read_domain(AccessDomain::FollowTarget))
            .union(AccessMask::read_domain(AccessDomain::Transform)),
        systems::sys_camera_follow,
    );

    // Single ordered apply stage.
    s.add_system(
        SimStage::ApplyIntents,
        10,
        "apply_controller_intents",
        AccessMask::write_domain(AccessDomain::CharacterControl)
            .union(AccessMask::write_domain(AccessDomain::CameraControl))
            .union(AccessMask::write_domain(AccessDomain::ControllerIntents)),
        systems::sys_apply_controller_intents,
    );
    s.add_system(
        SimStage::ApplyIntents,
        20,
        "camera_rig_to_transform",
        AccessMask::read_domain(AccessDomain::CameraRig)
            .union(AccessMask::write_domain(AccessDomain::Transform)),
        systems::sys_camera_rig_to_transform,
    );

    // Physics.
    s.add_system(
        SimStage::Physics,
        10,
        "integrate_velocities",
        AccessMask::read_domain(AccessDomain::Velocity)
            .union(AccessMask::write_domain(AccessDomain::Transform))
            .union(AccessMask::write_domain(AccessDomain::PhysicsState)),
        systems::sys_integrate_velocities,
    );

    s
}
