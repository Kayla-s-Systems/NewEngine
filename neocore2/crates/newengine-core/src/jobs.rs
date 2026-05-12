#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::collections_prelude::NeVecDeque as VecDeque;
use parking_lot::Mutex;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex as StdMutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

type JobFn = Box<dyn FnOnce() + Send + 'static>;

pub const JOB_LANE_COUNT: usize = 6;
pub const JOB_PRIORITY_COUNT: usize = 4;

/// Stable work lane used by engine systems when they submit CPU work.
///
/// This is intentionally a contract, not just telemetry. The scheduler can
/// protect frame-critical lanes from bulk streaming/background work without each
/// module inventing its own thread pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum JobLane {
    Simulation,
    RenderPrep,
    Streaming,
    AssetIo,
    Plugin,
    Background,
}

impl JobLane {
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

impl Default for JobLane {
    #[inline]
    fn default() -> Self {
        Self::Simulation
    }
}

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

    #[inline]
    pub const fn fixed(worker_threads: usize) -> Self {
        Self { worker_threads }
    }
}

impl Default for JobSystemConfig {
    #[inline]
    fn default() -> Self {
        Self::auto()
    }
}

/// Stable job priority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum JobPriority {
    Background,
    #[default]
    Normal,
    Interactive,
    Critical,
}

impl JobPriority {
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
}

/// Engine-standard task envelope used by systems that submit CPU work.
#[derive(Clone, Debug)]
pub struct JobRequest {
    pub label: &'static str,
    pub lane: JobLane,
    pub priority: JobPriority,
}

impl JobRequest {
    #[inline]
    pub const fn new(label: &'static str) -> Self {
        Self {
            label,
            lane: JobLane::Simulation,
            priority: JobPriority::Normal,
        }
    }

    #[inline]
    pub const fn with_lane(mut self, lane: JobLane) -> Self {
        self.lane = lane;
        self
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
    pub pending_by_lane: [usize; JOB_LANE_COUNT],
    pub completed_by_lane: [u64; JOB_LANE_COUNT],
}

impl JobSystemSnapshot {
    #[inline]
    pub fn pending_for_lane(&self, lane: JobLane) -> usize {
        self.pending_by_lane[lane.index()]
    }

    #[inline]
    pub fn completed_for_lane(&self, lane: JobLane) -> u64 {
        self.completed_by_lane[lane.index()]
    }
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
        let lane_index = self.request.lane.index();
        let Some(job) = self.job.take() else {
            self.completion.complete();
            shared.completed.fetch_add(1, Ordering::AcqRel);
            shared.completed_by_lane[lane_index].fetch_add(1, Ordering::AcqRel);
            return;
        };

        let result = catch_unwind(AssertUnwindSafe(move || {
            job();
        }));
        self.completion.complete();
        shared.completed.fetch_add(1, Ordering::AcqRel);
        shared.completed_by_lane[lane_index].fetch_add(1, Ordering::AcqRel);

        if result.is_err() {
            shared.panicked.fetch_add(1, Ordering::AcqRel);
            log::error!(
                "job-system: worker job panicked label='{}' lane='{}' priority={:?}; worker recovered and continues",
                self.request.label,
                self.request.lane.as_str(),
                self.request.priority
            );
        }
    }
}

struct JobShared {
    queues: Vec<Mutex<VecDeque<QueuedJob>>>,
    pending_by_lane: Vec<AtomicUsize>,
    completed_by_lane: Vec<AtomicU64>,
    worker_threads: usize,
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
        let queue_count = JOB_LANE_COUNT * JOB_PRIORITY_COUNT;
        Self {
            queues: (0..queue_count)
                .map(|_| Mutex::new(VecDeque::new()))
                .collect(),
            pending_by_lane: (0..JOB_LANE_COUNT).map(|_| AtomicUsize::new(0)).collect(),
            completed_by_lane: (0..JOB_LANE_COUNT).map(|_| AtomicU64::new(0)).collect(),
            worker_threads,
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
        self.worker_threads.max(1)
    }

    #[inline]
    fn queue_index(lane: JobLane, priority: JobPriority) -> usize {
        priority.index() * JOB_LANE_COUNT + lane.index()
    }

    fn submit(&self, job: QueuedJob) {
        if self.shutdown.load(Ordering::Acquire) {
            job.completion.complete();
            return;
        }

        let lane_index = job.request.lane.index();
        let queue_index = Self::queue_index(job.request.lane, job.request.priority);

        self.submitted.fetch_add(1, Ordering::AcqRel);
        self.pending.fetch_add(1, Ordering::Release);
        self.pending_by_lane[lane_index].fetch_add(1, Ordering::Release);
        self.queues[queue_index].lock().push_back(job);
        self.sleep_wake.notify_one();
    }

    fn pop_next(&self) -> Option<QueuedJob> {
        for priority in JobPriority::service_order() {
            for lane in JobLane::all() {
                let idx = Self::queue_index(lane, priority);
                if let Some(job) = self.queues[idx].lock().pop_front() {
                    self.pending_by_lane[job.request.lane.index()].fetch_sub(1, Ordering::AcqRel);
                    return Some(job);
                }
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

    fn snapshot(&self) -> JobSystemSnapshot {
        let mut pending_by_lane = [0usize; JOB_LANE_COUNT];
        let mut completed_by_lane = [0u64; JOB_LANE_COUNT];

        for lane in JobLane::all() {
            pending_by_lane[lane.index()] = self.pending_by_lane[lane.index()].load(Ordering::Acquire);
            completed_by_lane[lane.index()] = self.completed_by_lane[lane.index()].load(Ordering::Acquire);
        }

        JobSystemSnapshot {
            worker_threads: self.worker_count(),
            pending_jobs: self.pending.load(Ordering::Acquire),
            submitted_jobs: self.submitted.load(Ordering::Acquire),
            completed_jobs: self.completed.load(Ordering::Acquire),
            panicked_jobs: self.panicked.load(Ordering::Acquire),
            pending_by_lane,
            completed_by_lane,
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

    pub fn submit_lane<F>(&self, lane: JobLane, label: &'static str, f: F) -> JobTicket
    where
        F: FnOnce() + Send + 'static,
    {
        self.submit_request(JobRequest::new(label).with_lane(lane), f)
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
        self.run_indexed_request(len, JobRequest::new("indexed"), f)
    }

    /// Runs indexed jobs through an explicit lane/priority envelope.
    pub fn run_indexed_request<T, F>(&self, len: usize, request: JobRequest, f: F) -> Vec<T>
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
            tickets.push(self.submit_request(request.clone(), move || {
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
    pub fn pending_for_lane(&self, lane: JobLane) -> usize {
        self.shared.pending_by_lane[lane.index()].load(Ordering::Acquire)
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

/// Persistent CPU job system with stable lanes and priority-aware queues.
///
/// GPU work stays on the render thread. CPU-heavy work such as terrain generation,
/// component preparation, asset preprocessing and plugin maintenance goes through
/// this standard pipeline.
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
                .spawn(move || worker_loop(worker_shared))
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
    pub fn pending_for_lane(&self, lane: JobLane) -> usize {
        self.handle.pending_for_lane(lane)
    }

    #[inline]
    pub fn snapshot(&self) -> JobSystemSnapshot {
        self.handle.snapshot()
    }

    /// Requests cooperative stop and joins all engine-owned worker threads.
    ///
    /// This must run before plugin DLL/service teardown so queued jobs cannot execute
    /// against unloaded plugin-owned state during shutdown.
    pub fn shutdown_and_join(&mut self) {
        self.handle.shared.shutdown.store(true, Ordering::Release);
        self.handle.shared.sleep_wake.notify_all();

        for handle in self.workers.drain(..) {
            let _ = handle.join();
        }
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

    pub fn submit_lane<F>(&self, lane: JobLane, label: &'static str, f: F) -> JobTicket
    where
        F: FnOnce() + Send + 'static,
    {
        self.handle.submit_lane(lane, label, f)
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

    #[inline]
    pub fn run_indexed_request<T, F>(&self, len: usize, request: JobRequest, f: F) -> Vec<T>
    where
        T: Send + 'static,
        F: Fn(usize) -> T + Send + Sync + 'static,
    {
        self.handle.run_indexed_request(len, request, f)
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
        self.shutdown_and_join();
    }
}

fn worker_loop(shared: Arc<JobShared>) {
    loop {
        if let Some(job) = shared.pop_next() {
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
