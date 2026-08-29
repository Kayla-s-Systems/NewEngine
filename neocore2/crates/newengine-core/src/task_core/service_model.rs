use crate::events::EventHub;
use newengine_loading_api::{EngineTaskControlAction, EngineTaskControlEvent};
use parking_lot::Mutex;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use super::config::{TaskLane, ThreadPoolCoreConfig};
use super::control::{CoreTaskControl, CoreTaskTicket, TaskCompletion};
use super::queue::{QueuedTask, TaskCoreShared};
use super::request::TaskRequest;
use super::status::{CoreTaskRuntimeStatus, ThreadPoolCoreSnapshot};
use super::worker::worker_loop;

/// Private backing endpoint for `engine.threading`.
///
/// This type is intentionally not exported from `newengine-core`; runtime modules
/// consume `ThreadPoolHandle` instead.
#[derive(Clone)]
pub struct ThreadPoolCoreHandle {
    shared: Arc<TaskCoreShared>,
}

impl ThreadPoolCoreHandle {
    pub fn submit_named<F>(&self, label: &'static str, f: F) -> CoreTaskTicket
    where
        F: FnOnce() + Send + 'static,
    {
        self.submit_request(TaskRequest::new(label), f)
    }

    pub fn submit_lane<F>(&self, lane: TaskLane, label: &'static str, f: F) -> CoreTaskTicket
    where
        F: FnOnce() + Send + 'static,
    {
        self.submit_request(TaskRequest::new(label).with_lane(lane), f)
    }

    pub fn submit_request<F>(&self, request: TaskRequest, f: F) -> CoreTaskTicket
    where
        F: FnOnce() + Send + 'static,
    {
        self.submit_controlled(request, move |_control| f())
    }

    pub fn submit_controlled<F>(&self, request: TaskRequest, f: F) -> CoreTaskTicket
    where
        F: FnOnce(CoreTaskControl) + Send + 'static,
    {
        let completion = Arc::new(TaskCompletion::new());
        let task_id = request
            .task_id
            .clone()
            .unwrap_or_else(|| self.shared.next_task_id());
        let control = CoreTaskControl::new(task_id, &request, self.shared.events.clone());
        let ticket = CoreTaskTicket {
            completion: Arc::clone(&completion),
            control: control.clone(),
            shared: Arc::clone(&self.shared),
        };
        self.shared
            .tasks
            .lock()
            .insert(control.task_id().to_owned(), control.clone());
        self.shared
            .completions
            .lock()
            .insert(control.task_id().to_owned(), Arc::clone(&completion));
        self.shared
            .register_task_hierarchy(control.task_id(), request.parent_task_id.as_deref());
        self.shared.submit(QueuedTask {
            request,
            job: Some(Box::new(f)),
            control,
        });
        ticket
    }

    /// Runs indexed jobs through an explicit lane/priority envelope.
    pub fn run_indexed_request<T, F>(&self, len: usize, request: TaskRequest, f: F) -> Vec<T>
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
            let request = request
                .clone()
                .with_task_id(format!("indexed.{}.{}", request.label, index));
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
            .map(|value| value.expect("thread-pool: indexed task completed without result"))
            .collect()
    }

    #[inline]
    pub fn pending_jobs(&self) -> usize {
        self.shared.pending.load(Ordering::Acquire)
    }

    #[inline]
    pub fn pending_for_lane(&self, lane: TaskLane) -> usize {
        self.shared.pending_by_lane[lane.index()].load(Ordering::Acquire)
    }

    #[inline]
    pub fn worker_threads(&self) -> usize {
        self.shared.worker_count()
    }

    #[inline]
    pub fn snapshot(&self) -> ThreadPoolCoreSnapshot {
        self.shared.snapshot()
    }

    #[inline]
    pub fn set_frame_cpu_budget(&self, budget: Duration) {
        self.shared.set_frame_cpu_budget(budget);
    }

    #[inline]
    pub fn begin_frame_budget(&self, budget: Duration) {
        self.shared.begin_frame_budget(budget);
    }

    pub fn task_status(&self, task_id: &str) -> Option<CoreTaskRuntimeStatus> {
        self.shared
            .tasks
            .lock()
            .get(task_id)
            .map(CoreTaskControl::status)
    }

    pub fn cancel_task(&self, task_id: &str) -> bool {
        self.shared
            .tasks
            .lock()
            .get(task_id)
            .map(CoreTaskControl::request_cancel)
            .unwrap_or(false)
    }

    pub fn pause_task(&self, task_id: &str) -> bool {
        self.shared
            .tasks
            .lock()
            .get(task_id)
            .map(CoreTaskControl::request_pause)
            .unwrap_or(false)
    }

    pub fn resume_task(&self, task_id: &str) -> bool {
        self.shared
            .tasks
            .lock()
            .get(task_id)
            .map(CoreTaskControl::resume)
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

/// Persistent CPU worker system backing `ThreadPoolManager`.
pub struct ThreadPoolCore {
    handle: ThreadPoolCoreHandle,
    workers: Vec<JoinHandle<()>>,
}

impl ThreadPoolCore {
    pub fn new(config: ThreadPoolCoreConfig) -> Self {
        Self::new_with_events(config, None)
    }

    pub fn new_with_event_hub(config: ThreadPoolCoreConfig, events: EventHub) -> Self {
        Self::new_with_events(config, Some(events))
    }

    fn new_with_events(config: ThreadPoolCoreConfig, events: Option<EventHub>) -> Self {
        let worker_threads = config.worker_threads.max(1);
        let frame_cpu_budget = Duration::from_millis(u64::from(config.frame_cpu_budget_ms));
        let shared = Arc::new(TaskCoreShared::new(
            worker_threads,
            frame_cpu_budget,
            events,
        ));
        let handle = ThreadPoolCoreHandle {
            shared: Arc::clone(&shared),
        };
        let mut workers = Vec::with_capacity(worker_threads);

        for index in 0..worker_threads {
            let worker_shared = Arc::clone(&shared);
            let name = format!("newengine-threadpool-{index}");
            // no-hidden-thread-scan: allowed engine.threading executor executor; every queued closure receives a task id and lifecycle events.
            let handle = thread::Builder::new()
                .name(name)
                .spawn(move || worker_loop(worker_shared))
                .expect("thread-pool: failed to spawn worker thread");
            workers.push(handle);
        }

        Self { handle, workers }
    }

    #[inline]
    pub fn handle(&self) -> ThreadPoolCoreHandle {
        self.handle.clone()
    }

    #[inline]
    pub fn worker_threads(&self) -> usize {
        self.handle.worker_threads()
    }

    #[inline]
    pub fn snapshot(&self) -> ThreadPoolCoreSnapshot {
        self.handle.snapshot()
    }

    #[inline]
    pub fn set_frame_cpu_budget(&self, budget: Duration) {
        self.handle.set_frame_cpu_budget(budget);
    }

    #[inline]
    pub fn begin_frame_budget(&self, budget: Duration) {
        self.handle.begin_frame_budget(budget);
    }

    /// Requests cooperative stop and joins all engine-runtime worker threads.
    pub fn shutdown_and_join(&mut self) {
        self.handle.shared.shutdown.store(true, Ordering::Release);
        self.handle.shared.sleep_wake.notify_all();

        for handle in self.workers.drain(..) {
            let _ = handle.join();
        }
    }
}

impl Default for ThreadPoolCore {
    #[inline]
    fn default() -> Self {
        Self::new(ThreadPoolCoreConfig::auto())
    }
}

impl Drop for ThreadPoolCore {
    fn drop(&mut self) {
        self.shutdown_and_join();
    }
}
