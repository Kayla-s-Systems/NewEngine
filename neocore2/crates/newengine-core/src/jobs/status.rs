use super::config::{JobLane, JobPriority, JOB_LANE_COUNT};
use newengine_loading_api::EngineTaskPhase;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct JobSystemSnapshot {
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
}

impl JobSystemSnapshot {
    #[inline]
    pub fn pending_for_lane(&self, lane: JobLane) -> usize {
        self.pending_by_lane[lane.index()]
    }

    #[inline]
    pub fn running_for_lane(&self, lane: JobLane) -> usize {
        self.running_by_lane[lane.index()]
    }

    #[inline]
    pub fn completed_for_lane(&self, lane: JobLane) -> u64 {
        self.completed_by_lane[lane.index()]
    }
}

#[derive(Clone, Debug)]
pub struct JobTaskStatus {
    pub task_id: String,
    pub label: &'static str,
    pub lane: JobLane,
    pub priority: JobPriority,
    pub frame_id: Option<u64>,
    pub dependency_group: Option<String>,
    pub job_domain: &'static str,
    pub job_pass: &'static str,
    pub phase: EngineTaskPhase,
    pub can_pause: bool,
    pub can_cancel: bool,
    pub cancel_requested: bool,
    pub pause_requested: bool,
}
