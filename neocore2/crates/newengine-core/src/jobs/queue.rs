use crate::events::EventHub;
use newengine_loading_api::EngineTaskPhase;
use newengine_math::collections_prelude::{NeHashMap as HashMap, NeVecDeque as VecDeque};
use parking_lot::Mutex;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex as StdMutex};
use std::time::Duration;

use super::config::{JobLane, JobPriority, JOB_LANE_COUNT, JOB_PRIORITY_COUNT};
use super::control::{JobCompletion, JobControl};
use super::id;
use super::request::JobRequest;
use super::status::JobSystemSnapshot;

type JobFn = Box<dyn FnOnce(JobControl) + Send + 'static>;

pub(super) struct QueuedJob {
    pub(super) request: JobRequest,
    pub(super) job: Option<JobFn>,
    pub(super) completion: Arc<JobCompletion>,
    pub(super) control: JobControl,
}

impl QueuedJob {
    pub(super) fn run(mut self, shared: &JobShared) {
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

pub(super) struct JobShared {
    pub(super) queues: Vec<Mutex<VecDeque<QueuedJob>>>,
    pub(super) pending_by_lane: Vec<AtomicUsize>,
    pub(super) completed_by_lane: Vec<AtomicU64>,
    pub(super) worker_threads: usize,
    pub(super) pending: AtomicUsize,
    pub(super) running: AtomicUsize,
    pub(super) paused: AtomicUsize,
    pub(super) submitted: AtomicU64,
    pub(super) completed: AtomicU64,
    pub(super) cancelled: AtomicU64,
    pub(super) panicked: AtomicU64,
    pub(super) next_task_id: AtomicU64,
    pub(super) shutdown: AtomicBool,
    pub(super) events: Option<EventHub>,
    pub(super) tasks: Mutex<HashMap<String, JobControl>>,
    pub(super) sleep_lock: StdMutex<()>,
    pub(super) sleep_wake: Condvar,
}

impl JobShared {
    pub(super) fn new(worker_threads: usize, events: Option<EventHub>) -> Self {
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
    pub(super) fn worker_count(&self) -> usize {
        self.worker_threads.max(1)
    }

    #[inline]
    pub(super) fn next_task_id(&self) -> String {
        id::format_task_id(self.next_task_id.fetch_add(1, Ordering::Relaxed))
    }

    #[inline]
    fn queue_index(lane: JobLane, priority: JobPriority) -> usize {
        priority.index() * JOB_LANE_COUNT + lane.index()
    }

    pub(super) fn submit(&self, job: QueuedJob) {
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

    pub(super) fn pop_next(&self) -> Option<QueuedJob> {
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

    pub(super) fn wait_for_work_or_shutdown(&self) {
        if self.shutdown.load(Ordering::Acquire) || self.pending.load(Ordering::Acquire) > 0 {
            return;
        }

        let guard = self.sleep_lock.lock().unwrap_or_else(|e| e.into_inner());
        let _ = self
            .sleep_wake
            .wait_timeout(guard, Duration::from_millis(2))
            .unwrap_or_else(|e| e.into_inner());
    }

    pub(super) fn snapshot(&self) -> JobSystemSnapshot {
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
