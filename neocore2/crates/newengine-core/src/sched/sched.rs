use std::time::Duration;

/// Lightweight scheduler placeholder.
///
/// Replace with your existing scheduler implementation.
pub struct Scheduler;

impl Scheduler {
    #[inline]
    pub fn new() -> Self {
        Self
    }

    #[inline]
    pub fn tick(&mut self, _dt: Duration) {}
}
