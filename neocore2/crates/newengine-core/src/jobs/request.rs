use super::config::{JobLane, JobPriority};

#[derive(Clone, Debug)]
pub struct JobRequest {
    pub label: &'static str,
    pub lane: JobLane,
    pub priority: JobPriority,
    pub task_id: Option<String>,
    pub parent_task_id: Option<String>,
    pub can_pause: bool,
    pub can_cancel: bool,
}

impl JobRequest {
    #[inline]
    pub const fn new(label: &'static str) -> Self {
        Self {
            label,
            lane: JobLane::Simulation,
            priority: JobPriority::Normal,
            task_id: None,
            parent_task_id: None,
            can_pause: false,
            can_cancel: true,
        }
    }

    #[inline]
    pub const fn with_lane(mut self, lane: JobLane) -> Self {
        self.lane = lane;
        self
    }

    #[inline]
    pub const fn with_priority(mut self, priority: JobPriority) -> Self {
        self.priority = priority;
        self
    }

    #[inline]
    pub fn with_task_id(mut self, task_id: impl Into<String>) -> Self {
        self.task_id = Some(task_id.into());
        self
    }

    #[inline]
    pub fn with_parent_task_id(mut self, parent_task_id: impl Into<String>) -> Self {
        self.parent_task_id = Some(parent_task_id.into());
        self
    }

    #[inline]
    pub const fn pausable(mut self, can_pause: bool) -> Self {
        self.can_pause = can_pause;
        self
    }

    #[inline]
    pub const fn cancellable(mut self, can_cancel: bool) -> Self {
        self.can_cancel = can_cancel;
        self
    }
}

