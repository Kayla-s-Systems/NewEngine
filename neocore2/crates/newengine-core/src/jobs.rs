#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::collections_prelude::NeVecDeque as VecDeque;
use parking_lot::Mutex;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex as StdMutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

type JobFn = Box<dyn FnOnce() + Send + 'static>;

/// Stable configuration for the engine-wide CPU job system.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JobSystemConfig {
    /// Number of persistent worker threads. Values lower than 1 are clamped.
    pub worker_threads: usize,
}

impl JobSystemConfig {
    #[inline]
    pub fn auto() -> Self {
        let logical = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        Self {
            worker_threads: logical.saturating_sub(1).max(1),
        }
    }
}

impl Default for JobSystemConfig {
    #[inline]
    fn default() -> Self {
        Self::auto()
    }
}

/// Stable job priority. The first implementation keeps queues FIFO/local-first;
/// priority is stored in telemetry and leaves room for strict lane scheduling.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum JobPriority {
    Background,
    #[default]
    Normal,
    Interactive,
    Critical,
}

/// Engine-standard task envelope used by systems that submit CPU work.
#[derive(Clone, Debug)]
pub struct JobRequest {
    pub label: &'static str,
    pub priority: JobPriority,
}

impl JobRequest {
    #[inline]
    pub const fn new(label: &'static str) -> Self {
        Self {
            label,
            priority: JobPriority::Normal,
        }
    }

    #[inline]
    pub const fn with_priority(mut self, priority: JobPriority) -> Self {
        self.priority = priority;
        self
    }
}

/// Lightweight job-system snapshot for profiling and UI provider read-models.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct JobSystemSnapshot {
    pub worker_threads: usize,
    pub pending_jobs: usize,
    pub submitted_jobs: u64,
    pub completed_jobs: u64,
    pub panicked_jobs: u64,
}

/// Wait handle for a submitted CPU job.
pub struct JobTicket {
    completion: Arc<JobCompletion>,
}

impl JobTicket {
    #[inline]
    pub fn is_complete(&self) -> bool {
        self.completion.is_complete()
    }

    #[inline]
    pub fn wait(self) {
        self.completion.wait();
    }
}

struct JobCompletion {
    done: AtomicBool,
    lock: StdMutex<()>,
    wake: Condvar,
}

impl JobCompletion {
    #[inline]
    fn new() -> Self {
        Self {
            done: AtomicBool::new(false),
            lock: StdMutex::new(()),
            wake: Condvar::new(),
        }
    }

    #[inline]
    fn is_complete(&self) -> bool {
        self.done.load(Ordering::Acquire)
    }

    fn wait(&self) {
        if self.is_complete() {
            return;
        }

        let mut guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        while !self.is_complete() {
            guard = self.wake.wait(guard).unwrap_or_else(|e| e.into_inner());
        }
    }

    fn complete(&self) {
        self.done.store(true, Ordering::Release);
        self.wake.notify_all();
    }
}

struct QueuedJob {
    request: JobRequest,
    job: Option<JobFn>,
    completion: Arc<JobCompletion>,
}

impl QueuedJob {
    fn run(mut self, shared: &JobShared) {
        let Some(job) = self.job.take() else {
            self.completion.complete();
            shared.completed.fetch_add(1, Ordering::AcqRel);
            return;
        };

        let result = catch_unwind(AssertUnwindSafe(move || {
            job();
        }));
        self.completion.complete();
        shared.completed.fetch_add(1, Ordering::AcqRel);

        if result.is_err() {
            shared.panicked.fetch_add(1, Ordering::AcqRel);
            log::error!(
                "job-system: worker job panicked label='{}' priority={:?}; worker recovered and continues",
                self.request.label,
                self.request.priority
            );
        }
    }
}

struct JobShared {
    queues: Vec<Mutex<VecDeque<QueuedJob>>>,
    next_queue: AtomicUsize,
    pending: AtomicUsize,
    submitted: AtomicU64,
    completed: AtomicU64,
    panicked: AtomicU64,
    shutdown: AtomicBool,
    sleep_lock: StdMutex<()>,
    sleep_wake: Condvar,
}

impl JobShared {
    fn new(worker_threads: usize) -> Self {
        let worker_threads = worker_threads.max(1);
        Self {
            queues: (0..worker_threads)
                .map(|_| Mutex::new(VecDeque::new()))
                .collect(),
            next_queue: AtomicUsize::new(0),
            pending: AtomicUsize::new(0),
            submitted: AtomicU64::new(0),
            completed: AtomicU64::new(0),
            panicked: AtomicU64::new(0),
            shutdown: AtomicBool::new(false),
            sleep_lock: StdMutex::new(()),
            sleep_wake: Condvar::new(),
        }
    }

    #[inline]
    fn worker_count(&self) -> usize {
        self.queues.len().max(1)
    }

    fn submit(&self, job: QueuedJob) {
        if self.shutdown.load(Ordering::Acquire) {
            job.completion.complete();
            return;
        }

        self.submitted.fetch_add(1, Ordering::AcqRel);
        self.pending.fetch_add(1, Ordering::Release);
        let idx = self.next_queue.fetch_add(1, Ordering::Relaxed) % self.worker_count();
        self.queues[idx].lock().push_back(job);
        self.sleep_wake.notify_one();
    }

    fn pop_local_or_steal(&self, worker_index: usize) -> Option<QueuedJob> {
        let count = self.worker_count();
        let local = worker_index % count;

        if let Some(job) = self.queues[local].lock().pop_front() {
            return Some(job);
        }

        for offset in 1..count {
            let victim = (local + offset) % count;
            if let Some(job) = self.queues[victim].lock().pop_back() {
                return Some(job);
            }
        }

        None
    }

    fn wait_for_work_or_shutdown(&self) {
        if self.shutdown.load(Ordering::Acquire) || self.pending.load(Ordering::Acquire) > 0 {
            return;
        }

        let guard = self.sleep_lock.lock().unwrap_or_else(|e| e.into_inner());
        let _ = self
            .sleep_wake
            .wait_timeout(guard, Duration::from_millis(2))
            .unwrap_or_else(|e| e.into_inner());
    }

    #[inline]
    fn snapshot(&self) -> JobSystemSnapshot {
        JobSystemSnapshot {
            worker_threads: self.worker_count(),
            pending_jobs: self.pending.load(Ordering::Acquire),
            submitted_jobs: self.submitted.load(Ordering::Acquire),
            completed_jobs: self.completed.load(Ordering::Acquire),
            panicked_jobs: self.panicked.load(Ordering::Acquire),
        }
    }
}

/// Cloneable submission endpoint. Engine systems should depend on this handle,
/// not on ad-hoc threads, `std::thread::spawn`, or backend-specific workers.
#[derive(Clone)]
pub struct JobSystemHandle {
    shared: Arc<JobShared>,
}

impl JobSystemHandle {
    pub fn submit<F>(&self, f: F) -> JobTicket
    where
        F: FnOnce() + Send + 'static,
    {
        self.submit_request(JobRequest::new("job"), f)
    }

    pub fn submit_named<F>(&self, label: &'static str, f: F) -> JobTicket
    where
        F: FnOnce() + Send + 'static,
    {
        self.submit_request(JobRequest::new(label), f)
    }

    pub fn submit_request<F>(&self, request: JobRequest, f: F) -> JobTicket
    where
        F: FnOnce() + Send + 'static,
    {
        let completion = Arc::new(JobCompletion::new());
        let ticket = JobTicket {
            completion: Arc::clone(&completion),
        };
        self.shared.submit(QueuedJob {
            request,
            job: Some(Box::new(f)),
            completion,
        });
        ticket
    }

    /// Runs indexed jobs and returns results in index order, regardless of worker execution order.
    pub fn run_indexed<T, F>(&self, len: usize, f: F) -> Vec<T>
    where
        T: Send + 'static,
        F: Fn(usize) -> T + Send + Sync + 'static,
    {
        if len == 0 {
            return Vec::new();
        }

        let f = Arc::new(f);
        let results = Arc::new(Mutex::new(Vec::<Option<T>>::new()));
        results.lock().resize_with(len, || None);

        let mut tickets = Vec::with_capacity(len);
        for index in 0..len {
            let f = Arc::clone(&f);
            let results = Arc::clone(&results);
            tickets.push(self.submit_named("indexed", move || {
                let value = f(index);
                results.lock()[index] = Some(value);
            }));
        }

        for ticket in tickets {
            ticket.wait();
        }

        let mut guard = results.lock();
        std::mem::take(&mut *guard)
            .into_iter()
            .map(|value| value.expect("job-system: indexed job completed without result"))
            .collect()
    }

    #[inline]
    pub fn pending_jobs(&self) -> usize {
        self.shared.pending.load(Ordering::Acquire)
    }

    #[inline]
    pub fn worker_threads(&self) -> usize {
        self.shared.worker_count()
    }

    #[inline]
    pub fn snapshot(&self) -> JobSystemSnapshot {
        self.shared.snapshot()
    }
}

/// Persistent CPU job system with per-worker queues and deterministic submission handles.
///
/// GPU work stays on the render thread. CPU-heavy work such as terrain generation,
/// component preparation and asset preprocessing goes through this standard pipeline.
pub struct JobSystem {
    handle: JobSystemHandle,
    workers: Vec<JoinHandle<()>>,
}

impl JobSystem {
    pub fn new(config: JobSystemConfig) -> Self {
        let worker_threads = config.worker_threads.max(1);
        let shared = Arc::new(JobShared::new(worker_threads));
        let handle = JobSystemHandle {
            shared: Arc::clone(&shared),
        };
        let mut workers = Vec::with_capacity(worker_threads);

        for index in 0..worker_threads {
            let worker_shared = Arc::clone(&shared);
            let name = format!("newengine-job-{index}");
            let handle = thread::Builder::new()
                .name(name)
                .spawn(move || worker_loop(index, worker_shared))
                .expect("job-system: failed to spawn worker thread");
            workers.push(handle);
        }

        Self { handle, workers }
    }

    #[inline]
    pub fn new_auto() -> Self {
        Self::new(JobSystemConfig::auto())
    }

    #[inline]
    pub fn handle(&self) -> JobSystemHandle {
        self.handle.clone()
    }

    #[inline]
    pub fn worker_threads(&self) -> usize {
        self.handle.worker_threads()
    }

    #[inline]
    pub fn pending_jobs(&self) -> usize {
        self.handle.pending_jobs()
    }

    #[inline]
    pub fn snapshot(&self) -> JobSystemSnapshot {
        self.handle.snapshot()
    }

    pub fn submit<F>(&self, f: F) -> JobTicket
    where
        F: FnOnce() + Send + 'static,
    {
        self.handle.submit(f)
    }

    pub fn submit_named<F>(&self, label: &'static str, f: F) -> JobTicket
    where
        F: FnOnce() + Send + 'static,
    {
        self.handle.submit_named(label, f)
    }

    pub fn submit_request<F>(&self, request: JobRequest, f: F) -> JobTicket
    where
        F: FnOnce() + Send + 'static,
    {
        self.handle.submit_request(request, f)
    }

    #[inline]
    pub fn run_indexed<T, F>(&self, len: usize, f: F) -> Vec<T>
    where
        T: Send + 'static,
        F: Fn(usize) -> T + Send + Sync + 'static,
    {
        self.handle.run_indexed(len, f)
    }
}

impl Default for JobSystem {
    #[inline]
    fn default() -> Self {
        Self::new_auto()
    }
}

impl Drop for JobSystem {
    fn drop(&mut self) {
        self.handle.shared.shutdown.store(true, Ordering::Release);
        self.handle.shared.sleep_wake.notify_all();

        for handle in self.workers.drain(..) {
            let _ = handle.join();
        }
    }
}

fn worker_loop(index: usize, shared: Arc<JobShared>) {
    loop {
        if let Some(job) = shared.pop_local_or_steal(index) {
            job.run(&shared);
            shared.pending.fetch_sub(1, Ordering::AcqRel);
            continue;
        }

        if shared.shutdown.load(Ordering::Acquire) && shared.pending.load(Ordering::Acquire) == 0 {
            break;
        }

        shared.wait_for_work_or_shutdown();
    }
}
