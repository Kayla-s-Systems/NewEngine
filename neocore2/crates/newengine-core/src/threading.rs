#![forbid(unsafe_op_in_unsafe_fn)]

mod config;
mod dto;
mod handle;
mod manager;
mod snapshot;

#[cfg(test)]
mod tests;

pub use config::{
    ThreadPoolConfig, ENGINE_THREADING_GATEWAY_ID, THREADING_BACKEND_CAPABILITY_ID,
    THREADING_PROVIDER_SERVICE_ID, THREADING_RUNTIME_CONTRACT,
};
pub use dto::{CpuTaskDto, CpuTaskPriority, CpuTaskResultDto, TaskStatus};
pub use handle::{CpuTaskTicket, TaskContext, TaskRuntimeStatus, TaskTicket, ThreadPoolHandle};
pub use manager::ThreadPoolManager;
pub use snapshot::{ThreadPoolLaneSnapshot, ThreadPoolSnapshot};
