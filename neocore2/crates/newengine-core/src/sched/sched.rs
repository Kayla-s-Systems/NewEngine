use newengine_math::collections_prelude::NeVecDeque as VecDeque;
use std::time::{Duration, Instant};

pub const SCHEDULE_PHASE_COUNT: usize = 5;

/// Scheduler phase within a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SchedulePhase {
    BeginFrame,
    FixedUpdate,
    Update,
    Render,
    EndFrame,
}

impl SchedulePhase {
    #[inline]
    pub const fn index(self) -> usize {
        match self {
            Self::BeginFrame => 0,
            Self::FixedUpdate => 1,
            Self::Update => 2,
            Self::Render => 3,
            Self::EndFrame => 4,
        }
    }

    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BeginFrame => "begin-frame",
            Self::FixedUpdate => "fixed-update",
            Self::Update => "update",
            Self::Render => "render",
            Self::EndFrame => "end-frame",
        }
    }

    #[inline]
    pub const fn all() -> [Self; SCHEDULE_PHASE_COUNT] {
        [
            Self::BeginFrame,
            Self::FixedUpdate,
            Self::Update,
            Self::Render,
            Self::EndFrame,
        ]
    }
}

/// Work budget class for engine-thread tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ScheduleBudgetClass {
    /// Must run this frame. Use sparingly for state publication and tiny command commits.
    Critical,
    /// Frame-visible work such as gameplay command application or render packet finalization.
    #[default]
    Interactive,
    /// Maintenance work that may be time-sliced across frames.
    Maintenance,
    /// Opportunistic background work. It should never make the frame miss its budget.
    Background,
}

impl ScheduleBudgetClass {
    #[inline]
    const fn max_tasks_per_slice(self) -> usize {
        match self {
            Self::Critical => usize::MAX,
            Self::Interactive => 256,
            Self::Maintenance => 64,
            Self::Background => 16,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScheduleTaskDesc {
    pub label: &'static str,
    pub budget_class: ScheduleBudgetClass,
}

impl ScheduleTaskDesc {
    #[inline]
    pub const fn new(label: &'static str) -> Self {
        Self {
            label,
            budget_class: ScheduleBudgetClass::Interactive,
        }
    }

    #[inline]
    pub const fn with_budget_class(mut self, budget_class: ScheduleBudgetClass) -> Self {
        self.budget_class = budget_class;
        self
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SchedulePhaseStats {
    pub queued: usize,
    pub executed: u64,
    pub deferred: u64,
    pub panicked: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SchedulerSnapshot {
    pub frame_dt: Duration,
    pub phase_stats: [SchedulePhaseStats; SCHEDULE_PHASE_COUNT],
}

impl SchedulerSnapshot {
    #[inline]
    pub fn stats_for(&self, phase: SchedulePhase) -> SchedulePhaseStats {
        self.phase_stats[phase.index()]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScheduleRunReport {
    pub phase: SchedulePhase,
    pub executed: usize,
    pub deferred: usize,
    pub elapsed: Duration,
}

struct ScheduledTask {
    desc: ScheduleTaskDesc,
    f: Task,
}

type Task = Box<dyn FnOnce() + Send + 'static>;

/// Engine-thread scheduler with explicit frame phases and soft time slicing.
///
/// The scheduler is intentionally local to the engine thread. CPU-heavy work must
/// use the job system; this scheduler is for deterministic commits, state swaps,
/// command application and small callbacks that must be ordered inside a frame.
pub struct Scheduler {
    queues: [VecDeque<ScheduledTask>; SCHEDULE_PHASE_COUNT],
    stats: [SchedulePhaseStats; SCHEDULE_PHASE_COUNT],
    frame_dt: Duration,
    phase_budget: Duration,
}

impl Scheduler {
    #[inline]
    pub fn new() -> Self {
        Self {
            queues: std::array::from_fn(|_| VecDeque::new()),
            stats: [SchedulePhaseStats::default(); SCHEDULE_PHASE_COUNT],
            frame_dt: Duration::from_secs(0),
            phase_budget: Duration::from_micros(750),
        }
    }

    /// Enqueue a task to be executed in the given frame phase.
    #[inline]
    pub fn schedule<F>(&mut self, phase: SchedulePhase, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.schedule_with(phase, ScheduleTaskDesc::new("scheduled-task"), f);
    }

    /// Enqueue a named task with an explicit budget class.
    #[inline]
    pub fn schedule_with<F>(&mut self, phase: SchedulePhase, desc: ScheduleTaskDesc, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let idx = phase.index();
        self.queues[idx].push_back(ScheduledTask {
            desc,
            f: Box::new(f),
        });
        self.stats[idx].queued = self.queues[idx].len();
    }

    #[inline]
    pub fn set_phase_budget(&mut self, budget: Duration) {
        self.phase_budget = budget;
    }

    #[inline]
    pub fn begin_frame(&mut self, dt: Duration) -> ScheduleRunReport {
        self.frame_dt = dt;
        self.run_phase(SchedulePhase::BeginFrame)
    }

    #[inline]
    pub fn run_fixed_update(&mut self, dt: Duration) -> ScheduleRunReport {
        self.frame_dt = dt;
        self.run_phase(SchedulePhase::FixedUpdate)
    }

    #[inline]
    pub fn run_update(&mut self, dt: Duration) -> ScheduleRunReport {
        self.frame_dt = dt;
        self.run_phase(SchedulePhase::Update)
    }

    #[inline]
    pub fn run_render(&mut self, dt: Duration) -> ScheduleRunReport {
        self.frame_dt = dt;
        self.run_phase(SchedulePhase::Render)
    }

    #[inline]
    pub fn end_frame(&mut self, dt: Duration) -> ScheduleRunReport {
        self.frame_dt = dt;
        self.run_phase(SchedulePhase::EndFrame)
    }

    pub fn run_phase(&mut self, phase: SchedulePhase) -> ScheduleRunReport {
        let idx = phase.index();
        let started = Instant::now();
        let mut executed = 0usize;
        let mut deferred = 0usize;
        let mut keep = VecDeque::new();

        while let Some(task) = self.queues[idx].pop_front() {
            let elapsed = started.elapsed();
            let can_defer = task.desc.budget_class != ScheduleBudgetClass::Critical;
            let max_tasks = task.desc.budget_class.max_tasks_per_slice();

            if can_defer && (elapsed >= self.phase_budget || executed >= max_tasks) {
                keep.push_back(task);
                deferred += 1;
                continue;
            }

            let label = task.desc.label;
            let f = task.f;
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                f();
            }));
            executed += 1;
            self.stats[idx].executed = self.stats[idx].executed.saturating_add(1);

            if result.is_err() {
                self.stats[idx].panicked = self.stats[idx].panicked.saturating_add(1);
                newengine_ulog_api::ulog::error!(
                    "scheduler: task panicked phase='{}' label='{}'",
                    phase.as_str(),
                    label
                );
            }
        }

        if !keep.is_empty() {
            self.stats[idx].deferred = self.stats[idx].deferred.saturating_add(keep.len() as u64);
            while let Some(task) = keep.pop_back() {
                self.queues[idx].push_front(task);
            }
        }

        self.stats[idx].queued = self.queues[idx].len();

        ScheduleRunReport {
            phase,
            executed,
            deferred,
            elapsed: started.elapsed(),
        }
    }

    /// Last frame delta as provided by the engine.
    #[inline]
    pub fn frame_dt(&self) -> Duration {
        self.frame_dt
    }

    #[inline]
    pub fn queued(&self, phase: SchedulePhase) -> usize {
        self.queues[phase.index()].len()
    }

    pub fn snapshot(&self) -> SchedulerSnapshot {
        let mut phase_stats = self.stats;
        for phase in SchedulePhase::all() {
            phase_stats[phase.index()].queued = self.queues[phase.index()].len();
        }
        SchedulerSnapshot {
            frame_dt: self.frame_dt,
            phase_stats,
        }
    }
}

impl Default for Scheduler {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
