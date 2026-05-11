#![forbid(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemTaskKind {
    Boot,
    PluginDiscovery,
    AssetImport,
    ShaderCompile,
    TextureUpload,
    TerrainCook,
    RenderWarmup,
    StreamingInstall,
    StagedApply,
    Benchmark,
    Recovery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemTaskPhase {
    Queued,
    Preparing,
    Running,
    Applying,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemTaskStatus {
    pub id: String,
    pub kind: SystemTaskKind,
    pub phase: SystemTaskPhase,
    pub label: String,
    pub detail: String,
    pub current: u64,
    pub total: Option<u64>,
}

impl SystemTaskStatus {
    #[inline]
    pub fn progress_01(&self) -> Option<f32> {
        self.total
            .filter(|total| *total != 0)
            .map(|total| (self.current as f32 / total as f32).clamp(0.0, 1.0))
    }
}
