use super::config::{JobLane, JobPriority};

#[derive(Clone, Debug)]
pub struct JobRequest {
    pub label: &'static str,
    pub source: &'static str,
    pub owner: &'static str,
    pub category: &'static str,
    pub lane: JobLane,
    pub priority: JobPriority,
    pub task_id: Option<String>,
    pub parent_task_id: Option<String>,
    pub frame_id: Option<u64>,
    pub dependency_group: Option<String>,
    pub job_domain: &'static str,
    pub job_pass: &'static str,
    pub can_pause: bool,
    pub can_cancel: bool,
}

impl JobRequest {
    #[inline]
    pub const fn new(label: &'static str) -> Self {
        Self {
            label,
            source: "newengine-core.job-system",
            owner: "newengine-core",
            category: "cpu-job",
            lane: JobLane::Simulation,
            priority: JobPriority::Normal,
            task_id: None,
            parent_task_id: None,
            frame_id: None,
            dependency_group: None,
            job_domain: "engine.jobs",
            job_pass: "cpu-work",
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
    pub const fn with_source(mut self, source: &'static str) -> Self {
        self.source = source;
        self
    }

    #[inline]
    pub const fn with_owner(mut self, owner: &'static str) -> Self {
        self.owner = owner;
        self
    }

    #[inline]
    pub const fn with_category(mut self, category: &'static str) -> Self {
        self.category = category;
        self
    }

    #[inline]
    pub const fn with_frame_id(mut self, frame_id: u64) -> Self {
        self.frame_id = Some(frame_id);
        self
    }

    #[inline]
    pub fn with_dependency_group(mut self, dependency_group: impl Into<String>) -> Self {
        self.dependency_group = Some(dependency_group.into());
        self
    }

    #[inline]
    pub const fn with_job_domain(mut self, job_domain: &'static str) -> Self {
        self.job_domain = job_domain;
        self
    }

    #[inline]
    pub const fn with_job_pass(mut self, job_pass: &'static str) -> Self {
        self.job_pass = job_pass;
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
