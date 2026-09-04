use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::time::Duration;

use newengine_loading_api::LoadingScreenSnapshot;

#[derive(Clone)]
pub struct SharedLoadingSnapshot {
    inner: Arc<RwLock<LoadingScreenSnapshot>>,
    version: Arc<AtomicU64>,
    wake: Arc<(Mutex<u64>, Condvar)>,
}

impl Default for SharedLoadingSnapshot {
    #[inline]
    fn default() -> Self {
        Self::new(LoadingScreenSnapshot::inactive())
    }
}

impl SharedLoadingSnapshot {
    #[inline]
    pub fn new(initial: LoadingScreenSnapshot) -> Self {
        Self {
            inner: Arc::new(RwLock::new(initial.normalize())),
            version: Arc::new(AtomicU64::new(1)),
            wake: Arc::new((Mutex::new(1), Condvar::new())),
        }
    }

    #[inline]
    pub fn publish(&self, snapshot: LoadingScreenSnapshot) {
        let mut next = snapshot.normalize();
        match self.inner.write() {
            Ok(mut guard) => {
                if next.active && guard.active {
                    next.progress_01 = next.progress_01.max(guard.progress_01);
                }
                *guard = next;
            }
            Err(e) => {
                let mut guard = e.into_inner();
                if next.active && guard.active {
                    next.progress_01 = next.progress_01.max(guard.progress_01);
                }
                *guard = next;
            }
        }
        let version = self.version.fetch_add(1, Ordering::AcqRel) + 1;
        let (lock, cv) = &*self.wake;
        match lock.lock() {
            Ok(mut guard) => *guard = version,
            Err(e) => *e.into_inner() = version,
        }
        cv.notify_all();
    }

    #[inline]
    pub fn snapshot(&self) -> LoadingScreenSnapshot {
        match self.inner.read() {
            Ok(guard) => guard.clone(),
            Err(e) => e.into_inner().clone(),
        }
    }

    #[inline]
    pub fn version(&self) -> u64 {
        self.version.load(Ordering::Acquire)
    }

    pub fn wait_for_update_or_timeout(&self, observed_version: u64, timeout: Duration) -> u64 {
        if self.version() != observed_version {
            return self.version();
        }
        let (lock, cv) = &*self.wake;
        let guard = match lock.lock() {
            Ok(guard) => guard,
            Err(e) => e.into_inner(),
        };
        let result = cv
            .wait_timeout_while(guard, timeout, |version| *version == observed_version)
            .unwrap_or_else(|e| e.into_inner());
        *result.0
    }
}
