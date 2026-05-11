use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderUploadQueuePolicy {
    Immediate,
    FrameBudgeted,
    BackgroundStaged,
}

impl Default for RenderUploadQueuePolicy {
    #[inline]
    fn default() -> Self {
        Self::FrameBudgeted
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RenderWorkBudget {
    pub max_upload_bytes_per_frame: u64,
    pub max_upload_jobs_per_frame: u32,
    pub max_pipeline_builds_per_frame: u32,
    pub max_blocking_ms_per_frame: f32,
    pub upload_policy: RenderUploadQueuePolicy,
}

impl Default for RenderWorkBudget {
    #[inline]
    fn default() -> Self {
        Self {
            max_upload_bytes_per_frame: 8 * 1024 * 1024,
            max_upload_jobs_per_frame: 4,
            max_pipeline_builds_per_frame: 1,
            max_blocking_ms_per_frame: 2.0,
            upload_policy: RenderUploadQueuePolicy::FrameBudgeted,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RenderFramePacingConfig {
    pub target_frame_ms: f32,
    pub warn_frame_ms: f32,
    pub warn_upload_ms: f32,
    pub warn_pipeline_ms: f32,
}

impl Default for RenderFramePacingConfig {
    #[inline]
    fn default() -> Self {
        Self {
            target_frame_ms: 16.6667,
            warn_frame_ms: 22.0,
            warn_upload_ms: 3.0,
            warn_pipeline_ms: 8.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct RenderFrameTiming {
    pub frame_index: u64,
    pub last_begin_frame_ms: f32,
    pub last_end_frame_ms: f32,
    pub last_gpu_submit_ms: f32,
    pub last_blocking_upload_ms: f32,
    pub last_pipeline_build_ms: f32,
    pub worst_blocking_upload_ms: f32,
    pub worst_pipeline_build_ms: f32,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct RenderQueueStats {
    pub queued_upload_jobs: u32,
    pub queued_upload_bytes: u64,
    pub queued_pipeline_builds: u32,
    pub queued_shader_bakes: u32,
    pub completed_upload_jobs: u64,
    pub completed_upload_bytes: u64,
    pub blocking_upload_jobs: u64,
    pub blocking_upload_bytes: u64,
    pub pipeline_builds: u64,
    pub pipeline_cache_hits: u64,
    pub pipeline_cache_misses: u64,
    pub shader_cache_hits: u64,
    pub shader_cache_misses: u64,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct RenderResourceStats {
    pub buffers: u32,
    pub textures: u32,
    pub samplers: u32,
    pub shaders: u32,
    pub pipelines: u32,
    pub bind_group_layouts: u32,
    pub bind_groups: u32,
    pub render_targets: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderDiagnosticsSnapshot {
    pub frame: RenderFrameTiming,
    pub queue: RenderQueueStats,
    pub resources: RenderResourceStats,
    pub budget: RenderWorkBudget,
    pub pacing: RenderFramePacingConfig,
    pub notes: Vec<String>,
}

impl Default for RenderDiagnosticsSnapshot {
    #[inline]
    fn default() -> Self {
        Self {
            frame: RenderFrameTiming::default(),
            queue: RenderQueueStats::default(),
            resources: RenderResourceStats::default(),
            budget: RenderWorkBudget::default(),
            pacing: RenderFramePacingConfig::default(),
            notes: Vec::new(),
        }
    }
}
