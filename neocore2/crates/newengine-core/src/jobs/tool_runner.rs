use super::{JobControl, JobLane, JobPriority, JobRequest, JobSystemHandle, JobTicket};

/// Tool-facing job runner bound to the engine-wide job authority.
///
/// Authoring tools can execute blocking work through this adapter instead of
/// inventing their own local thread pools. The executor remains the core job
/// system, but the semantic identity is a tool-owned `JobId` visible through the
/// shared `engine.jobs` event stream.
#[derive(Clone)]
pub struct ToolJobRunner {
    jobs: JobSystemHandle,
    owner: &'static str,
    category: &'static str,
    lane: JobLane,
}

impl ToolJobRunner {
    #[inline]
    pub fn new(jobs: JobSystemHandle, owner: &'static str) -> Self {
        Self {
            jobs,
            owner,
            category: "tool-work",
            lane: JobLane::Background,
        }
    }

    #[inline]
    pub fn with_category(mut self, category: &'static str) -> Self {
        self.category = category;
        self
    }

    #[inline]
    pub fn with_lane(mut self, lane: JobLane) -> Self {
        self.lane = lane;
        self
    }

    pub fn submit<F>(&self, label: &'static str, f: F) -> JobTicket
    where
        F: FnOnce() + Send + 'static,
    {
        let task_id = format!("tool.{}.{}", self.owner, label);
        self.jobs.submit_request(
            JobRequest::new(label)
                .with_lane(self.lane)
                .with_priority(JobPriority::Normal)
                .with_source("engine.jobs.tool-runner")
                .with_owner(self.owner)
                .with_category(self.category)
                .with_task_id(task_id)
                .cancellable(true)
                .pausable(false),
            f,
        )
    }

    pub fn submit_controlled<F>(&self, label: &'static str, f: F) -> JobTicket
    where
        F: FnOnce(JobControl) + Send + 'static,
    {
        let task_id = format!("tool.{}.{}", self.owner, label);
        self.jobs.submit_controlled(
            JobRequest::new(label)
                .with_lane(self.lane)
                .with_priority(JobPriority::Normal)
                .with_source("engine.jobs.tool-runner")
                .with_owner(self.owner)
                .with_category(self.category)
                .with_task_id(task_id)
                .cancellable(true)
                .pausable(false),
            f,
        )
    }

    #[inline]
    pub const fn owner(&self) -> &'static str {
        self.owner
    }

    #[inline]
    pub const fn category(&self) -> &'static str {
        self.category
    }
}
