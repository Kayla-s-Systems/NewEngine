#![forbid(unsafe_op_in_unsafe_fn)]

use core::cmp::Ordering;

use newengine_ecs::World;
use newengine_task_api::{task_domain, task_pass, EngineTaskEvent, EngineTaskPhase};
use serde::{Deserialize, Serialize};

use crate::{access::AccessMask, commands::CommandBuffer, systems, SimFrame};

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
/// world-owner capture -> SimReadSnapshot DTO -> engine.threading read-only batches
///                       -> SimCommandBatch -> world-owner apply stage
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

/// Host-owned adapter that maps simulation read batches to `engine.threading`.
///
/// `newengine-sim` deliberately depends only on this trait, not on
/// `newengine-core`, so the simulation crate stays provider/runtime agnostic.
pub trait SimReadBatchExecutor {
    fn run_read_batch(
        &self,
        batch: &SimulationJobBatch,
        snapshot: SimReadSnapshot,
        job: Box<dyn FnOnce(SimReadSnapshot) -> SimReadBatchReport + Send + 'static>,
    ) -> SimReadBatchReport;
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

#[inline]
fn run_stage_single_thread(
    world: &mut World,
    stage: SimStage,
    systems: &[SystemEntry],
    frame: SimFrame,
    telemetry: Option<&SimulationJobTelemetry<'_>>,
    executor: Option<&dyn SimReadBatchExecutor>,
) {
    let snapshot = SimReadSnapshot::capture(world, frame, stage, systems);
    let batch = SimulationJobBatch::new(
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
    );
    if let Some(telemetry) = telemetry {
        telemetry.publish_batch(&batch, EngineTaskPhase::Scheduled, "Simulation read snapshot captured", format!("SimReadSnapshot captured dependency_group='{}' systems={} entities={} storages={} resources={}; DTO is serializable and worker-safe.", snapshot.dependency_group, snapshot.system_count(), snapshot.world.entity_count, snapshot.world.storage_count, snapshot.world.resource_count), Some(0.0));
    }

    if let Some(executor) = executor {
        let report = executor.run_read_batch(
            &batch,
            snapshot.clone(),
            Box::new(|snapshot| SimReadBatchReport::from_snapshot(&snapshot, 0)),
        );
        if let Some(telemetry) = telemetry {
            telemetry.publish_batch(&batch, EngineTaskPhase::Completed, "Simulation read snapshot processed", format!("engine.threading processed SimReadSnapshot dependency_group='{}' systems={} worker_safe={}; apply stage remains world-owner.", report.dependency_group, report.system_count, report.worker_safe), Some(0.35));
        }
    }

    if let Some(telemetry) = telemetry {
        telemetry.publish_batch(&batch, EngineTaskPhase::Running, "Simulation apply-stage running", "World-owner simulation systems are executing from a captured read boundary; generated command buffers are applied on the owner thread.", None);
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
    if let Some(telemetry) = telemetry {
        telemetry.publish_batch(
            &batch,
            EngineTaskPhase::Completed,
            "Simulation command batch applied",
            "SimCommandBatch apply-stage completed on the world owner thread.",
            Some(1.0),
        );
    }
}

// Parallel simulation is intentionally not implemented through `rayon` here.
// When this scheduler grows parallel execution again, each batch must be
// submitted through `engine.threading` so it has a JobId, lane, priority, progress
// events and cooperative cancellation.

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
        AccessMask::write(crate::Subsystem::Gameplay as u32),
        systems::sys_character_motor,
    );
    s.add_system(
        SimStage::Controllers,
        20,
        "orbit_camera",
        AccessMask::write(crate::Subsystem::Camera as u32),
        systems::sys_orbit_camera,
    );
    s.add_system(
        SimStage::Controllers,
        25,
        "camera_follow",
        AccessMask::write(crate::Subsystem::Camera as u32),
        systems::sys_camera_follow,
    );

    // Single ordered apply stage.
    s.add_system(
        SimStage::ApplyIntents,
        10,
        "apply_controller_intents",
        AccessMask::rw(
            0,
            (1u128 << (crate::Subsystem::Gameplay as u32))
                | (1u128 << (crate::Subsystem::Camera as u32)),
        ),
        systems::sys_apply_controller_intents,
    );
    s.add_system(
        SimStage::ApplyIntents,
        20,
        "camera_rig_to_transform",
        AccessMask::write(crate::Subsystem::Camera as u32),
        systems::sys_camera_rig_to_transform,
    );

    // Physics.
    s.add_system(
        SimStage::Physics,
        10,
        "integrate_velocities",
        AccessMask::write(crate::Subsystem::Gameplay as u32),
        systems::sys_integrate_velocities,
    );

    s
}
