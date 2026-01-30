use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

static GLOBAL_SHUTDOWN: AtomicBool = AtomicBool::new(false);

/// Cooperative shutdown token.
///
/// Platform adapter may set it from ctrl-c, window close, etc.
#[derive(Clone)]
pub struct ShutdownToken {
    flag: Arc<AtomicBool>,
}

impl ShutdownToken {
    #[inline]
    pub fn new() -> Self {
        Self { flag: Arc::new(AtomicBool::new(false)) }
    }

    #[inline]
    pub fn request(&self) {
        self.flag.store(true, Ordering::Relaxed);
        GLOBAL_SHUTDOWN.store(true, Ordering::Relaxed);
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