use std::collections::VecDeque;
use std::time::Duration;

/// Scheduler phase within a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulePhase {
    /// Runs at the very beginning of `Engine::step()` (before fixed/update/render).
    BeginFrame,
    /// Runs at the end of `Engine::step()` (after fixed/update/render).
    EndFrame,
}

/// A tiny scheduler that provides a strict timing contract without forcing an execution model.
///
/// - It is intentionally engine-thread local.
/// - Tasks are executed synchronously on the engine thread.
/// - Tasks are non-capturing beyond what you store inside the closure.
///
/// This is a foundation: you can later add time-based jobs, priorities, fibers, async bridges, etc.
pub struct Scheduler {
    begin: VecDeque<Task>,
    end: VecDeque<Task>,
    frame_dt: Duration,
}

type Task = Box<dyn FnOnce() + Send + 'static>;

impl Scheduler {
    #[inline]
    pub fn new() -> Self {
        Self {
            begin: VecDeque::new(),
            end: VecDeque::new(),
            frame_dt: Duration::from_secs(0),
        }
    }

    /// Enqueue a task to be executed in the given frame phase.
    ///
    /// The task executes on the engine thread and must never block for long.
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

    /// Called by the engine at the very beginning of a frame.
    #[inline]
    pub fn begin_frame(&mut self, dt: Duration) {
        self.frame_dt = dt;
        Self::run_queue(&mut self.begin);
    }

    /// Called by the engine at the end of a frame.
    #[inline]
    pub fn end_frame(&mut self, dt: Duration) {
        self.frame_dt = dt;
        Self::run_queue(&mut self.end);
    }

    /// Last frame delta as provided by the engine.
    #[inline]
    pub fn frame_dt(&self) -> Duration {
        self.frame_dt
    }

    #[inline]
    fn run_queue(q: &mut VecDeque<Task>) {
        while let Some(job) = q.pop_front() {
            job();
        }
    }
}

impl Default for Scheduler {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
