use super::config::{TaskLane, TaskPriority, JOB_LANE_COUNT};
use newengine_loading_api::EngineTaskPhase;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ThreadPoolCoreSnapshot {
    pub worker_threads: usize,
    pub pending_jobs: usize,
    pub running_jobs: usize,
    pub paused_jobs: usize,
    pub submitted_jobs: u64,
    pub completed_jobs: u64,
    pub cancelled_jobs: u64,
    pub panicked_jobs: u64,
    pub pending_by_lane: [usize; JOB_LANE_COUNT],
    pub running_by_lane: [usize; JOB_LANE_COUNT],
    pub completed_by_lane: [u64; JOB_LANE_COUNT],
    pub total_cpu_time_ns: u64,
    pub frame_cpu_budget_ns: u64,
    pub frame_cpu_used_ns: u64,
    pub overbudget_frames: u64,
    pub budget_deferred_polls: u64,
    pub cpu_time_ns_by_lane: [u64; JOB_LANE_COUNT],
}

#[derive(Clone, Debug)]
pub struct CoreTaskRuntimeStatus {
    pub task_id: String,
    pub label: &'static str,
    pub lane: TaskLane,
    pub priority: TaskPriority,
    pub frame_id: Option<u64>,
    pub dependency_group: Option<String>,
    pub task_domain: &'static str,
    pub task_pass: &'static str,
    pub phase: EngineTaskPhase,
    pub can_pause: bool,
    pub can_cancel: bool,
    pub cancel_requested: bool,
    pub pause_requested: bool,
}
