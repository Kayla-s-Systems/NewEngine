use std::{sync::Arc, time::Instant};

use newengine_ecs::World;
use newengine_task_api::EngineTaskPhase;

use crate::{commands::CommandBuffer, SimFrame};

use super::{
    planning::plan_conflict_free_batches, SimBatchDiagnostics, SimReadBatchExecutor,
    SimReadSnapshot, SimStage, SimSystemBatchExecutor, SimSystemJob, SimulationJobBatch,
    SimulationJobTelemetry, SystemEntry,
};

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
    _stage: SimStage,
    system: &SystemEntry,
    frame: SimFrame,
) -> u64 {
    let mut cb = CommandBuffer::new();
    (system.f)(world, frame, &mut cb);
    #[cfg(debug_assertions)]
    validate_commands(_stage, system.name, &cb);
    let commit_started = Instant::now();
    if !cb.is_empty() {
        cb.apply_all(world);
    }
    elapsed_ns(commit_started)
}

pub(super) fn run_stage_parallel(
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
            indices.iter().copied().zip(result.commands)
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
pub(super) fn run_stage_single_thread(
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
