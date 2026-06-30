use crate::task_core::{ThreadPoolCoreConfig, DEFAULT_FRAME_CPU_BUDGET_MS};

pub const ENGINE_THREADING_GATEWAY_ID: &str = "engine.threading";
pub const THREADING_PROVIDER_SERVICE_ID: &str = "threading.api";
pub const THREADING_BACKEND_CAPABILITY_ID: &str = "threading.backend";
pub const THREADING_RUNTIME_CONTRACT: &str = "newengine.threading.runtime.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThreadPoolConfig {
    pub worker_threads: usize,
    pub frame_cpu_budget_ms: u32,
}

impl ThreadPoolConfig {
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

    #[inline]
    pub const fn fixed(worker_threads: usize) -> Self {
        Self {
            worker_threads,
            frame_cpu_budget_ms: DEFAULT_FRAME_CPU_BUDGET_MS,
        }
    }

    #[inline]
    pub const fn with_frame_cpu_budget_ms(mut self, frame_cpu_budget_ms: u32) -> Self {
        self.frame_cpu_budget_ms = frame_cpu_budget_ms;
        self
    }

    #[inline]
    pub(crate) const fn to_core_config(self) -> ThreadPoolCoreConfig {
        ThreadPoolCoreConfig {
            worker_threads: self.worker_threads,
            frame_cpu_budget_ms: self.frame_cpu_budget_ms,
        }
    }
}

impl Default for ThreadPoolConfig {
    #[inline]
    fn default() -> Self {
        Self::auto()
    }
}
