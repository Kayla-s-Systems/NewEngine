#![forbid(unsafe_op_in_unsafe_fn)]

mod config;
mod control;
mod events;
mod id;
mod queue;
mod request;
mod service_model;
mod status;
mod worker;

pub(crate) use config::ThreadPoolCoreConfig;
pub use config::{
    TaskLane, TaskPriority, DEFAULT_FRAME_CPU_BUDGET_MS, JOB_LANE_COUNT, JOB_PRIORITY_COUNT,
};
pub(crate) use control::{CoreTaskControl, CoreTaskTicket};
pub use request::TaskRequest;
pub(crate) use service_model::{ThreadPoolCore, ThreadPoolCoreHandle};
pub(crate) use status::{CoreTaskRuntimeStatus, ThreadPoolCoreSnapshot};

#[cfg(test)]
mod tests {
    use super::{
        TaskLane, TaskPriority, TaskRequest, ThreadPoolCore, ThreadPoolCoreConfig,
        DEFAULT_FRAME_CPU_BUDGET_MS,
    };
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    #[test]
    fn pending_counter_returns_to_zero_after_task_completion() {
        let mut jobs = ThreadPoolCore::new(ThreadPoolCoreConfig {
            worker_threads: 1,
            frame_cpu_budget_ms: DEFAULT_FRAME_CPU_BUDGET_MS,
        });
        let ran = Arc::new(AtomicUsize::new(0));
        let ran_job = Arc::clone(&ran);

        let handle = jobs.handle();
        let ticket = handle.submit_request(
            TaskRequest::new("pending-counter-smoke")
                .with_lane(TaskLane::Background)
                .with_priority(TaskPriority::Critical),
            move || {
                ran_job.fetch_add(1, Ordering::SeqCst);
            },
        );
        ticket.wait();

        let snapshot = jobs.snapshot();
        assert_eq!(ran.load(Ordering::SeqCst), 1);
        assert_eq!(snapshot.pending_jobs, 0);
        assert_eq!(handle.pending_jobs(), 0);
        assert_eq!(handle.pending_for_lane(TaskLane::Background), 0);
        assert!(snapshot.total_cpu_time_ns > 0);

        jobs.shutdown_and_join();
    }
}
