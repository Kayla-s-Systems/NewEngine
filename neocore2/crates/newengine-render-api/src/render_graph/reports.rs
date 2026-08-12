use crate::UploadPumpReport;
use serde::{Deserialize, Serialize};

use super::{RenderDrawListKind, RenderGraphPassId, RenderGraphResourceId};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RenderGraphLifetimeStats {
    pub persistent: u32,
    pub transient: u32,
    pub retired: u32,
    pub destroyed: u32,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct RenderGraphBarrierStats {
    pub read_after_write: u32,
    pub write_after_read: u32,
    pub write_after_write: u32,
    pub external_imports: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RenderGraphDiagnosticsStats {
    pub compiled_graphs: u64,
    pub submitted_graphs: u64,
    pub skipped_passes: u64,
    pub lifetime: RenderGraphLifetimeStats,
    pub barriers: RenderGraphBarrierStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderGraphValidationIssue {
    pub code: String,
    pub message: String,
    pub pass: Option<RenderGraphPassId>,
    pub resource: Option<RenderGraphResourceId>,
}

impl RenderGraphValidationIssue {
    #[inline]
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            pass: None,
            resource: None,
        }
    }

    #[inline]
    pub fn with_pass(mut self, pass: RenderGraphPassId) -> Self {
        self.pass = Some(pass);
        self
    }

    #[inline]
    pub fn with_resource(mut self, resource: RenderGraphResourceId) -> Self {
        self.resource = Some(resource);
        self
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RenderGraphCompileReport {
    pub pass_count: u32,
    pub resource_count: u32,
    pub execution_order: Vec<RenderGraphPassId>,
    pub lifetime: RenderGraphLifetimeStats,
    pub barriers: RenderGraphBarrierStats,
    pub warnings: Vec<RenderGraphValidationIssue>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RecordedDrawListStats {
    pub draw_list: RenderDrawListKind,
    pub recorded_commands: u32,
    pub draw_calls: u32,
    pub indexed_draw_calls: u32,
    pub skipped_commands: u32,
    #[serde(default)]
    pub viewport_sets: u32,
    #[serde(default)]
    pub scissor_sets: u32,
    #[serde(default)]
    pub pipeline_binds: u32,
    #[serde(default)]
    pub vertex_buffer_binds: u32,
    #[serde(default)]
    pub index_buffer_binds: u32,
    #[serde(default)]
    pub bind_group_binds: u32,
    #[serde(default)]
    pub invalid_draw_calls: u32,
}

impl RecordedDrawListStats {
    #[inline]
    pub fn total_draw_calls(self) -> u32 {
        self.draw_calls.saturating_add(self.indexed_draw_calls)
    }

    #[inline]
    pub fn submitted_commands(self) -> u32 {
        self.recorded_commands.saturating_sub(self.skipped_commands)
    }
}

impl Default for RecordedDrawListStats {
    #[inline]
    fn default() -> Self {
        Self {
            draw_list: RenderDrawListKind::OpaqueForward,
            recorded_commands: 0,
            draw_calls: 0,
            indexed_draw_calls: 0,
            skipped_commands: 0,
            viewport_sets: 0,
            scissor_sets: 0,
            pipeline_binds: 0,
            vertex_buffer_binds: 0,
            index_buffer_binds: 0,
            bind_group_binds: 0,
            invalid_draw_calls: 0,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RenderGraphValidationReport {
    pub ok: bool,
    pub errors: Vec<RenderGraphValidationIssue>,
    pub warnings: Vec<RenderGraphValidationIssue>,
    pub compile: Option<RenderGraphCompileReport>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RenderGraphSubmitReport {
    pub cpu_record_ms: f32,
    pub gpu_submit_ms: f32,
    pub executed_passes: u32,
    pub skipped_passes: u32,
    pub uploads: UploadPumpReport,
    pub compile: RenderGraphCompileReport,
    #[serde(default)]
    pub draw_list_stats: Vec<RecordedDrawListStats>,
}
