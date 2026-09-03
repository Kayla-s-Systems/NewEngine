use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

static GLOBAL_SHUTDOWN: AtomicBool = AtomicBool::new(false);

/// Cooperative shutdown token.
///
/// Instance requests stop only the owning host/runtime. Process-wide shutdown
/// (for example Ctrl-C) must be requested explicitly through `global_request`.
#[derive(Clone)]
pub struct ShutdownToken {
    flag: Arc<AtomicBool>,
}

impl Default for ShutdownToken {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl ShutdownToken {
    #[inline]
    pub fn new() -> Self {
        Self {
            flag: Arc::new(AtomicBool::new(false)),
        }
    }

    #[inline]
    pub fn request(&self) {
        self.flag.store(true, Ordering::Relaxed);
    }

    #[inline]
    pub fn is_requested(&self) -> bool {
        self.flag.load(Ordering::Relaxed) || GLOBAL_SHUTDOWN.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn global_request() {
        GLOBAL_SHUTDOWN.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_shutdown_does_not_poison_independent_host_token() {
        let first = ShutdownToken::new();
        let second = ShutdownToken::new();

        first.request();

        assert!(first.is_requested());
        assert!(!second.is_requested());
    }
}
