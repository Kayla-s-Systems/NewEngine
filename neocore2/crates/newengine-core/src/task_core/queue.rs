use crate::events::EventHub;
use newengine_math::collections_prelude::{NeHashMap as HashMap, NeVecDeque as VecDeque};
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex as StdMutex};
use std::time::Duration;

use super::config::{TaskLane, TaskPriority, JOB_LANE_COUNT, JOB_PRIORITY_COUNT};
use super::control::{CoreTaskControl, TaskCompletion};
use super::id;
use super::request::TaskRequest;

#[path = "queue_dependency.rs"]
mod dependency;
#[path = "queue_execution.rs"]
mod execution;
#[path = "queue_hierarchy.rs"]
mod hierarchy;
#[path = "queue_scheduler.rs"]
mod scheduler;

use dependency::TaskDependencyGraph;
use execution::duration_to_ns;
use hierarchy::TaskHierarchyGraph;

type TaskFn = Box<dyn FnOnce(CoreTaskControl) + Send + 'static>;

pub(super) struct QueuedTask {
    pub(super) request: TaskRequest,
    pub(super) job: Option<TaskFn>,
    pub(super) control: CoreTaskControl,
}

pub(super) struct TaskCoreShared {
    pub(super) queues: Vec<Mutex<VecDeque<QueuedTask>>>,
    /// One bit per (priority, lane) ready queue. Workers consult this lock-free
    /// summary before touching queue mutexes; sparse workloads therefore avoid
    /// scanning and locking every empty queue on each poll.
    ready_queue_mask: AtomicU64,
    pub(super) pending_by_lane: Vec<AtomicUsize>,
    pub(super) running_by_lane: Vec<AtomicUsize>,
    pub(super) completed_by_lane: Vec<AtomicU64>,
    pub(super) cpu_time_ns_by_lane: Vec<AtomicU64>,
    pub(super) worker_threads: usize,
    pub(super) pending: AtomicUsize,
    pub(super) running: AtomicUsize,
    pub(super) paused: AtomicUsize,
    pub(super) submitted: AtomicU64,
    pub(super) completed: AtomicU64,
    pub(super) cancelled: AtomicU64,
    pub(super) panicked: AtomicU64,
    pub(super) total_cpu_time_ns: AtomicU64,
    pub(super) frame_cpu_budget_ns: AtomicU64,
    pub(super) frame_cpu_used_ns: AtomicU64,
    pub(super) overbudget_frames: AtomicU64,
    pub(super) budget_deferred_polls: AtomicU64,
    pub(super) next_task_id: AtomicU64,
    pub(super) shutdown: AtomicBool,
    pub(super) events: Option<EventHub>,
    pub(super) tasks: Mutex<HashMap<String, CoreTaskControl>>,
    /// Completion tokens retained by task id for ticket/status lookup and the
    /// worker helping-wait path. Dependency scheduling itself is event driven.
    pub(super) completions: Mutex<HashMap<String, Arc<TaskCompletion>>>,
    dependency_graph: Mutex<TaskDependencyGraph>,
    task_hierarchy: Mutex<TaskHierarchyGraph>,
    pub(super) sleep_lock: StdMutex<()>,
    pub(super) sleep_wake: Condvar,
}

impl TaskCoreShared {
    pub(super) fn new(
        worker_threads: usize,
        frame_cpu_budget: Duration,
        events: Option<EventHub>,
    ) -> Self {
        let worker_threads = worker_threads.max(1);
        let queue_count = JOB_LANE_COUNT * JOB_PRIORITY_COUNT;
        assert!(
            queue_count <= u64::BITS as usize,
            "task-core ready queue mask supports at most 64 lane/priority queues"
        );
        Self {
            queues: (0..queue_count)
                .map(|_| Mutex::new(VecDeque::new()))
                .collect(),
            ready_queue_mask: AtomicU64::new(0),
            pending_by_lane: (0..JOB_LANE_COUNT).map(|_| AtomicUsize::new(0)).collect(),
            running_by_lane: (0..JOB_LANE_COUNT).map(|_| AtomicUsize::new(0)).collect(),
            completed_by_lane: (0..JOB_LANE_COUNT).map(|_| AtomicU64::new(0)).collect(),
            cpu_time_ns_by_lane: (0..JOB_LANE_COUNT).map(|_| AtomicU64::new(0)).collect(),
            worker_threads,
            pending: AtomicUsize::new(0),
            running: AtomicUsize::new(0),
            paused: AtomicUsize::new(0),
            submitted: AtomicU64::new(0),
            completed: AtomicU64::new(0),
            cancelled: AtomicU64::new(0),
            panicked: AtomicU64::new(0),
            total_cpu_time_ns: AtomicU64::new(0),
            frame_cpu_budget_ns: AtomicU64::new(duration_to_ns(frame_cpu_budget)),
            frame_cpu_used_ns: AtomicU64::new(0),
            overbudget_frames: AtomicU64::new(0),
            budget_deferred_polls: AtomicU64::new(0),
            next_task_id: AtomicU64::new(1),
            shutdown: AtomicBool::new(false),
            events,
            tasks: Mutex::new(HashMap::default()),
            completions: Mutex::new(HashMap::default()),
            dependency_graph: Mutex::new(TaskDependencyGraph::default()),
            task_hierarchy: Mutex::new(TaskHierarchyGraph::default()),
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
}
