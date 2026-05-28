use crate::events::EventHub;
use newengine_loading_api::{EngineTaskControlAction, EngineTaskControlEvent};
use parking_lot::Mutex;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use super::config::{JobLane, JobSystemConfig};
use super::control::{JobCompletion, JobControl, JobTicket};
use super::queue::{JobShared, QueuedJob};
use super::request::JobRequest;
use super::status::{JobSystemSnapshot, JobTaskStatus};
use super::worker::worker_loop;

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
            // no-hidden-thread-scan: allowed engine.jobs worker executor; every queued closure receives a JobId and lifecycle events.
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

    /// Requests cooperative stop and joins all engine-runtime worker threads.
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
