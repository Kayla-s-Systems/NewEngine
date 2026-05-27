#![forbid(unsafe_op_in_unsafe_fn)]

mod config;
mod control;
mod events;
mod id;
mod queue;
mod request;
mod service_model;
mod status;
mod tool_runner;
mod worker;

pub use config::{JobLane, JobPriority, JobSystemConfig, JOB_LANE_COUNT, JOB_PRIORITY_COUNT};
pub use control::{JobControl, JobTicket};
pub use request::JobRequest;
pub use service_model::{JobSystem, JobSystemHandle};
pub use status::{JobSystemSnapshot, JobTaskStatus};
pub use tool_runner::ToolJobRunner;


#[cfg(test)]
mod tests {
    use super::{JobLane, JobPriority, JobRequest, JobSystem, JobSystemConfig};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    #[test]
    fn pending_counter_returns_to_zero_after_job_completion() {
        let mut jobs = JobSystem::new(JobSystemConfig::fixed(1));
        let ran = Arc::new(AtomicUsize::new(0));
        let ran_job = Arc::clone(&ran);

        let ticket = jobs.submit_request(
            JobRequest::new("pending-counter-smoke")
                .with_lane(JobLane::Background)
                .with_priority(JobPriority::Critical),
            move || {
                ran_job.fetch_add(1, Ordering::SeqCst);
            },
        );
        ticket.wait();

        let snapshot = jobs.snapshot();
        assert_eq!(ran.load(Ordering::SeqCst), 1);
        assert_eq!(snapshot.pending_jobs, 0);
        assert_eq!(jobs.pending_jobs(), 0);
        assert_eq!(jobs.pending_for_lane(JobLane::Background), 0);

        jobs.shutdown_and_join();
    }
}
