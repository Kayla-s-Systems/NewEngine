use crate::jobs::{JobSystemHandle, JobTicket};
use newengine_math::collections_prelude::NeVecDeque as VecDeque;
use std::time::Duration;

/// Scheduler phase within a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulePhase {
    /// Runs at the very beginning of `Engine::step()` (before fixed/update/render).
    BeginFrame,
    /// Runs at the end of `Engine::step()` (after fixed/update/render).
    EndFrame,
}

/// Frame-phase scheduler backed by the engine JobSystem.
///
/// The scheduler keeps the deterministic phase barrier, but task execution is no
/// longer hardwired to the engine thread: phase queues are drained into the
/// standard JobSystem and joined before the frame proceeds.
pub struct Scheduler {
    begin: VecDeque<Task>,
    end: VecDeque<Task>,
    jobs: Option<JobSystemHandle>,
    frame_dt: Duration,
}

type Task = Box<dyn FnOnce() + Send + 'static>;

impl Scheduler {
    #[inline]
    pub fn new() -> Self {
        Self {
            begin: VecDeque::new(),
            end: VecDeque::new(),
            jobs: None,
            frame_dt: Duration::from_secs(0),
        }
    }

    #[inline]
    pub fn with_job_system(jobs: JobSystemHandle) -> Self {
        Self {
            jobs: Some(jobs),
            ..Self::new()
        }
    }

    #[inline]
    pub fn set_job_system(&mut self, jobs: JobSystemHandle) {
        self.jobs = Some(jobs);
    }

    /// Enqueue a task to be executed in the given frame phase.
    ///
    /// The task is dispatched through [`JobSystemHandle`] at the phase barrier.
    /// Use [`Self::submit_now`] for immediate background work.
    #[inline]
    pub fn schedule<F>(&mut self, phase: SchedulePhase, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        match phase {
            SchedulePhase::BeginFrame => self.begin.push_back(Box::new(f)),
            SchedulePhase::EndFrame => self.end.push_back(Box::new(f)),
        }
    }

    /// Submit work immediately through the engine-standard job pipeline.
    /// Falls back to synchronous execution only when the scheduler was created
    /// outside an engine and no JobSystem has been attached.
    pub fn submit_now<F>(&self, label: &'static str, f: F) -> Option<JobTicket>
    where
        F: FnOnce() + Send + 'static,
    {
        if let Some(jobs) = &self.jobs {
            Some(jobs.submit_named(label, f))
        } else {
            f();
            None
        }
    }

    /// Called by the engine at the very beginning of a frame.
    #[inline]
    pub fn begin_frame(&mut self, dt: Duration) {
        self.frame_dt = dt;
        Self::run_queue(&self.jobs, "scheduler.begin_frame", &mut self.begin);
    }

    /// Called by the engine at the end of a frame.
    #[inline]
    pub fn end_frame(&mut self, dt: Duration) {
        self.frame_dt = dt;
        Self::run_queue(&self.jobs, "scheduler.end_frame", &mut self.end);
    }

    /// Last frame delta as provided by the engine.
    #[inline]
    pub fn frame_dt(&self) -> Duration {
        self.frame_dt
    }

    #[inline]
    pub fn job_system(&self) -> Option<&JobSystemHandle> {
        self.jobs.as_ref()
    }

    fn run_queue(jobs: &Option<JobSystemHandle>, label: &'static str, q: &mut VecDeque<Task>) {
        if q.is_empty() {
            return;
        }

        if let Some(jobs) = jobs {
            let mut tickets = Vec::with_capacity(q.len());
            while let Some(task) = q.pop_front() {
                tickets.push(jobs.submit_named(label, move || task()));
            }
            for ticket in tickets {
                ticket.wait();
            }
        } else {
            while let Some(task) = q.pop_front() {
                task();
            }
        }
    }
}

impl Default for Scheduler {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
