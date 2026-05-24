#![forbid(unsafe_op_in_unsafe_fn)]

use crate::events::EventHub;
use newengine_loading_api::{
    EngineTaskControlAction, EngineTaskControlEvent, EngineTaskEvent, EngineTaskPhase,
    ENGINE_TASK_EVENT_TOPIC_V1,
};
use newengine_math::collections_prelude::{NeHashMap as HashMap, NeVecDeque as VecDeque};
use parking_lot::Mutex;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex as StdMutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

type JobFn = Box<dyn FnOnce(JobControl) + Send + 'static>;

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

/// Engine-standard task envelope used by systems that submit CPU work.
#[derive(Clone, Debug)]
pub struct JobRequest {
    pub label: &'static str,
    pub lane: JobLane,
    pub priority: JobPriority,
    pub task_id: Option<String>,
    pub parent_task_id: Option<String>,
    pub can_pause: bool,
    pub can_cancel: bool,
}

impl JobRequest {
    #[inline]
    pub const fn new(label: &'static str) -> Self {
        Self {
            label,
            lane: JobLane::Simulation,
            priority: JobPriority::Normal,
            task_id: None,
            parent_task_id: None,
            can_pause: false,
            can_cancel: true,
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

    #[inline]
    pub fn with_task_id(mut self, task_id: impl Into<String>) -> Self {
        self.task_id = Some(task_id.into());
        self
    }

    #[inline]
    pub fn with_parent_task_id(mut self, parent_task_id: impl Into<String>) -> Self {
        self.parent_task_id = Some(parent_task_id.into());
        self
    }

    #[inline]
    pub const fn pausable(mut self, can_pause: bool) -> Self {
        self.can_pause = can_pause;
        self
    }

    #[inline]
    pub const fn cancellable(mut self, can_cancel: bool) -> Self {
        self.can_cancel = can_cancel;
        self
    }
}

/// Lightweight job-system snapshot for profiling and UI provider read-models.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct JobSystemSnapshot {
    pub worker_threads: usize,
    pub pending_jobs: usize,
    pub running_jobs: usize,
    pub paused_jobs: usize,
    pub submitted_jobs: u64,
    pub completed_jobs: u64,
    pub cancelled_jobs: u64,
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

#[derive(Clone, Debug)]
pub struct JobTaskStatus {
    pub task_id: String,
    pub label: &'static str,
    pub lane: JobLane,
    pub priority: JobPriority,
    pub phase: EngineTaskPhase,
    pub can_pause: bool,
    pub can_cancel: bool,
    pub cancel_requested: bool,
    pub pause_requested: bool,
}

/// Cooperative task control token.
///
/// Long-running jobs should periodically call `checkpoint()` or
/// `wait_while_paused()` to honor engine-bus pause/cancel requests. Short jobs
/// are still tracked and cancellable before they begin execution.
#[derive(Clone)]
pub struct JobControl {
    inner: Arc<JobControlInner>,
}

struct JobControlInner {
    task_id: String,
    parent_task_id: Option<String>,
    label: &'static str,
    lane: JobLane,
    priority: JobPriority,
    can_pause: bool,
    can_cancel: bool,
    cancel_requested: AtomicBool,
    pause_requested: AtomicBool,
    phase: Mutex<EngineTaskPhase>,
    events: Option<EventHub>,
    pause_lock: StdMutex<()>,
    pause_wake: Condvar,
}

impl JobControl {
    fn new(task_id: String, request: &JobRequest, events: Option<EventHub>) -> Self {
        Self {
            inner: Arc::new(JobControlInner {
                task_id,
                parent_task_id: request.parent_task_id.clone(),
                label: request.label,
                lane: request.lane,
                priority: request.priority,
                can_pause: request.can_pause,
                can_cancel: request.can_cancel,
                cancel_requested: AtomicBool::new(false),
                pause_requested: AtomicBool::new(false),
                phase: Mutex::new(EngineTaskPhase::Scheduled),
                events,
                pause_lock: StdMutex::new(()),
                pause_wake: Condvar::new(),
            }),
        }
    }

    #[inline]
    pub fn task_id(&self) -> &str {
        self.inner.task_id.as_str()
    }

    #[inline]
    pub fn is_cancel_requested(&self) -> bool {
        self.inner.cancel_requested.load(Ordering::Acquire)
    }

    #[inline]
    pub fn is_pause_requested(&self) -> bool {
        self.inner.pause_requested.load(Ordering::Acquire)
    }

    #[inline]
    pub fn status(&self) -> JobTaskStatus {
        JobTaskStatus {
            task_id: self.inner.task_id.clone(),
            label: self.inner.label,
            lane: self.inner.lane,
            priority: self.inner.priority,
            phase: *self.inner.phase.lock(),
            can_pause: self.inner.can_pause,
            can_cancel: self.inner.can_cancel,
            cancel_requested: self.is_cancel_requested(),
            pause_requested: self.is_pause_requested(),
        }
    }

    pub fn request_cancel(&self) -> bool {
        if !self.inner.can_cancel {
            return false;
        }
        self.inner.cancel_requested.store(true, Ordering::Release);
        self.publish(EngineTaskPhase::CancelRequested, "Cancel requested", "Task cancellation was requested through engine task control.", None);
        self.inner.pause_wake.notify_all();
        true
    }

    pub fn request_pause(&self) -> bool {
        if !self.inner.can_pause {
            return false;
        }
        self.inner.pause_requested.store(true, Ordering::Release);
        self.publish(EngineTaskPhase::PauseRequested, "Pause requested", "Task pause was requested through engine task control.", None);
        true
    }

    pub fn resume(&self) -> bool {
        if !self.inner.can_pause {
            return false;
        }
        self.inner.pause_requested.store(false, Ordering::Release);
        self.publish(EngineTaskPhase::ResumeRequested, "Resume requested", "Task resume was requested through engine task control.", None);
        self.inner.pause_wake.notify_all();
        true
    }

    /// Waits while pause is requested and returns `false` when cancellation wins.
    pub fn wait_while_paused(&self) -> bool {
        if !self.inner.can_pause {
            return !self.is_cancel_requested();
        }

        if !self.is_pause_requested() {
            return !self.is_cancel_requested();
        }

        self.publish(EngineTaskPhase::Paused, "Task paused", "Task is paused at a cooperative checkpoint.", None);
        let mut guard = self.inner.pause_lock.lock().unwrap_or_else(|e| e.into_inner());
        while self.is_pause_requested() && !self.is_cancel_requested() {
            guard = self.inner.pause_wake.wait(guard).unwrap_or_else(|e| e.into_inner());
        }
        !self.is_cancel_requested()
    }

    #[inline]
    pub fn checkpoint(&self) -> bool {
        if self.is_cancel_requested() {
            return false;
        }
        self.wait_while_paused()
    }

    pub fn publish_progress(
        &self,
        progress_01: f32,
        status: impl Into<String>,
        detail: impl Into<String>,
    ) {
        self.publish(EngineTaskPhase::Running, status, detail, Some(progress_01));
    }

    fn publish(
        &self,
        phase: EngineTaskPhase,
        status: impl Into<String>,
        detail: impl Into<String>,
        progress_01: Option<f32>,
    ) {
        {
            let mut current = self.inner.phase.lock();
            *current = phase;
        }

        let mut event = EngineTaskEvent::new(
            self.inner.task_id.clone(),
            "newengine-core.job-system",
            "newengine-core",
            "cpu-job",
            self.inner.label,
            self.inner.lane.as_str(),
            phase,
            status.into(),
            detail.into(),
        )
        .with_controls(self.inner.can_pause, self.inner.can_cancel);

        if let Some(parent) = self.inner.parent_task_id.as_ref() {
            event = event.with_parent_task_id(parent.clone());
        }
        if let Some(progress) = progress_01 {
            event = event.with_progress(progress);
        }

        publish_task_event(self.inner.events.as_ref(), event);
    }
}

/// Wait handle for a submitted CPU job.
pub struct JobTicket {
    completion: Arc<JobCompletion>,
    control: JobControl,
}

impl JobTicket {
    #[inline]
    pub fn is_complete(&self) -> bool {
        self.completion.is_complete()
    }

    #[inline]
    pub fn task_id(&self) -> &str {
        self.control.task_id()
    }

    #[inline]
    pub fn control(&self) -> JobControl {
        self.control.clone()
    }

    #[inline]
    pub fn status(&self) -> JobTaskStatus {
        self.control.status()
    }

    #[inline]
    pub fn cancel(&self) -> bool {
        self.control.request_cancel()
    }

    #[inline]
    pub fn pause(&self) -> bool {
        self.control.request_pause()
    }

    #[inline]
    pub fn resume(&self) -> bool {
        self.control.resume()
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
    control: JobControl,
}

impl QueuedJob {
    fn run(mut self, shared: &JobShared) {
        let lane_index = self.request.lane.index();
        let Some(job) = self.job.take() else {
            self.completion.complete();
            shared.completed.fetch_add(1, Ordering::AcqRel);
            shared.completed_by_lane[lane_index].fetch_add(1, Ordering::AcqRel);
            self.control.publish(EngineTaskPhase::Completed, "Task completed", "Task completed without a job closure.", Some(1.0));
            return;
        };

        if self.control.is_cancel_requested() {
            shared.cancelled.fetch_add(1, Ordering::AcqRel);
            self.completion.complete();
            self.control.publish(EngineTaskPhase::Cancelled, "Task cancelled", "Task was cancelled before worker execution.", Some(1.0));
            return;
        }

        shared.running.fetch_add(1, Ordering::AcqRel);
        self.control.publish(EngineTaskPhase::Running, "Task running", "Worker picked up the task from the engine queue.", None);
        if !self.control.wait_while_paused() {
            shared.running.fetch_sub(1, Ordering::AcqRel);
            shared.cancelled.fetch_add(1, Ordering::AcqRel);
            self.completion.complete();
            self.control.publish(EngineTaskPhase::Cancelled, "Task cancelled", "Task was cancelled while paused before execution.", Some(1.0));
            return;
        }

        let control = self.control.clone();
        let result = catch_unwind(AssertUnwindSafe(move || {
            job(control);
        }));
        shared.running.fetch_sub(1, Ordering::AcqRel);
        self.completion.complete();
        shared.completed.fetch_add(1, Ordering::AcqRel);
        shared.completed_by_lane[lane_index].fetch_add(1, Ordering::AcqRel);

        if result.is_err() {
            shared.panicked.fetch_add(1, Ordering::AcqRel);
            self.control.publish(EngineTaskPhase::Failed, "Task failed", "Worker job panicked; worker recovered and continues.", Some(1.0));
            log::error!(
                "job-system: worker job panicked label='{}' lane='{}' priority={:?}; worker recovered and continues",
                self.request.label,
                self.request.lane.as_str(),
                self.request.priority
            );
        } else if self.control.is_cancel_requested() {
            shared.cancelled.fetch_add(1, Ordering::AcqRel);
            self.control.publish(EngineTaskPhase::Cancelled, "Task cancelled", "Task completed after observing cancellation.", Some(1.0));
        } else {
            self.control.publish(EngineTaskPhase::Completed, "Task completed", "Task finished on engine-owned worker thread.", Some(1.0));
        }
    }
}

struct JobShared {
    queues: Vec<Mutex<VecDeque<QueuedJob>>>,
    pending_by_lane: Vec<AtomicUsize>,
    completed_by_lane: Vec<AtomicU64>,
    worker_threads: usize,
    pending: AtomicUsize,
    running: AtomicUsize,
    paused: AtomicUsize,
    submitted: AtomicU64,
    completed: AtomicU64,
    cancelled: AtomicU64,
    panicked: AtomicU64,
    next_task_id: AtomicU64,
    shutdown: AtomicBool,
    events: Option<EventHub>,
    tasks: Mutex<HashMap<String, JobControl>>,
    sleep_lock: StdMutex<()>,
    sleep_wake: Condvar,
}

impl JobShared {
    fn new(worker_threads: usize, events: Option<EventHub>) -> Self {
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
            running: AtomicUsize::new(0),
            paused: AtomicUsize::new(0),
            submitted: AtomicU64::new(0),
            completed: AtomicU64::new(0),
            cancelled: AtomicU64::new(0),
            panicked: AtomicU64::new(0),
            next_task_id: AtomicU64::new(1),
            shutdown: AtomicBool::new(false),
            events,
            tasks: Mutex::new(HashMap::default()),
            sleep_lock: StdMutex::new(()),
            sleep_wake: Condvar::new(),
        }
    }

    #[inline]
    fn worker_count(&self) -> usize {
        self.worker_threads.max(1)
    }

    #[inline]
    fn next_task_id(&self) -> String {
        format!("engine.job.{}", self.next_task_id.fetch_add(1, Ordering::Relaxed))
    }

    #[inline]
    fn queue_index(lane: JobLane, priority: JobPriority) -> usize {
        priority.index() * JOB_LANE_COUNT + lane.index()
    }

    fn submit(&self, job: QueuedJob) {
        if self.shutdown.load(Ordering::Acquire) {
            job.control.publish(EngineTaskPhase::Cancelled, "Task rejected", "Job system is shutting down; task was not queued.", Some(1.0));
            job.completion.complete();
            return;
        }

        let lane_index = job.request.lane.index();
        let queue_index = Self::queue_index(job.request.lane, job.request.priority);

        self.submitted.fetch_add(1, Ordering::AcqRel);
        self.pending.fetch_add(1, Ordering::Release);
        self.pending_by_lane[lane_index].fetch_add(1, Ordering::Release);
        job.control.publish(EngineTaskPhase::Scheduled, "Task scheduled", "Task was registered in the engine job queue.", Some(0.0));
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
            running_jobs: self.running.load(Ordering::Acquire),
            paused_jobs: self.paused.load(Ordering::Acquire),
            submitted_jobs: self.submitted.load(Ordering::Acquire),
            completed_jobs: self.completed.load(Ordering::Acquire),
            cancelled_jobs: self.cancelled.load(Ordering::Acquire),
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
        self.submit_controlled(request, move |_control| f())
    }

    pub fn submit_controlled<F>(&self, request: JobRequest, f: F) -> JobTicket
    where
        F: FnOnce(JobControl) + Send + 'static,
    {
        let completion = Arc::new(JobCompletion::new());
        let task_id = request.task_id.clone().unwrap_or_else(|| self.shared.next_task_id());
        let control = JobControl::new(task_id, &request, self.shared.events.clone());
        let ticket = JobTicket {
            completion: Arc::clone(&completion),
            control: control.clone(),
        };
        self.shared.tasks.lock().insert(control.task_id().to_owned(), control.clone());
        self.shared.submit(QueuedJob {
            request,
            job: Some(Box::new(f)),
            completion,
            control,
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
            let request = request.clone().with_task_id(format!("indexed.{}.{}", request.label, index));
            tickets.push(self.submit_controlled(request, move |control| {
                if !control.checkpoint() {
                    return;
                }
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

    pub fn task_status(&self, task_id: &str) -> Option<JobTaskStatus> {
        self.shared.tasks.lock().get(task_id).map(JobControl::status)
    }

    pub fn cancel_task(&self, task_id: &str) -> bool {
        self.shared
            .tasks
            .lock()
            .get(task_id)
            .map(JobControl::request_cancel)
            .unwrap_or(false)
    }

    pub fn pause_task(&self, task_id: &str) -> bool {
        self.shared
            .tasks
            .lock()
            .get(task_id)
            .map(JobControl::request_pause)
            .unwrap_or(false)
    }

    pub fn resume_task(&self, task_id: &str) -> bool {
        self.shared
            .tasks
            .lock()
            .get(task_id)
            .map(JobControl::resume)
            .unwrap_or(false)
    }

    pub fn apply_control_event(&self, event: &EngineTaskControlEvent) -> bool {
        match event.action {
            EngineTaskControlAction::Pause => self.pause_task(event.task_id.as_str()),
            EngineTaskControlAction::Resume => self.resume_task(event.task_id.as_str()),
            EngineTaskControlAction::Cancel => self.cancel_task(event.task_id.as_str()),
        }
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
        Self::new_with_events(config, None)
    }

    pub fn new_with_event_hub(config: JobSystemConfig, events: EventHub) -> Self {
        Self::new_with_events(config, Some(events))
    }

    fn new_with_events(config: JobSystemConfig, events: Option<EventHub>) -> Self {
        let worker_threads = config.worker_threads.max(1);
        let shared = Arc::new(JobShared::new(worker_threads, events));
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

    #[inline]
    pub fn task_status(&self, task_id: &str) -> Option<JobTaskStatus> {
        self.handle.task_status(task_id)
    }

    #[inline]
    pub fn cancel_task(&self, task_id: &str) -> bool {
        self.handle.cancel_task(task_id)
    }

    #[inline]
    pub fn pause_task(&self, task_id: &str) -> bool {
        self.handle.pause_task(task_id)
    }

    #[inline]
    pub fn resume_task(&self, task_id: &str) -> bool {
        self.handle.resume_task(task_id)
    }

    #[inline]
    pub fn apply_control_event(&self, event: &EngineTaskControlEvent) -> bool {
        self.handle.apply_control_event(event)
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

    pub fn submit_controlled<F>(&self, request: JobRequest, f: F) -> JobTicket
    where
        F: FnOnce(JobControl) + Send + 'static,
    {
        self.handle.submit_controlled(request, f)
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

fn publish_task_event(events: Option<&EventHub>, event: EngineTaskEvent) {
    if let Some(events) = events {
        let _ = events.publish(event.clone());
    }
    if let Ok(bytes) = serde_json::to_vec(&event) {
        let _ = newengine_plugin_host::emit_plugin_event(ENGINE_TASK_EVENT_TOPIC_V1, &bytes);
    }
}
