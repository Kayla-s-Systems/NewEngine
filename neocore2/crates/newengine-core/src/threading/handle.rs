use newengine_loading_api::{EngineTaskControlEvent, EngineTaskPhase};
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::task_core::{
    CoreTaskControl, CoreTaskRuntimeStatus, CoreTaskTicket, TaskLane,
    TaskPriority as CoreTaskPriority, TaskRequest, ThreadPoolCoreHandle,
};

use super::dto::{task_status_from_phase, CpuTaskDto, CpuTaskResultDto, TaskStatus};
use super::snapshot::ThreadPoolSnapshot;

/// Cooperative context passed to CPU task handlers.
#[derive(Clone)]
pub struct TaskContext {
    control: CoreTaskControl,
}

impl TaskContext {
    #[inline]
    pub fn task_id(&self) -> &str {
        self.control.task_id()
    }

    #[inline]
    pub fn checkpoint(&self) -> bool {
        self.control.checkpoint()
    }

    #[inline]
    pub fn is_cancel_requested(&self) -> bool {
        self.control.is_cancel_requested()
    }

    #[inline]
    pub fn publish_progress(
        &self,
        progress_01: f32,
        status: impl Into<String>,
        detail: impl Into<String>,
    ) {
        self.control
            .publish_progress(progress_01, status.into(), detail.into());
    }
}

#[derive(Clone, Debug)]
pub struct TaskRuntimeStatus {
    pub task_id: String,
    pub label: &'static str,
    pub lane: TaskLane,
    pub priority: CoreTaskPriority,
    pub frame_id: Option<u64>,
    pub dependency_group: Option<String>,
    pub task_domain: &'static str,
    pub task_pass: &'static str,
    pub phase: EngineTaskPhase,
    pub can_pause: bool,
    pub can_cancel: bool,
    pub cancel_requested: bool,
    pub pause_requested: bool,
}

impl TaskRuntimeStatus {
    pub(crate) fn from_core_status(status: CoreTaskRuntimeStatus) -> Self {
        Self {
            task_id: status.task_id,
            label: status.label,
            lane: status.lane,
            priority: status.priority,
            frame_id: status.frame_id,
            dependency_group: status.dependency_group,
            task_domain: status.task_domain,
            task_pass: status.task_pass,
            phase: status.phase,
            can_pause: status.can_pause,
            can_cancel: status.can_cancel,
            cancel_requested: status.cancel_requested,
            pause_requested: status.pause_requested,
        }
    }

    #[inline]
    pub fn task_status(&self) -> TaskStatus {
        task_status_from_phase(self.phase)
    }
}

pub struct TaskTicket {
    ticket: CoreTaskTicket,
}

impl TaskTicket {
    #[inline]
    pub(crate) fn new(ticket: CoreTaskTicket) -> Self {
        Self { ticket }
    }

    #[inline]
    pub fn is_complete(&self) -> bool {
        self.ticket.is_complete()
    }

    #[inline]
    pub fn task_id(&self) -> &str {
        self.ticket.task_id()
    }

    #[inline]
    pub fn status(&self) -> TaskRuntimeStatus {
        TaskRuntimeStatus::from_core_status(self.ticket.status())
    }

    #[inline]
    pub fn task_status(&self) -> TaskStatus {
        self.status().task_status()
    }

    #[inline]
    pub fn cancel(&self) -> bool {
        self.ticket.cancel()
    }

    #[inline]
    pub fn pause(&self) -> bool {
        self.ticket.pause()
    }

    #[inline]
    pub fn resume(&self) -> bool {
        self.ticket.resume()
    }

    #[inline]
    pub fn wait(self) {
        self.ticket.wait();
    }
}

/// Wait handle for DTO-based engine.threading CPU tasks.
pub struct CpuTaskTicket {
    ticket: TaskTicket,
    result: Arc<Mutex<Option<CpuTaskResultDto>>>,
}

impl CpuTaskTicket {
    #[inline]
    pub fn task_id(&self) -> &str {
        self.ticket.task_id()
    }

    #[inline]
    pub fn is_complete(&self) -> bool {
        self.ticket.is_complete()
    }

    #[inline]
    pub fn cancel(&self) -> bool {
        self.ticket.cancel()
    }

    #[inline]
    pub fn status(&self) -> TaskStatus {
        self.ticket.task_status()
    }

    pub fn wait_result(self) -> CpuTaskResultDto {
        let task_id = self.ticket.task_id().to_owned();
        let fallback_status = self.status();
        self.ticket.wait();
        self.result.lock().take().unwrap_or(CpuTaskResultDto {
            task_id,
            status: fallback_status,
            cpu_time_ns: 0,
            output: Vec::new(),
        })
    }
}

/// Cloneable engine.threading submission/control endpoint.
#[derive(Clone)]
pub struct ThreadPoolHandle {
    core: ThreadPoolCoreHandle,
}

impl ThreadPoolHandle {
    #[inline]
    pub(crate) fn new(core: ThreadPoolCoreHandle) -> Self {
        Self { core }
    }

    pub fn submit_request<F>(&self, request: TaskRequest, f: F) -> TaskTicket
    where
        F: FnOnce() + Send + 'static,
    {
        TaskTicket::new(self.core.submit_request(request, f))
    }

    pub fn submit_controlled<F>(&self, request: TaskRequest, f: F) -> TaskTicket
    where
        F: FnOnce(TaskContext) + Send + 'static,
    {
        TaskTicket::new(self.core.submit_controlled(request, move |control| {
            f(TaskContext { control });
        }))
    }

    pub fn submit_named<F>(&self, label: &'static str, f: F) -> TaskTicket
    where
        F: FnOnce() + Send + 'static,
    {
        TaskTicket::new(self.core.submit_named(label, f))
    }

    pub fn submit_lane<F>(&self, lane: TaskLane, label: &'static str, f: F) -> TaskTicket
    where
        F: FnOnce() + Send + 'static,
    {
        TaskTicket::new(self.core.submit_lane(lane, label, f))
    }

    pub fn run_indexed_request<T, F>(&self, len: usize, request: TaskRequest, f: F) -> Vec<T>
    where
        T: Send + 'static,
        F: Fn(usize) -> T + Send + Sync + 'static,
    {
        self.core.run_indexed_request(len, request, f)
    }

    pub fn submit<F>(&self, task: CpuTaskDto, f: F) -> CpuTaskTicket
    where
        F: FnOnce(CpuTaskDto, TaskContext) -> Vec<u8> + Send + 'static,
    {
        let result = Arc::new(Mutex::new(None));
        let result_worker = Arc::clone(&result);
        let request = task.to_task_request();
        let ticket = self.submit_controlled(request, move |context| {
            let started = Instant::now();
            if !context.checkpoint() {
                *result_worker.lock() = Some(CpuTaskResultDto {
                    task_id: context.task_id().to_owned(),
                    status: TaskStatus::Cancelled,
                    cpu_time_ns: started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64,
                    output: Vec::new(),
                });
                return;
            }
            let output = f(task, context.clone());
            let status = if context.is_cancel_requested() {
                TaskStatus::Cancelled
            } else {
                TaskStatus::Completed
            };
            *result_worker.lock() = Some(CpuTaskResultDto {
                task_id: context.task_id().to_owned(),
                status,
                cpu_time_ns: started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64,
                output,
            });
        });
        CpuTaskTicket { ticket, result }
    }

    #[inline]
    pub fn query(&self, task_id: &str) -> Option<TaskRuntimeStatus> {
        self.core
            .task_status(task_id)
            .map(TaskRuntimeStatus::from_core_status)
    }

    #[inline]
    pub fn cancel(&self, task_id: &str) -> bool {
        self.core.cancel_task(task_id)
    }

    #[inline]
    pub fn apply_control_event(&self, event: &EngineTaskControlEvent) -> bool {
        self.core.apply_control_event(event)
    }

    #[inline]
    pub fn begin_frame_budget(&self, budget: Duration) {
        self.core.begin_frame_budget(budget);
    }

    #[inline]
    pub fn set_frame_cpu_budget(&self, budget: Duration) {
        self.core.set_frame_cpu_budget(budget);
    }

    #[inline]
    pub fn worker_threads(&self) -> usize {
        self.core.worker_threads()
    }

    #[inline]
    pub fn pending_for_lane(&self, lane: TaskLane) -> usize {
        self.core.pending_for_lane(lane)
    }

    #[inline]
    pub fn pending_jobs(&self) -> usize {
        self.core.pending_jobs()
    }

    pub fn task_status(&self, task_id: &str) -> Option<TaskRuntimeStatus> {
        self.query(task_id)
    }

    #[inline]
    pub fn cancel_task(&self, task_id: &str) -> bool {
        self.core.cancel_task(task_id)
    }

    #[inline]
    pub fn pause_task(&self, task_id: &str) -> bool {
        self.core.pause_task(task_id)
    }

    #[inline]
    pub fn resume_task(&self, task_id: &str) -> bool {
        self.core.resume_task(task_id)
    }

    #[inline]
    pub fn snapshot(&self) -> ThreadPoolSnapshot {
        ThreadPoolSnapshot::from_core_snapshot(self.core.snapshot())
    }
}
