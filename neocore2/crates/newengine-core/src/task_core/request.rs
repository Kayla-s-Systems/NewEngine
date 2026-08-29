use super::config::{TaskLane, TaskPriority};

#[derive(Clone, Debug)]
pub struct TaskRequest {
    pub label: &'static str,
    pub source: &'static str,
    pub owner: &'static str,
    pub category: &'static str,
    pub lane: TaskLane,
    pub priority: TaskPriority,
    pub task_id: Option<String>,
    pub parent_task_id: Option<String>,
    pub frame_id: Option<u64>,
    pub dependency_group: Option<String>,
    /// Explicit task prerequisites. The scheduler will not dispatch this task until
    /// all referenced task ids have completed. This is execution ordering, unlike
    /// `dependency_group`, which is diagnostic/trace metadata only.
    pub prerequisite_task_ids: Vec<String>,
    pub task_domain: &'static str,
    pub task_pass: &'static str,
    pub can_pause: bool,
    pub can_cancel: bool,
}

impl TaskRequest {
    #[inline]
    pub const fn new(label: &'static str) -> Self {
        Self {
            label,
            source: "newengine-core.task-core",
            owner: "newengine-core",
            category: "cpu-job",
            lane: TaskLane::Simulation,
            priority: TaskPriority::Normal,
            task_id: None,
            parent_task_id: None,
            frame_id: None,
            dependency_group: None,
            prerequisite_task_ids: Vec::new(),
            task_domain: "engine.threading",
            task_pass: "cpu-work",
            can_pause: false,
            can_cancel: true,
        }
    }

    #[inline]
    pub const fn with_lane(mut self, lane: TaskLane) -> Self {
        self.lane = lane;
        self
    }

    #[inline]
    pub const fn with_priority(mut self, priority: TaskPriority) -> Self {
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

    /// Adds one prerequisite task id. The task remains queued without occupying a
    /// worker until the prerequisite completes.
    #[inline]
    pub fn after_task(mut self, task_id: impl Into<String>) -> Self {
        let task_id = task_id.into();
        if !task_id.trim().is_empty() && !self.prerequisite_task_ids.contains(&task_id) {
            self.prerequisite_task_ids.push(task_id);
        }
        self
    }

    /// Adds multiple prerequisite task ids, preserving deterministic insertion order.
    #[inline]
    pub fn after_tasks<I, S>(mut self, task_ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for task_id in task_ids {
            let task_id = task_id.into();
            if !task_id.trim().is_empty() && !self.prerequisite_task_ids.contains(&task_id) {
                self.prerequisite_task_ids.push(task_id);
            }
        }
        self
    }

    #[inline]
    pub const fn with_task_domain(mut self, task_domain: &'static str) -> Self {
        self.task_domain = task_domain;
        self
    }

    #[inline]
    pub const fn with_task_pass(mut self, task_pass: &'static str) -> Self {
        self.task_pass = task_pass;
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
