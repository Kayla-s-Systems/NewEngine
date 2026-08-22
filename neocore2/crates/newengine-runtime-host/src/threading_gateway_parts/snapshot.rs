use newengine_core::{TaskLane, ThreadPoolHandle};
use newengine_task_api::{TaskQueueLaneSnapshotJsonV1, TaskQueueSnapshotJsonV1};

pub(crate) fn snapshot_from_pool(thread_pool: &ThreadPoolHandle) -> TaskQueueSnapshotJsonV1 {
    let snapshot = thread_pool.snapshot();
    let mut lanes = std::collections::BTreeMap::new();
    for lane in [
        TaskLane::Simulation,
        TaskLane::RenderPrep,
        TaskLane::Streaming,
        TaskLane::AssetIo,
        TaskLane::Plugin,
        TaskLane::Background,
    ] {
        lanes.insert(
            lane.as_str().to_owned(),
            TaskQueueLaneSnapshotJsonV1 {
                pending_threading: snapshot.pending_for_lane(lane),
                running_threading: snapshot.running_for_lane(lane),
                completed_threading: snapshot.completed_for_lane(lane),
                cpu_time_ns: snapshot.cpu_time_ns_for_lane(lane),
            },
        );
    }
    TaskQueueSnapshotJsonV1 {
        worker_threads: snapshot.active_threads,
        pending_threading: snapshot.pending_jobs,
        running_threading: snapshot.running_jobs,
        paused_threading: snapshot.paused_jobs,
        submitted_threading: snapshot.submitted_jobs,
        completed_threading: snapshot.completed_jobs,
        cancelled_threading: snapshot.cancelled_jobs,
        panicked_threading: snapshot.panicked_jobs,
        total_cpu_time_ns: snapshot.total_cpu_time_ns,
        frame_cpu_budget_ns: snapshot.frame_cpu_budget_ns,
        frame_cpu_used_ns: snapshot.frame_cpu_used_ns,
        frame_over_budget: snapshot.frame_over_budget,
        overbudget_frames: snapshot.overbudget_frames,
        budget_deferred_polls: snapshot.budget_deferred_polls,
        lanes,
    }
}
