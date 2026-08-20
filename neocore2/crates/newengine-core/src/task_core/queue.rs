use crate::events::EventHub;
use newengine_loading_api::EngineTaskPhase;
use newengine_math::collections_prelude::{NeHashMap as HashMap, NeVecDeque as VecDeque};
use parking_lot::Mutex;
use std::cell::RefCell;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex as StdMutex};
use std::time::{Duration, Instant};

use super::config::{TaskLane, TaskPriority, JOB_LANE_COUNT, JOB_PRIORITY_COUNT};
use super::control::{CoreTaskControl, TaskCompletion};
use super::id;
use super::request::TaskRequest;
use super::status::ThreadPoolCoreSnapshot;

type TaskFn = Box<dyn FnOnce(CoreTaskControl) + Send + 'static>;

pub(super) struct QueuedTask {
    pub(super) request: TaskRequest,
    pub(super) job: Option<TaskFn>,
    pub(super) control: CoreTaskControl,
}

struct BlockedTask {
    task: QueuedTask,
    unresolved_dependencies: usize,
}

#[derive(Default)]
struct TaskDependencyGraph {
    /// Tasks that have unresolved prerequisites and therefore must not occupy a
    /// lane/priority ready queue.
    blocked: HashMap<String, BlockedTask>,
    /// Reverse edges: prerequisite task id -> tasks waiting on that prerequisite.
    dependents: HashMap<String, Vec<String>>,
}

#[derive(Clone, Copy, Debug)]
enum TaskBodyOutcome {
    CompletedNoClosure,
    Completed,
    Failed,
    CancelledBeforeExecution,
    CancelledWhilePaused,
    CancelledAfterExecution,
}

impl TaskBodyOutcome {
    #[inline]
    fn phase(self) -> EngineTaskPhase {
        match self {
            Self::CompletedNoClosure | Self::Completed => EngineTaskPhase::Completed,
            Self::Failed => EngineTaskPhase::Failed,
            Self::CancelledBeforeExecution
            | Self::CancelledWhilePaused
            | Self::CancelledAfterExecution => EngineTaskPhase::Cancelled,
        }
    }

    #[inline]
    fn status(self) -> &'static str {
        match self {
            Self::CompletedNoClosure | Self::Completed => "Task completed",
            Self::Failed => "Task failed",
            Self::CancelledBeforeExecution
            | Self::CancelledWhilePaused
            | Self::CancelledAfterExecution => "Task cancelled",
        }
    }

    #[inline]
    fn detail(self) -> &'static str {
        match self {
            Self::CompletedNoClosure => "Task completed without a task closure.",
            Self::Completed => "Task finished on engine-runtime worker thread.",
            Self::Failed => "Worker task panicked; worker recovered and continues.",
            Self::CancelledBeforeExecution => "Task was cancelled before worker execution.",
            Self::CancelledWhilePaused => "Task was cancelled while paused before execution.",
            Self::CancelledAfterExecution => "Task completed after observing cancellation.",
        }
    }

    #[inline]
    fn counts_completed(self) -> bool {
        !matches!(
            self,
            Self::CancelledBeforeExecution | Self::CancelledWhilePaused
        )
    }

    #[inline]
    fn counts_cancelled(self) -> bool {
        matches!(
            self,
            Self::CancelledBeforeExecution
                | Self::CancelledWhilePaused
                | Self::CancelledAfterExecution
        )
    }

    #[inline]
    fn counts_panicked(self) -> bool {
        matches!(self, Self::Failed)
    }
}

struct TaskHierarchyNode {
    parent_task_id: Option<String>,
    pending_children: usize,
    body_outcome: Option<TaskBodyOutcome>,
    finalized: bool,
}

#[derive(Default)]
struct TaskHierarchyGraph {
    nodes: HashMap<String, TaskHierarchyNode>,
    /// Children may be submitted before their parent is registered. Keep the
    /// relationship so parent completion remains order-independent.
    waiting_children: HashMap<String, Vec<String>>,
}

impl TaskHierarchyGraph {
    fn register(&mut self, task_id: &str, parent_task_id: Option<&str>) {
        if self.nodes.contains_key(task_id) {
            return;
        }

        let parent_task_id = parent_task_id
            .map(str::trim)
            .filter(|parent| !parent.is_empty() && *parent != task_id)
            .map(str::to_owned);

        let pending_children = self
            .waiting_children
            .remove(task_id)
            .unwrap_or_default()
            .into_iter()
            .filter(|child_id| {
                self.nodes
                    .get(child_id)
                    .is_some_and(|child| !child.finalized)
            })
            .count();

        self.nodes.insert(
            task_id.to_owned(),
            TaskHierarchyNode {
                parent_task_id: parent_task_id.clone(),
                pending_children,
                body_outcome: None,
                finalized: false,
            },
        );

        let Some(parent_task_id) = parent_task_id else {
            return;
        };

        if let Some(parent) = self.nodes.get_mut(&parent_task_id) {
            if !parent.finalized {
                parent.pending_children = parent.pending_children.saturating_add(1);
            }
        } else {
            self.waiting_children
                .entry(parent_task_id)
                .or_default()
                .push(task_id.to_owned());
        }
    }

    fn finish_body(
        &mut self,
        task_id: &str,
        outcome: TaskBodyOutcome,
    ) -> (bool, Vec<(String, TaskBodyOutcome)>) {
        let Some(node) = self.nodes.get_mut(task_id) else {
            return (false, Vec::new());
        };
        if node.finalized {
            return (false, Vec::new());
        }
        node.body_outcome = Some(outcome);
        let waiting_for_children = node.pending_children > 0;

        let mut finalized = Vec::new();
        let mut candidate = Some(task_id.to_owned());
        while let Some(candidate_id) = candidate.take() {
            let (parent_task_id, candidate_outcome) = {
                let Some(candidate_node) = self.nodes.get_mut(&candidate_id) else {
                    break;
                };
                if candidate_node.finalized || candidate_node.pending_children != 0 {
                    break;
                }
                let Some(candidate_outcome) = candidate_node.body_outcome else {
                    break;
                };
                candidate_node.finalized = true;
                (candidate_node.parent_task_id.clone(), candidate_outcome)
            };

            finalized.push((candidate_id, candidate_outcome));

            let Some(parent_task_id) = parent_task_id else {
                continue;
            };
            let Some(parent) = self.nodes.get_mut(&parent_task_id) else {
                continue;
            };
            if parent.finalized {
                continue;
            }

            parent.pending_children = parent.pending_children.saturating_sub(1);
            if parent.pending_children == 0 && parent.body_outcome.is_some() {
                candidate = Some(parent_task_id);
            }
        }

        (waiting_for_children, finalized)
    }
}

thread_local! {
    /// Execution stack exists only while this thread is actively executing work
    /// from the engine task core. External waiters therefore never become ad-hoc
    /// workers merely by calling `CoreTaskTicket::wait`.
    static EXECUTION_LANES: RefCell<Vec<TaskLane>> = const { RefCell::new(Vec::new()) };
}

struct TaskExecutionScope;

impl TaskExecutionScope {
    #[inline]
    fn enter(lane: TaskLane) -> Self {
        EXECUTION_LANES.with(|lanes| lanes.borrow_mut().push(lane));
        Self
    }
}

impl Drop for TaskExecutionScope {
    fn drop(&mut self) {
        EXECUTION_LANES.with(|lanes| {
            let _ = lanes.borrow_mut().pop();
        });
    }
}

#[inline]
fn execution_depth_for_lane(lane: TaskLane) -> usize {
    EXECUTION_LANES.with(|lanes| {
        lanes
            .borrow()
            .iter()
            .filter(|active_lane| **active_lane == lane)
            .count()
    })
}

#[inline]
fn in_worker_task_context() -> bool {
    EXECUTION_LANES.with(|lanes| !lanes.borrow().is_empty())
}

#[inline]
fn duration_to_ns(duration: Duration) -> u64 {
    duration.as_nanos().min(u128::from(u64::MAX)) as u64
}

impl QueuedTask {
    pub(super) fn run(mut self, shared: &TaskCoreShared) {
        let lane = self.request.lane;
        let task_id = self.control.task_id().to_owned();

        let Some(job) = self.job.take() else {
            shared.finish_task_body(&task_id, TaskBodyOutcome::CompletedNoClosure);
            shared.release_lane(lane);
            return;
        };

        if self.control.is_cancel_requested() {
            shared.finish_task_body(&task_id, TaskBodyOutcome::CancelledBeforeExecution);
            shared.release_lane(lane);
            return;
        }

        shared.running.fetch_add(1, Ordering::AcqRel);
        self.control.publish(
            EngineTaskPhase::Running,
            "Task running",
            "Worker picked up the task from the engine queue.",
            None,
        );
        if !self.control.wait_while_paused() {
            shared.running.fetch_sub(1, Ordering::AcqRel);
            shared.finish_task_body(&task_id, TaskBodyOutcome::CancelledWhilePaused);
            shared.release_lane(lane);
            return;
        }

        let control = self.control.clone();
        let started = Instant::now();
        let result = catch_unwind(AssertUnwindSafe(move || {
            job(control);
        }));
        let elapsed_ns = started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
        shared.record_cpu_time_ns(lane, elapsed_ns);
        shared.running.fetch_sub(1, Ordering::AcqRel);

        let outcome = if result.is_err() {
            newengine_ulog_api::ulog::error!(
                "task-core: worker task panicked label='{}' lane='{}' priority={:?}; worker recovered and continues",
                self.request.label,
                lane.as_str(),
                self.request.priority
            );
            TaskBodyOutcome::Failed
        } else if self.control.is_cancel_requested() {
            TaskBodyOutcome::CancelledAfterExecution
        } else {
            TaskBodyOutcome::Completed
        };

        shared.finish_task_body(&task_id, outcome);
        shared.release_lane(lane);
    }
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

    #[inline]
    fn queue_index(lane: TaskLane, priority: TaskPriority) -> usize {
        priority.index() * JOB_LANE_COUNT + lane.index()
    }

    #[inline]
    fn queue_bit(queue_index: usize) -> u64 {
        debug_assert!(queue_index < u64::BITS as usize);
        1u64 << queue_index
    }

    fn enqueue_ready(&self, task: QueuedTask) {
        let queue_index = Self::queue_index(task.request.lane, task.request.priority);
        let mut queue = self.queues[queue_index].lock();
        queue.push_back(task);
        // Publish readiness while the queue lock is still held. A consumer that
        // drains the queue clears the bit under the same lock, preventing a lost
        // ready transition between push and mask update.
        self.ready_queue_mask
            .fetch_or(Self::queue_bit(queue_index), Ordering::Release);
    }

    #[inline]
    fn try_pop_ready_queue(&self, queue_index: usize) -> Option<QueuedTask> {
        let bit = Self::queue_bit(queue_index);
        if self.ready_queue_mask.load(Ordering::Acquire) & bit == 0 {
            return None;
        }

        let mut queue = self.queues[queue_index].lock();
        let job = queue.pop_front();
        if queue.is_empty() {
            // `enqueue_ready` sets this bit while holding the same queue lock, so
            // clearing it here cannot erase a concurrent producer's transition.
            self.ready_queue_mask.fetch_and(!bit, Ordering::AcqRel);
        }
        job
    }

    pub(super) fn register_task_hierarchy(&self, task_id: &str, parent_task_id: Option<&str>) {
        self.task_hierarchy.lock().register(task_id, parent_task_id);
    }

    #[inline]
    pub(super) fn run_ready_task(&self, job: QueuedTask) {
        let _execution_scope = TaskExecutionScope::enter(job.request.lane);
        job.run(self);
    }

    pub(super) fn wait_for_completion(&self, completion: &TaskCompletion) {
        if completion.is_complete() {
            return;
        }

        if !in_worker_task_context() {
            completion.wait();
            return;
        }

        while !completion.is_complete() {
            if let Some(job) = self.pop_next_helping() {
                self.run_ready_task(job);
                continue;
            }

            // The target may be running on another worker or blocked on a graph
            // prerequisite. A bounded wait avoids spinning while still letting
            // newly-ready work be helped promptly.
            completion.wait_timeout(Duration::from_millis(1));
        }
    }

    fn release_lane(&self, lane: TaskLane) {
        self.running_by_lane[lane.index()].fetch_sub(1, Ordering::AcqRel);
        self.sleep_wake.notify_one();
    }

    fn finish_task_body(&self, task_id: &str, outcome: TaskBodyOutcome) {
        let (waiting_for_children, finalized) =
            self.task_hierarchy.lock().finish_body(task_id, outcome);

        if waiting_for_children {
            if let Some(control) = self.tasks.lock().get(task_id).cloned() {
                control.publish(
                    EngineTaskPhase::Blocked,
                    "Task waiting for nested work",
                    "Task body finished; completion is deferred until nested child tasks finish.",
                    None,
                );
            }
        }

        for (finalized_task_id, finalized_outcome) in finalized {
            self.finalize_task(&finalized_task_id, finalized_outcome);
        }
    }

    fn finalize_task(&self, task_id: &str, outcome: TaskBodyOutcome) {
        let control = self.tasks.lock().get(task_id).cloned();
        if let Some(control) = control.as_ref() {
            let lane_index = control.status().lane.index();
            if outcome.counts_completed() {
                self.completed.fetch_add(1, Ordering::AcqRel);
                self.completed_by_lane[lane_index].fetch_add(1, Ordering::AcqRel);
            }
            if outcome.counts_cancelled() {
                self.cancelled.fetch_add(1, Ordering::AcqRel);
            }
            if outcome.counts_panicked() {
                self.panicked.fetch_add(1, Ordering::AcqRel);
            }

            control.publish(
                outcome.phase(),
                outcome.status(),
                outcome.detail(),
                Some(1.0),
            );
        }

        if let Some(completion) = self.completions.lock().get(task_id).cloned() {
            completion.complete();
        }
        self.release_dependents(task_id);
        self.sleep_wake.notify_all();
    }

    /// Registers reverse prerequisite edges under one graph lock. A task either
    /// becomes immediately runnable or lives exclusively in `blocked` until the
    /// last prerequisite completes.
    fn register_dependencies(&self, task: QueuedTask) -> Option<QueuedTask> {
        if task.request.prerequisite_task_ids.is_empty() {
            return Some(task);
        }

        let task_id = task.control.task_id().to_owned();
        let mut graph = self.dependency_graph.lock();
        // Completion state is sampled while holding the dependency graph lock.
        // A concurrent terminal task publishes its atomic completion before it
        // can acquire this same graph lock to release dependents, closing the
        // submit-vs-complete race without a second completed-id registry.
        let completions = self.completions.lock();
        let mut unresolved_dependencies = 0usize;
        let mut registered = Vec::<String>::new();

        for prerequisite in &task.request.prerequisite_task_ids {
            let prerequisite = prerequisite.trim();
            if prerequisite.is_empty()
                || registered
                    .iter()
                    .any(|registered_id| registered_id == prerequisite)
            {
                continue;
            }
            registered.push(prerequisite.to_owned());

            if completions
                .get(prerequisite)
                .is_some_and(|completion| completion.is_complete())
            {
                continue;
            }

            unresolved_dependencies = unresolved_dependencies.saturating_add(1);
            graph
                .dependents
                .entry(prerequisite.to_owned())
                .or_default()
                .push(task_id.clone());
        }

        if unresolved_dependencies == 0 {
            return Some(task);
        }

        graph.blocked.insert(
            task_id,
            BlockedTask {
                task,
                unresolved_dependencies,
            },
        );
        None
    }

    /// Marks a task terminal and directly wakes graph nodes whose final
    /// prerequisite was satisfied. Workers never rescan blocked dependencies.
    fn release_dependents(&self, completed_task_id: &str) {
        let ready = {
            let mut graph = self.dependency_graph.lock();
            let dependent_ids = graph
                .dependents
                .remove(completed_task_id)
                .unwrap_or_default();
            let mut ready_ids = Vec::new();

            for dependent_id in dependent_ids {
                let Some(blocked) = graph.blocked.get_mut(&dependent_id) else {
                    continue;
                };
                blocked.unresolved_dependencies = blocked.unresolved_dependencies.saturating_sub(1);
                if blocked.unresolved_dependencies == 0 {
                    ready_ids.push(dependent_id);
                }
            }

            let mut ready = Vec::with_capacity(ready_ids.len());
            for ready_id in ready_ids {
                if let Some(blocked) = graph.blocked.remove(&ready_id) {
                    ready.push(blocked.task);
                }
            }
            ready
        };

        if ready.is_empty() {
            return;
        }
        for task in ready {
            self.enqueue_ready(task);
        }
        self.sleep_wake.notify_all();
    }

    pub(super) fn submit(&self, job: QueuedTask) {
        if self.shutdown.load(Ordering::Acquire) {
            let task_id = job.control.task_id().to_owned();
            job.control.publish(
                EngineTaskPhase::Cancelled,
                "Task rejected",
                "Task core is shutting down; task was not queued.",
                Some(1.0),
            );
            self.finish_task_body(&task_id, TaskBodyOutcome::CancelledBeforeExecution);
            return;
        }

        let lane_index = job.request.lane.index();

        self.submitted.fetch_add(1, Ordering::AcqRel);
        self.pending.fetch_add(1, Ordering::Release);
        self.pending_by_lane[lane_index].fetch_add(1, Ordering::Release);
        job.control.publish(
            EngineTaskPhase::Scheduled,
            "Task scheduled",
            "Task was registered in the engine task graph.",
            Some(0.0),
        );

        if let Some(ready) = self.register_dependencies(job) {
            self.enqueue_ready(ready);
            self.sleep_wake.notify_one();
        }
    }

    fn lane_active_limit(&self, lane: TaskLane) -> usize {
        let workers = self.worker_count().max(1);
        match lane {
            TaskLane::Simulation => workers,
            TaskLane::RenderPrep => workers.saturating_sub(1).max(1),
            TaskLane::Streaming => (workers / 2).max(1),
            TaskLane::AssetIo => (workers / 3).max(1),
            TaskLane::Plugin => 1,
            // Background work includes external compiler/tool processes. Keep it visible,
            // but never let it occupy the whole worker pool while frame-critical lanes wait.
            TaskLane::Background => (workers / 4).max(1),
        }
    }

    #[inline]
    fn lane_has_capacity(&self, lane: TaskLane) -> bool {
        self.running_by_lane[lane.index()].load(Ordering::Acquire) < self.lane_active_limit(lane)
    }

    #[inline]
    fn lane_has_helping_capacity(&self, lane: TaskLane) -> bool {
        let reentrant_depth = execution_depth_for_lane(lane);
        self.running_by_lane[lane.index()].load(Ordering::Acquire)
            < self.lane_active_limit(lane).saturating_add(reentrant_depth)
    }

    /// Keep a foreground reserve once bulk/background work consumes the soft
    /// frame budget. Critical work always proceeds; interactive Simulation and
    /// RenderPrep tasks may also proceed so AssetIo/Streaming throughput cannot
    /// turn the budget limiter into frame-critical priority inversion.
    #[inline]
    fn allowed_when_over_budget(priority: TaskPriority, lane: TaskLane) -> bool {
        priority == TaskPriority::Critical
            || (priority == TaskPriority::Interactive
                && matches!(lane, TaskLane::Simulation | TaskLane::RenderPrep))
    }

    fn has_schedulable_pending_work(&self) -> bool {
        let ready_mask = self.ready_queue_mask.load(Ordering::Acquire);
        if ready_mask == 0 {
            return false;
        }

        let over_budget = self.frame_over_budget();
        for priority in TaskPriority::service_order() {
            for lane in TaskLane::all() {
                if over_budget && !Self::allowed_when_over_budget(priority, lane) {
                    continue;
                }
                if !self.lane_has_capacity(lane) {
                    continue;
                }
                let idx = Self::queue_index(lane, priority);
                if ready_mask & Self::queue_bit(idx) != 0 {
                    return true;
                }
            }
        }
        false
    }

    pub(super) fn pop_next(&self) -> Option<QueuedTask> {
        let over_budget = self.frame_over_budget();
        for priority in TaskPriority::service_order() {
            for lane in TaskLane::all() {
                if over_budget && !Self::allowed_when_over_budget(priority, lane) {
                    continue;
                }
                if !self.lane_has_capacity(lane) {
                    continue;
                }
                let idx = Self::queue_index(lane, priority);
                if let Some(job) = self.try_pop_ready_queue(idx) {
                    let lane_index = job.request.lane.index();
                    self.pending_by_lane[lane_index].fetch_sub(1, Ordering::AcqRel);
                    self.pending.fetch_sub(1, Ordering::AcqRel);
                    self.running_by_lane[lane_index].fetch_add(1, Ordering::AcqRel);
                    return Some(job);
                }
            }
        }

        if over_budget && self.pending.load(Ordering::Acquire) > 0 {
            self.budget_deferred_polls.fetch_add(1, Ordering::AcqRel);
        }

        None
    }

    /// Pops work while a worker task is synchronously waiting for another
    /// task. The frame budget is intentionally not consulted here: progress on
    /// an explicit dependency must not deadlock behind the current frame budget.
    /// Lane caps remain enforced, with a strictly thread-local re-entrant allowance
    /// for lanes already present in this worker's execution stack.
    fn pop_next_helping(&self) -> Option<QueuedTask> {
        for priority in TaskPriority::service_order() {
            for lane in TaskLane::all() {
                if !self.lane_has_helping_capacity(lane) {
                    continue;
                }
                let idx = Self::queue_index(lane, priority);
                if let Some(job) = self.try_pop_ready_queue(idx) {
                    let lane_index = job.request.lane.index();
                    self.pending_by_lane[lane_index].fetch_sub(1, Ordering::AcqRel);
                    self.pending.fetch_sub(1, Ordering::AcqRel);
                    self.running_by_lane[lane_index].fetch_add(1, Ordering::AcqRel);
                    return Some(job);
                }
            }
        }
        None
    }

    #[inline]
    pub(super) fn set_frame_cpu_budget(&self, budget: Duration) {
        self.frame_cpu_budget_ns
            .store(duration_to_ns(budget), Ordering::Release);
    }

    #[inline]
    pub(super) fn begin_frame_budget(&self, budget: Duration) {
        self.set_frame_cpu_budget(budget);
        self.frame_cpu_used_ns.store(0, Ordering::Release);
        self.sleep_wake.notify_all();
    }

    #[inline]
    fn frame_over_budget(&self) -> bool {
        let budget = self.frame_cpu_budget_ns.load(Ordering::Acquire);
        budget > 0 && self.frame_cpu_used_ns.load(Ordering::Acquire) >= budget
    }

    fn record_cpu_time_ns(&self, lane: TaskLane, elapsed_ns: u64) {
        self.total_cpu_time_ns
            .fetch_add(elapsed_ns, Ordering::AcqRel);
        self.cpu_time_ns_by_lane[lane.index()].fetch_add(elapsed_ns, Ordering::AcqRel);

        let budget = self.frame_cpu_budget_ns.load(Ordering::Acquire);
        if budget == 0 {
            return;
        }

        let previous = self
            .frame_cpu_used_ns
            .fetch_add(elapsed_ns, Ordering::AcqRel);
        let next = previous.saturating_add(elapsed_ns);
        if previous < budget && next >= budget {
            self.overbudget_frames.fetch_add(1, Ordering::AcqRel);
        }
    }

    pub(super) fn wait_for_work_or_shutdown(&self) {
        if self.shutdown.load(Ordering::Acquire) || self.has_schedulable_pending_work() {
            return;
        }

        let guard = self.sleep_lock.lock().unwrap_or_else(|e| e.into_inner());
        if self.shutdown.load(Ordering::Acquire) || self.has_schedulable_pending_work() {
            return;
        }

        let _guard = self
            .sleep_wake
            .wait(guard)
            .unwrap_or_else(|e| e.into_inner());
    }

    pub(super) fn snapshot(&self) -> ThreadPoolCoreSnapshot {
        let mut pending_by_lane = [0usize; JOB_LANE_COUNT];
        let mut running_by_lane = [0usize; JOB_LANE_COUNT];
        let mut completed_by_lane = [0u64; JOB_LANE_COUNT];
        let mut cpu_time_ns_by_lane = [0u64; JOB_LANE_COUNT];
        for lane in TaskLane::all() {
            pending_by_lane[lane.index()] =
                self.pending_by_lane[lane.index()].load(Ordering::Acquire);
            running_by_lane[lane.index()] =
                self.running_by_lane[lane.index()].load(Ordering::Acquire);
            completed_by_lane[lane.index()] =
                self.completed_by_lane[lane.index()].load(Ordering::Acquire);
            cpu_time_ns_by_lane[lane.index()] =
                self.cpu_time_ns_by_lane[lane.index()].load(Ordering::Acquire);
        }

        ThreadPoolCoreSnapshot {
            worker_threads: self.worker_count(),
            pending_jobs: self.pending.load(Ordering::Acquire),
            running_jobs: self.running.load(Ordering::Acquire),
            paused_jobs: self.paused.load(Ordering::Acquire),
            submitted_jobs: self.submitted.load(Ordering::Acquire),
            completed_jobs: self.completed.load(Ordering::Acquire),
            cancelled_jobs: self.cancelled.load(Ordering::Acquire),
            panicked_jobs: self.panicked.load(Ordering::Acquire),
            pending_by_lane,
            running_by_lane,
            completed_by_lane,
            total_cpu_time_ns: self.total_cpu_time_ns.load(Ordering::Acquire),
            frame_cpu_budget_ns: self.frame_cpu_budget_ns.load(Ordering::Acquire),
            frame_cpu_used_ns: self.frame_cpu_used_ns.load(Ordering::Acquire),
            overbudget_frames: self.overbudget_frames.load(Ordering::Acquire),
            budget_deferred_polls: self.budget_deferred_polls.load(Ordering::Acquire),
            cpu_time_ns_by_lane,
        }
    }
}
