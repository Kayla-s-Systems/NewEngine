#![forbid(unsafe_op_in_unsafe_fn)]

use core::cmp::Ordering;

use newengine_ecs::World;
use newengine_jobs_api::{EngineTaskEvent, EngineTaskPhase};

use crate::{access::AccessMask, commands::CommandBuffer, systems, SimFrame};

/// Simulation stages.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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
            Self::Input => "input",
            Self::Controllers => "controllers",
            Self::ApplyIntents => "apply-intents",
            Self::Physics => "physics",
            Self::Derived => "derived",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimulationJobBatch {
    pub task_id: String,
    pub stage: SimStage,
    pub fixed_tick: u64,
    pub batch_index: usize,
    pub batch_count: usize,
    pub system_count: usize,
    pub executor: &'static str,
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
            executor,
        }
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
            format!("simulation:{}:batch:{}", self.stage.as_str(), self.batch_index),
            "simulation",
            phase,
            status,
            detail,
        )
        .with_controls(false, false);
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
        self.run_stage_with_telemetry(world, stage, frame, None);
    }

    pub fn run_stage_with_telemetry(
        &mut self,
        world: &mut World,
        stage: SimStage,
        frame: SimFrame,
        telemetry: Option<&SimulationJobTelemetry<'_>>,
    ) {
        self.sort_if_needed();

        let systems = &self.stages[stage.as_usize()];
        if systems.is_empty() {
            return;
        }

        #[cfg(feature = "parallel")]
        {
            run_stage_parallel(world, stage, systems, frame, telemetry);
            return;
        }

        #[cfg(not(feature = "parallel"))]
        {
            run_stage_single_thread(world, stage, systems, frame, telemetry);
        }
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
) {
    let batch = SimulationJobBatch::new(stage, frame, 0, 1, systems.len(), "single-thread");
    if let Some(telemetry) = telemetry {
        telemetry.publish_batch(&batch, EngineTaskPhase::Scheduled, "Simulation batch scheduled", "Single-thread simulation stage entered the engine.jobs telemetry bridge.", Some(0.0));
        telemetry.publish_batch(&batch, EngineTaskPhase::Running, "Simulation batch running", "Single-thread simulation systems are executing.", None);
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
        telemetry.publish_batch(&batch, EngineTaskPhase::Completed, "Simulation batch completed", "Single-thread simulation stage completed and command buffers were applied.", Some(1.0));
    }
}

#[cfg(feature = "parallel")]
fn run_stage_parallel(
    world: &mut World,
    stage: SimStage,
    systems: &[SystemEntry],
    frame: SimFrame,
    telemetry: Option<&SimulationJobTelemetry<'_>>,
) {
    use std::sync::mpsc;

    let batches = build_batches(systems);

    let batch_count = batches.len();
    for (batch_index, batch) in batches.into_iter().enumerate() {
        let telemetry_batch = SimulationJobBatch::new(
            stage,
            frame,
            batch_index,
            batch_count,
            batch.len(),
            "rayon-scope",
        );
        if let Some(telemetry) = telemetry {
            telemetry.publish_batch(&telemetry_batch, EngineTaskPhase::Scheduled, "Simulation batch scheduled", "Rayon simulation batch is visible through engine.jobs telemetry.", Some(0.0));
            telemetry.publish_batch(&telemetry_batch, EngineTaskPhase::Running, "Simulation batch running", "Rayon simulation systems are executing as provider-owned internal parallelism.", None);
        }

        // World snapshot for this batch.
        // Systems are required to only read from `world` and write to their command buffers.
        let wref: &World = world;

        let (tx, rx) = mpsc::channel::<((i32, u32), &'static str, CommandBuffer)>();

        // no-hidden-thread-scan: allowed simulation internal parallelism; SimulationJobBatch publishes engine.jobs-compatible telemetry before this executor.
        rayon::scope(|scope| {
            for sys in batch {
                let tx = tx.clone();
                scope.spawn(move |_| {
                    let mut cb = CommandBuffer::new();
                    (sys.f)(wref, frame, &mut cb);
                    let _ = tx.send(((sys.order, sys.seq), sys.name, cb));
                });
            }
        });

        drop(tx);

        let mut collected: Vec<((i32, u32), &'static str, CommandBuffer)> = rx.into_iter().collect();
        collected.sort_by(|a, b| a.0.0.cmp(&b.0.0).then(a.0.1.cmp(&b.0.1)));

        for (_key, name, cb) in collected {
            #[cfg(debug_assertions)]
            validate_commands(stage, name, &cb);
            if !cb.is_empty() {
                cb.apply_all(world);
            }
        }
        if let Some(telemetry) = telemetry {
            telemetry.publish_batch(&telemetry_batch, EngineTaskPhase::Completed, "Simulation batch completed", "Rayon simulation batch completed and command buffers were applied in deterministic order.", Some(1.0));
        }
    }
}

#[cfg(feature = "parallel")]
fn build_batches<'a>(systems: &'a [SystemEntry]) -> Vec<Vec<&'a SystemEntry>> {
    let mut batches: Vec<Vec<&'a SystemEntry>> = Vec::new();
    let mut masks: Vec<AccessMask> = Vec::new();

    'sys: for sys in systems {
        for (i, m) in masks.iter_mut().enumerate() {
            if !m.conflicts(sys.access) {
                *m = m.union(sys.access);
                batches[i].push(sys);
                continue 'sys;
            }
        }

        batches.push(vec![sys]);
        masks.push(sys.access);
    }

    batches
}

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
        AccessMask::rw(0, (1u128 << (crate::Subsystem::Gameplay as u32)) | (1u128 << (crate::Subsystem::Camera as u32))),
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
