use std::sync::Arc;

use newengine_ecs::World;
use newengine_task_api::{task_domain, task_pass, EngineTaskEvent, EngineTaskPhase};
use serde::{Deserialize, Serialize};

use crate::{
    access::{AccessConflictMask, AccessMask},
    commands::CommandBuffer,
    SimFrame,
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
    pub(super) fn from_entry(entry: &SystemEntry) -> Self {
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
    pub(super) fn capture(
        world: &World,
        frame: SimFrame,
        stage: SimStage,
        systems: &[SystemEntry],
    ) -> Self {
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
pub(super) struct SystemEntry {
    pub(super) order: i32,
    pub(super) seq: u32,
    pub(super) name: &'static str,
    pub(super) access: AccessMask,
    pub(super) f: SystemFn,
}
