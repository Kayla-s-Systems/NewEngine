#![forbid(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SystemDiagnosticsSnapshot {
    pub frame_index: u64,
    pub cpu_frame_ms: f32,
    pub gpu_frame_ms: Option<f32>,
    pub queued_jobs: u64,
    pub completed_jobs: u64,
    pub asset_uploads_queued: u64,
    pub asset_upload_mb_queued: f32,
    pub warnings: Vec<String>,
}
