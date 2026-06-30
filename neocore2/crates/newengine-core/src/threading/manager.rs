use std::time::Duration;

use crate::events::EventHub;
use crate::task_core::ThreadPoolCore;
use newengine_loading_api::EngineTaskControlEvent;

use super::config::ThreadPoolConfig;
use super::handle::ThreadPoolHandle;
use super::snapshot::ThreadPoolSnapshot;

/// Host-owned CPU execution manager.
pub struct ThreadPoolManager {
    core: ThreadPoolCore,
    frame_cpu_budget: Duration,
}

impl ThreadPoolManager {
    #[inline]
    pub fn new(config: ThreadPoolConfig) -> Self {
        let frame_cpu_budget = Duration::from_millis(u64::from(config.frame_cpu_budget_ms));
        Self {
            core: ThreadPoolCore::new(config.to_core_config()),
            frame_cpu_budget,
        }
    }

    #[inline]
    pub fn new_with_event_hub(config: ThreadPoolConfig, events: EventHub) -> Self {
        let frame_cpu_budget = Duration::from_millis(u64::from(config.frame_cpu_budget_ms));
        Self {
            core: ThreadPoolCore::new_with_event_hub(config.to_core_config(), events),
            frame_cpu_budget,
        }
    }

    #[inline]
    pub fn handle(&self) -> ThreadPoolHandle {
        ThreadPoolHandle::new(self.core.handle())
    }

    #[inline]
    pub fn snapshot(&self) -> ThreadPoolSnapshot {
        ThreadPoolSnapshot::from_core_snapshot(self.core.snapshot())
    }

    #[inline]
    pub fn apply_control_event(&self, event: &EngineTaskControlEvent) -> bool {
        self.core.handle().apply_control_event(event)
    }

    #[inline]
    pub fn begin_configured_frame_budget(&self) {
        self.core.begin_frame_budget(self.frame_cpu_budget);
    }

    #[inline]
    pub fn begin_frame_budget(&self, budget: Duration) {
        self.core.begin_frame_budget(budget);
    }

    #[inline]
    pub fn set_frame_cpu_budget(&mut self, budget: Duration) {
        self.frame_cpu_budget = budget;
        self.core.set_frame_cpu_budget(budget);
    }

    #[inline]
    pub fn worker_threads(&self) -> usize {
        self.core.worker_threads()
    }

    #[inline]
    pub fn shutdown_and_join(&mut self) {
        self.core.shutdown_and_join();
    }
}

impl Default for ThreadPoolManager {
    #[inline]
    fn default() -> Self {
        Self::new(ThreadPoolConfig::default())
    }
}
