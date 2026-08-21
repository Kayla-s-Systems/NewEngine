use super::super::status::ThreadPoolCoreSnapshot;
use super::execution::{execution_depth_for_lane, in_worker_task_context, TaskExecutionScope};
use super::hierarchy::TaskBodyOutcome;
use super::*;
use newengine_loading_api::EngineTaskPhase;

impl TaskCoreShared {
    #[inline]
    fn queue_index(lane: TaskLane, priority: TaskPriority) -> usize {
        priority.index() * JOB_LANE_COUNT + lane.index()
    }

    #[inline]
    fn queue_bit(queue_index: usize) -> u64 {
        debug_assert!(queue_index < u64::BITS as usize);
        1u64 << queue_index
    }

    pub(super) fn enqueue_ready(&self, task: QueuedTask) {
        let queue_index = Self::queue_index(task.request.lane, task.request.priority);
        let mut queue = self.queues[queue_index].lock();
        queue.push_back(task);
        // Publish readiness while the queue lock is still held. A consumer that
        // drains the queue clears the bit under the same lock, preventing a lost
        // ready transition between push and mask update.
        self.ready_queue_mask
            .fetch_or(Self::queue_bit(queue_index), Ordering::Release);
    }

    #[inline]
    fn try_pop_ready_queue(&self, queue_index: usize) -> Option<QueuedTask> {
        let bit = Self::queue_bit(queue_index);
        if self.ready_queue_mask.load(Ordering::Acquire) & bit == 0 {
            return None;
        }

        let mut queue = self.queues[queue_index].lock();
        let job = queue.pop_front();
        if queue.is_empty() {
            // `enqueue_ready` sets this bit while holding the same queue lock, so
            // clearing it here cannot erase a concurrent producer's transition.
            self.ready_queue_mask.fetch_and(!bit, Ordering::AcqRel);
        }
        job
    }

    pub(in crate::task_core) fn register_task_hierarchy(
        &self,
        task_id: &str,
        parent_task_id: Option<&str>,
    ) {
        self.task_hierarchy.lock().register(task_id, parent_task_id);
    }

    #[inline]
    pub(in crate::task_core) fn run_ready_task(&self, job: QueuedTask) {
        let _execution_scope = TaskExecutionScope::enter(job.request.lane);
        job.run(self);
    }

    pub(in crate::task_core) fn wait_for_completion(&self, completion: &TaskCompletion) {
        if completion.is_complete() {
            return;
        }

        if !in_worker_task_context() {
            completion.wait();
            return;
        }

        while !completion.is_complete() {
            if let Some(job) = self.pop_next_helping() {
                self.run_ready_task(job);
                continue;
            }

            // The target may be running on another worker or blocked on a graph
            // prerequisite. A bounded wait avoids spinning while still letting
            // newly-ready work be helped promptly.
            completion.wait_timeout(Duration::from_millis(1));
        }
    }

    pub(super) fn release_lane(&self, lane: TaskLane) {
        self.running_by_lane[lane.index()].fetch_sub(1, Ordering::AcqRel);
        self.sleep_wake.notify_one();
    }

    pub(super) fn finish_task_body(&self, task_id: &str, outcome: TaskBodyOutcome) {
        let (waiting_for_children, finalized) =
            self.task_hierarchy.lock().finish_body(task_id, outcome);

        if waiting_for_children {
            if let Some(control) = self.tasks.lock().get(task_id).cloned() {
                control.publish(
                    EngineTaskPhase::Blocked,
                    "Task waiting for nested work",
                    "Task body finished; completion is deferred until nested child tasks finish.",
                    None,
                );
            }
        }

        for (finalized_task_id, finalized_outcome) in finalized {
            self.finalize_task(&finalized_task_id, finalized_outcome);
        }
    }

    fn finalize_task(&self, task_id: &str, outcome: TaskBodyOutcome) {
        let control = self.tasks.lock().get(task_id).cloned();
        if let Some(control) = control.as_ref() {
            let lane_index = control.status().lane.index();
            if outcome.counts_completed() {
                self.completed.fetch_add(1, Ordering::AcqRel);
                self.completed_by_lane[lane_index].fetch_add(1, Ordering::AcqRel);
            }
            if outcome.counts_cancelled() {
                self.cancelled.fetch_add(1, Ordering::AcqRel);
            }
            if outcome.counts_panicked() {
                self.panicked.fetch_add(1, Ordering::AcqRel);
            }

            control.publish(
                outcome.phase(),
                outcome.status(),
                outcome.detail(),
                Some(1.0),
            );
        }

        if let Some(completion) = self.completions.lock().get(task_id).cloned() {
            completion.complete();
        }
        self.release_dependents(task_id);
        self.sleep_wake.notify_all();
    }

    pub(in crate::task_core) fn submit(&self, job: QueuedTask) {
        if self.shutdown.load(Ordering::Acquire) {
            let task_id = job.control.task_id().to_owned();
            job.control.publish(
                EngineTaskPhase::Cancelled,
                "Task rejected",
                "Task core is shutting down; task was not queued.",
                Some(1.0),
            );
            self.finish_task_body(&task_id, TaskBodyOutcome::CancelledBeforeExecution);
            return;
        }

        let lane_index = job.request.lane.index();

        self.submitted.fetch_add(1, Ordering::AcqRel);
        self.pending.fetch_add(1, Ordering::Release);
        self.pending_by_lane[lane_index].fetch_add(1, Ordering::Release);
        job.control.publish(
            EngineTaskPhase::Scheduled,
            "Task scheduled",
            "Task was registered in the engine task graph.",
            Some(0.0),
        );

        if let Some(ready) = self.register_dependencies(job) {
            self.enqueue_ready(ready);
            self.sleep_wake.notify_one();
        }
    }

    fn lane_active_limit(&self, lane: TaskLane) -> usize {
        let workers = self.worker_count().max(1);
        match lane {
            TaskLane::Simulation => workers,
            TaskLane::RenderPrep => workers.saturating_sub(1).max(1),
            TaskLane::Streaming => (workers / 2).max(1),
            TaskLane::AssetIo => (workers / 3).max(1),
            TaskLane::Plugin => 1,
            // Background work includes external compiler/tool processes. Keep it visible,
            // but never let it occupy the whole worker pool while frame-critical lanes wait.
            TaskLane::Background => (workers / 4).max(1),
        }
    }

    #[inline]
    fn lane_has_capacity(&self, lane: TaskLane) -> bool {
        self.running_by_lane[lane.index()].load(Ordering::Acquire) < self.lane_active_limit(lane)
    }

    #[inline]
    fn lane_has_helping_capacity(&self, lane: TaskLane) -> bool {
        let reentrant_depth = execution_depth_for_lane(lane);
        self.running_by_lane[lane.index()].load(Ordering::Acquire)
            < self.lane_active_limit(lane).saturating_add(reentrant_depth)
    }

    /// Keep a foreground reserve once bulk/background work consumes the soft
    /// frame budget. Critical work always proceeds; interactive Simulation and
    /// RenderPrep tasks may also proceed so AssetIo/Streaming throughput cannot
    /// turn the budget limiter into frame-critical priority inversion.
    #[inline]
    fn allowed_when_over_budget(priority: TaskPriority, lane: TaskLane) -> bool {
        priority == TaskPriority::Critical
            || (priority == TaskPriority::Interactive
                && matches!(lane, TaskLane::Simulation | TaskLane::RenderPrep))
    }

    fn has_schedulable_pending_work(&self) -> bool {
        let ready_mask = self.ready_queue_mask.load(Ordering::Acquire);
        if ready_mask == 0 {
            return false;
        }

        let over_budget = self.frame_over_budget();
        for priority in TaskPriority::service_order() {
            for lane in TaskLane::all() {
                if over_budget && !Self::allowed_when_over_budget(priority, lane) {
                    continue;
                }
                if !self.lane_has_capacity(lane) {
                    continue;
                }
                let idx = Self::queue_index(lane, priority);
                if ready_mask & Self::queue_bit(idx) != 0 {
                    return true;
                }
            }
        }
        false
    }

    pub(in crate::task_core) fn pop_next(&self) -> Option<QueuedTask> {
        // Shutdown is a drain phase, not a frame-budgeted phase. If the final
        // runtime frame exhausted the soft CPU budget, keeping that budget
        // active here can strand ready AssetIo/Streaming/Background work:
        // workers see pending > 0, but no queue is eligible, so join() can
        // never complete. Once shutdown is requested, drain every ready lane
        // regardless of the last frame's budget state.
        let over_budget = !self.shutdown.load(Ordering::Acquire) && self.frame_over_budget();
        for priority in TaskPriority::service_order() {
            for lane in TaskLane::all() {
                if over_budget && !Self::allowed_when_over_budget(priority, lane) {
                    continue;
                }
                if !self.lane_has_capacity(lane) {
                    continue;
                }
                let idx = Self::queue_index(lane, priority);
                if let Some(job) = self.try_pop_ready_queue(idx) {
                    let lane_index = job.request.lane.index();
                    self.pending_by_lane[lane_index].fetch_sub(1, Ordering::AcqRel);
                    self.pending.fetch_sub(1, Ordering::AcqRel);
                    self.running_by_lane[lane_index].fetch_add(1, Ordering::AcqRel);
                    return Some(job);
                }
            }
        }

        if over_budget && self.pending.load(Ordering::Acquire) > 0 {
            self.budget_deferred_polls.fetch_add(1, Ordering::AcqRel);
        }

        None
    }

    /// Pops work while a worker task is synchronously waiting for another
    /// task. The frame budget is intentionally not consulted here: progress on
    /// an explicit dependency must not deadlock behind the current frame budget.
    /// Lane caps remain enforced, with a strictly thread-local re-entrant allowance
    /// for lanes already present in this worker's execution stack.
    fn pop_next_helping(&self) -> Option<QueuedTask> {
        for priority in TaskPriority::service_order() {
            for lane in TaskLane::all() {
                if !self.lane_has_helping_capacity(lane) {
                    continue;
                }
                let idx = Self::queue_index(lane, priority);
                if let Some(job) = self.try_pop_ready_queue(idx) {
                    let lane_index = job.request.lane.index();
                    self.pending_by_lane[lane_index].fetch_sub(1, Ordering::AcqRel);
                    self.pending.fetch_sub(1, Ordering::AcqRel);
                    self.running_by_lane[lane_index].fetch_add(1, Ordering::AcqRel);
                    return Some(job);
                }
            }
        }
        None
    }

    #[inline]
    pub(in crate::task_core) fn set_frame_cpu_budget(&self, budget: Duration) {
        self.frame_cpu_budget_ns
            .store(duration_to_ns(budget), Ordering::Release);
    }

    #[inline]
    pub(in crate::task_core) fn begin_frame_budget(&self, budget: Duration) {
        self.set_frame_cpu_budget(budget);
        self.frame_cpu_used_ns.store(0, Ordering::Release);
        self.sleep_wake.notify_all();
    }

    #[inline]
    fn frame_over_budget(&self) -> bool {
        let budget = self.frame_cpu_budget_ns.load(Ordering::Acquire);
        budget > 0 && self.frame_cpu_used_ns.load(Ordering::Acquire) >= budget
    }

    pub(super) fn record_cpu_time_ns(&self, lane: TaskLane, elapsed_ns: u64) {
        self.total_cpu_time_ns
            .fetch_add(elapsed_ns, Ordering::AcqRel);
        self.cpu_time_ns_by_lane[lane.index()].fetch_add(elapsed_ns, Ordering::AcqRel);

        let budget = self.frame_cpu_budget_ns.load(Ordering::Acquire);
        if budget == 0 {
            return;
        }

        let previous = self
            .frame_cpu_used_ns
            .fetch_add(elapsed_ns, Ordering::AcqRel);
        let next = previous.saturating_add(elapsed_ns);
        if previous < budget && next >= budget {
            self.overbudget_frames.fetch_add(1, Ordering::AcqRel);
        }
    }

    pub(in crate::task_core) fn wait_for_work_or_shutdown(&self) {
        if self.shutdown.load(Ordering::Acquire) || self.has_schedulable_pending_work() {
            return;
        }

        let guard = self.sleep_lock.lock().unwrap_or_else(|e| e.into_inner());
        if self.shutdown.load(Ordering::Acquire) || self.has_schedulable_pending_work() {
            return;
        }

        let _guard = self
            .sleep_wake
            .wait(guard)
            .unwrap_or_else(|e| e.into_inner());
    }

    pub(in crate::task_core) fn snapshot(&self) -> ThreadPoolCoreSnapshot {
        let mut pending_by_lane = [0usize; JOB_LANE_COUNT];
        let mut running_by_lane = [0usize; JOB_LANE_COUNT];
        let mut completed_by_lane = [0u64; JOB_LANE_COUNT];
        let mut cpu_time_ns_by_lane = [0u64; JOB_LANE_COUNT];
        for lane in TaskLane::all() {
            pending_by_lane[lane.index()] =
                self.pending_by_lane[lane.index()].load(Ordering::Acquire);
            running_by_lane[lane.index()] =
                self.running_by_lane[lane.index()].load(Ordering::Acquire);
            completed_by_lane[lane.index()] =
                self.completed_by_lane[lane.index()].load(Ordering::Acquire);
            cpu_time_ns_by_lane[lane.index()] =
                self.cpu_time_ns_by_lane[lane.index()].load(Ordering::Acquire);
        }

        ThreadPoolCoreSnapshot {
            worker_threads: self.worker_count(),
            pending_jobs: self.pending.load(Ordering::Acquire),
            running_jobs: self.running.load(Ordering::Acquire),
            paused_jobs: self.paused.load(Ordering::Acquire),
            submitted_jobs: self.submitted.load(Ordering::Acquire),
            completed_jobs: self.completed.load(Ordering::Acquire),
            cancelled_jobs: self.cancelled.load(Ordering::Acquire),
            panicked_jobs: self.panicked.load(Ordering::Acquire),
            pending_by_lane,
            running_by_lane,
            completed_by_lane,
            total_cpu_time_ns: self.total_cpu_time_ns.load(Ordering::Acquire),
            frame_cpu_budget_ns: self.frame_cpu_budget_ns.load(Ordering::Acquire),
            frame_cpu_used_ns: self.frame_cpu_used_ns.load(Ordering::Acquire),
            overbudget_frames: self.overbudget_frames.load(Ordering::Acquire),
            budget_deferred_polls: self.budget_deferred_polls.load(Ordering::Acquire),
            cpu_time_ns_by_lane,
        }
    }
}
