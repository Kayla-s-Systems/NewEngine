pub const JOB_LANE_COUNT: usize = 6;
pub const JOB_PRIORITY_COUNT: usize = 4;
pub const DEFAULT_FRAME_CPU_BUDGET_MS: u32 = 16;

/// Stable work lane used by engine systems when they submit CPU work.
///
/// This is intentionally a contract, not just telemetry. The scheduler can
/// protect frame-critical lanes from bulk streaming/background work without each
/// module inventing its own thread pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TaskLane {
    Simulation,
    RenderPrep,
    Streaming,
    AssetIo,
    Plugin,
    Background,
}

impl TaskLane {
    #[inline]
    pub const fn index(self) -> usize {
        match self {
            Self::Simulation => 0,
            Self::RenderPrep => 1,
            Self::Streaming => 2,
            Self::AssetIo => 3,
            Self::Plugin => 4,
            Self::Background => 5,
        }
    }

    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Simulation => "simulation",
            Self::RenderPrep => "render-prep",
            Self::Streaming => "streaming",
            Self::AssetIo => "asset-io",
            Self::Plugin => "plugin",
            Self::Background => "background",
        }
    }

    #[inline]
    pub const fn all() -> [Self; JOB_LANE_COUNT] {
        [
            Self::Simulation,
            Self::RenderPrep,
            Self::Streaming,
            Self::AssetIo,
            Self::Plugin,
            Self::Background,
        ]
    }
}

impl Default for TaskLane {
    #[inline]
    fn default() -> Self {
        Self::Simulation
    }
}

/// Stable configuration for the engine-wide CPU job system.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThreadPoolCoreConfig {
    /// Number of persistent worker threads. Values lower than 1 are clamped.
    pub worker_threads: usize,
    /// Soft per-frame CPU budget for non-critical worker jobs.
    ///
    /// Once this budget is consumed, the worker pool only services critical jobs
    /// until the engine opens the next frame budget window.
    pub frame_cpu_budget_ms: u32,
}

impl ThreadPoolCoreConfig {
    #[inline]
    pub fn auto() -> Self {
        let logical = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        Self {
            worker_threads: logical.saturating_sub(1).max(1),
            frame_cpu_budget_ms: DEFAULT_FRAME_CPU_BUDGET_MS,
        }
    }
}

impl Default for ThreadPoolCoreConfig {
    #[inline]
    fn default() -> Self {
        Self::auto()
    }
}

/// Stable job priority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum TaskPriority {
    Background,
    #[default]
    Normal,
    Interactive,
    Critical,
}

impl TaskPriority {
    #[inline]
    pub const fn index(self) -> usize {
        match self {
            Self::Background => 0,
            Self::Normal => 1,
            Self::Interactive => 2,
            Self::Critical => 3,
        }
    }

    #[inline]
    pub const fn service_order() -> [Self; JOB_PRIORITY_COUNT] {
        [
            Self::Critical,
            Self::Interactive,
            Self::Normal,
            Self::Background,
        ]
    }

    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Background => "background",
            Self::Normal => "normal",
            Self::Interactive => "interactive",
            Self::Critical => "critical",
        }
    }
}
