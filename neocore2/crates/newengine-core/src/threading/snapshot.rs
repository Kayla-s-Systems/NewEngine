use crate::task_core::{TaskLane, ThreadPoolCoreSnapshot, JOB_LANE_COUNT};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ThreadPoolSnapshot {
    pub active_threads: usize,
    pub pending_jobs: usize,
    pub running_jobs: usize,
    pub paused_jobs: usize,
    pub submitted_jobs: u64,
    pub completed_jobs: u64,
    pub cancelled_jobs: u64,
    pub panicked_jobs: u64,
    pub total_cpu_time_ns: u64,
    pub frame_cpu_budget_ns: u64,
    pub frame_cpu_used_ns: u64,
    pub frame_over_budget: bool,
    pub overbudget_frames: u64,
    pub budget_deferred_polls: u64,
    pub lanes: [ThreadPoolLaneSnapshot; JOB_LANE_COUNT],
}

impl ThreadPoolSnapshot {
    #[inline]
    pub fn pending_for_lane(&self, lane: TaskLane) -> usize {
        self.lanes[lane.index()].pending_jobs
    }

    #[inline]
    pub fn running_for_lane(&self, lane: TaskLane) -> usize {
        self.lanes[lane.index()].running_jobs
    }

    #[inline]
    pub fn completed_for_lane(&self, lane: TaskLane) -> u64 {
        self.lanes[lane.index()].completed_jobs
    }

    #[inline]
    pub fn cpu_time_ns_for_lane(&self, lane: TaskLane) -> u64 {
        self.lanes[lane.index()].cpu_time_ns
    }

    pub(crate) fn from_core_snapshot(snapshot: ThreadPoolCoreSnapshot) -> Self {
        let lanes = std::array::from_fn(|index| ThreadPoolLaneSnapshot {
            pending_jobs: snapshot.pending_by_lane[index],
            running_jobs: snapshot.running_by_lane[index],
            completed_jobs: snapshot.completed_by_lane[index],
            cpu_time_ns: snapshot.cpu_time_ns_by_lane[index],
        });
        Self {
            active_threads: snapshot.worker_threads,
            pending_jobs: snapshot.pending_jobs,
            running_jobs: snapshot.running_jobs,
            paused_jobs: snapshot.paused_jobs,
            submitted_jobs: snapshot.submitted_jobs,
            completed_jobs: snapshot.completed_jobs,
            cancelled_jobs: snapshot.cancelled_jobs,
            panicked_jobs: snapshot.panicked_jobs,
            total_cpu_time_ns: snapshot.total_cpu_time_ns,
            frame_cpu_budget_ns: snapshot.frame_cpu_budget_ns,
            frame_cpu_used_ns: snapshot.frame_cpu_used_ns,
            frame_over_budget: snapshot.frame_cpu_budget_ns > 0
                && snapshot.frame_cpu_used_ns >= snapshot.frame_cpu_budget_ns,
            overbudget_frames: snapshot.overbudget_frames,
            budget_deferred_polls: snapshot.budget_deferred_polls,
            lanes,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ThreadPoolLaneSnapshot {
    pub pending_jobs: usize,
    pub running_jobs: usize,
    pub completed_jobs: u64,
    pub cpu_time_ns: u64,
}
