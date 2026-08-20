use crate::events::EventHub;
use newengine_loading_api::{EngineTaskEvent, EngineTaskPhase};
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex as StdMutex};

use super::config::{TaskLane, TaskPriority};
use super::events::publish_task_event;
use super::queue::TaskCoreShared;
use super::request::TaskRequest;
use super::status::CoreTaskRuntimeStatus;

/// Cooperative task control token.
///
/// Long-running jobs should periodically call `checkpoint()` or
/// `wait_while_paused()` to honor engine-bus pause/cancel requests. Short jobs
/// are still tracked and cancellable before they begin execution.
#[derive(Clone)]
pub struct CoreTaskControl {
    inner: Arc<CoreTaskControlInner>,
}

struct CoreTaskControlInner {
    task_id: String,
    parent_task_id: Option<String>,
    label: &'static str,
    source: &'static str,
    owner: &'static str,
    category: &'static str,
    lane: TaskLane,
    priority: TaskPriority,
    frame_id: Option<u64>,
    dependency_group: Option<String>,
    task_domain: &'static str,
    task_pass: &'static str,
    can_pause: bool,
    can_cancel: bool,
    cancel_requested: AtomicBool,
    pause_requested: AtomicBool,
    phase: Mutex<EngineTaskPhase>,
    events: Option<EventHub>,
    pause_lock: StdMutex<()>,
    pause_wake: Condvar,
}

impl CoreTaskControl {
    pub(super) fn new(task_id: String, request: &TaskRequest, events: Option<EventHub>) -> Self {
        Self {
            inner: Arc::new(CoreTaskControlInner {
                task_id,
                parent_task_id: request.parent_task_id.clone(),
                label: request.label,
                source: request.source,
                owner: request.owner,
                category: request.category,
                lane: request.lane,
                priority: request.priority,
                frame_id: request.frame_id,
                dependency_group: request.dependency_group.clone(),
                task_domain: request.task_domain,
                task_pass: request.task_pass,
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
    pub fn status(&self) -> CoreTaskRuntimeStatus {
        CoreTaskRuntimeStatus {
            task_id: self.inner.task_id.clone(),
            label: self.inner.label,
            lane: self.inner.lane,
            priority: self.inner.priority,
            frame_id: self.inner.frame_id,
            dependency_group: self.inner.dependency_group.clone(),
            task_domain: self.inner.task_domain,
            task_pass: self.inner.task_pass,
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
        self.publish(
            EngineTaskPhase::CancelRequested,
            "Cancel requested",
            "Task cancellation was requested through engine task control.",
            None,
        );
        self.inner.pause_wake.notify_all();
        true
    }

    pub fn request_pause(&self) -> bool {
        if !self.inner.can_pause {
            return false;
        }
        self.inner.pause_requested.store(true, Ordering::Release);
        self.publish(
            EngineTaskPhase::PauseRequested,
            "Pause requested",
            "Task pause was requested through engine task control.",
            None,
        );
        true
    }

    pub fn resume(&self) -> bool {
        if !self.inner.can_pause {
            return false;
        }
        self.inner.pause_requested.store(false, Ordering::Release);
        self.publish(
            EngineTaskPhase::ResumeRequested,
            "Resume requested",
            "Task resume was requested through engine task control.",
            None,
        );
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

        self.publish(
            EngineTaskPhase::Paused,
            "Task paused",
            "Task is paused at a cooperative checkpoint.",
            None,
        );
        let mut guard = self
            .inner
            .pause_lock
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        while self.is_pause_requested() && !self.is_cancel_requested() {
            guard = self
                .inner
                .pause_wake
                .wait(guard)
                .unwrap_or_else(|e| e.into_inner());
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

    pub(super) fn publish(
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
            self.inner.source,
            self.inner.owner,
            self.inner.category,
            self.inner.label,
            self.inner.lane.as_str(),
            phase,
            status.into(),
            detail.into(),
        )
        .with_controls(self.inner.can_pause, self.inner.can_cancel)
        .with_task_domain(self.inner.task_domain)
        .with_task_pass(self.inner.task_pass)
        .with_priority(self.inner.priority.as_str())
        .with_executor("engine-worker");

        if let Some(frame_id) = self.inner.frame_id {
            event = event.with_frame_id(frame_id);
        }
        if let Some(group) = self.inner.dependency_group.as_ref() {
            event = event.with_dependency_group(group.clone());
        }
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
pub struct CoreTaskTicket {
    pub(super) completion: Arc<TaskCompletion>,
    pub(super) control: CoreTaskControl,
    pub(super) shared: Arc<TaskCoreShared>,
}

impl CoreTaskTicket {
    #[inline]
    pub fn is_complete(&self) -> bool {
        self.completion.is_complete()
    }

    #[inline]
    pub fn task_id(&self) -> &str {
        self.control.task_id()
    }

    #[inline]
    pub fn status(&self) -> CoreTaskRuntimeStatus {
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
        self.shared.wait_for_completion(self.completion.as_ref());
    }
}
pub(super) struct TaskCompletion {
    done: AtomicBool,
    lock: StdMutex<()>,
    wake: Condvar,
}

impl TaskCompletion {
    #[inline]
    pub(super) fn new() -> Self {
        Self {
            done: AtomicBool::new(false),
            lock: StdMutex::new(()),
            wake: Condvar::new(),
        }
    }

    #[inline]
    pub(super) fn is_complete(&self) -> bool {
        self.done.load(Ordering::Acquire)
    }

    pub(super) fn wait(&self) {
        if self.is_complete() {
            return;
        }

        let mut guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        while !self.is_complete() {
            guard = self.wake.wait(guard).unwrap_or_else(|e| e.into_inner());
        }
    }

    pub(super) fn wait_timeout(&self, timeout: std::time::Duration) {
        if self.is_complete() {
            return;
        }

        let guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        if self.is_complete() {
            return;
        }
        let _guard = self
            .wake
            .wait_timeout(guard, timeout)
            .unwrap_or_else(|e| e.into_inner());
    }

    pub(super) fn complete(&self) {
        self.done.store(true, Ordering::Release);
        self.wake.notify_all();
    }
}
