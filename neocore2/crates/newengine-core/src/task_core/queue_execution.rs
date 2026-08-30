use super::hierarchy::TaskBodyOutcome;
use super::*;
use newengine_loading_api::EngineTaskPhase;
use newengine_plugin_host::with_host_context;
use std::cell::RefCell;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::Instant;

thread_local! {
    /// Execution stack exists only while this thread is actively executing work
    /// from the engine task core. External waiters therefore never become ad-hoc
    /// workers merely by calling `CoreTaskTicket::wait`.
    static EXECUTION_LANES: RefCell<Vec<TaskLane>> = const { RefCell::new(Vec::new()) };
}

pub(super) struct TaskExecutionScope;

impl TaskExecutionScope {
    #[inline]
    pub(super) fn enter(lane: TaskLane) -> Self {
        EXECUTION_LANES.with(|lanes| lanes.borrow_mut().push(lane));
        Self
    }
}

impl Drop for TaskExecutionScope {
    fn drop(&mut self) {
        EXECUTION_LANES.with(|lanes| {
            let _ = lanes.borrow_mut().pop();
        });
    }
}

#[inline]
pub(super) fn execution_depth_for_lane(lane: TaskLane) -> usize {
    EXECUTION_LANES.with(|lanes| {
        lanes
            .borrow()
            .iter()
            .filter(|active_lane| **active_lane == lane)
            .count()
    })
}

#[inline]
pub(super) fn in_worker_task_context() -> bool {
    EXECUTION_LANES.with(|lanes| !lanes.borrow().is_empty())
}

#[inline]
pub(super) fn duration_to_ns(duration: Duration) -> u64 {
    duration.as_nanos().min(u128::from(u64::MAX)) as u64
}

impl QueuedTask {
    pub(super) fn run(self, shared: &TaskCoreShared) {
        let host_context = self.control.host_context();
        with_host_context(&host_context, || self.run_in_host_context(shared));
    }

    fn run_in_host_context(mut self, shared: &TaskCoreShared) {
        let lane = self.request.lane;
        let task_id = self.control.task_id().to_owned();

        let Some(job) = self.job.take() else {
            shared.finish_task_body(&task_id, TaskBodyOutcome::CompletedNoClosure);
            shared.release_lane(lane);
            return;
        };

        if self.control.is_cancel_requested() {
            shared.finish_task_body(&task_id, TaskBodyOutcome::CancelledBeforeExecution);
            shared.release_lane(lane);
            return;
        }

        shared.running.fetch_add(1, Ordering::AcqRel);
        self.control.publish(
            EngineTaskPhase::Running,
            "Task running",
            "Worker picked up the task from the engine queue.",
            None,
        );
        if !self.control.wait_while_paused() {
            shared.running.fetch_sub(1, Ordering::AcqRel);
            shared.finish_task_body(&task_id, TaskBodyOutcome::CancelledWhilePaused);
            shared.release_lane(lane);
            return;
        }

        let control = self.control.clone();
        let started = Instant::now();
        let result = catch_unwind(AssertUnwindSafe(move || {
            job(control);
        }));
        let elapsed_ns = started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
        shared.record_cpu_time_ns(lane, elapsed_ns);
        shared.running.fetch_sub(1, Ordering::AcqRel);

        let outcome = if result.is_err() {
            newengine_ulog_api::ulog::error!(
                "task-core: worker task panicked label='{}' lane='{}' priority={:?}; worker recovered and continues",
                self.request.label,
                lane.as_str(),
                self.request.priority
            );
            TaskBodyOutcome::Failed
        } else if self.control.is_cancel_requested() {
            TaskBodyOutcome::CancelledAfterExecution
        } else {
            TaskBodyOutcome::Completed
        };

        shared.finish_task_body(&task_id, outcome);
        shared.release_lane(lane);
    }
}
