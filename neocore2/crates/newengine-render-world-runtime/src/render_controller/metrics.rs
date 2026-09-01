#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::{
    RecordedDrawListStats, RenderDebugChartSample, RenderDebugTelemetry, RenderDiagnosticsSnapshot,
    RenderFrameDebugSnapshot, RenderGraphSubmitReport,
};
use newengine_scene_bridge_runtime::scene_bridge::{
    EngineViewDiagnostics, EngineViewTransitionPhase,
};

const DEBUG_HISTORY_CAPACITY: usize = 240;

#[derive(Clone, Debug)]
pub(super) struct RuntimeOverlayMetrics {
    pub(super) frame_triangles: u64,
    pub(super) frame_draws: u32,
    fps_ema: f32,
    initialized: bool,
    last_submit: Option<RenderGraphSubmitReport>,
    latest_debug: Option<RenderFrameDebugSnapshot>,
    history: Vec<RenderDebugChartSample>,
    resource_buffers: u32,
    resource_textures: u32,
    resource_pipelines: u32,
    queued_upload_jobs: u32,
    queued_upload_bytes: u64,
    view_report: Option<EngineViewDiagnostics>,
    backend_notes: Vec<String>,
}

impl RuntimeOverlayMetrics {
    #[inline]
    pub(super) fn new() -> Self {
        Self {
            frame_triangles: 0,
            frame_draws: 0,
            fps_ema: 0.0,
            initialized: false,
            last_submit: None,
            latest_debug: None,
            history: Vec::new(),
            resource_buffers: 0,
            resource_textures: 0,
            resource_pipelines: 0,
            queued_upload_jobs: 0,
            queued_upload_bytes: 0,
            view_report: None,
            backend_notes: Vec::new(),
        }
    }

    #[inline]
    pub(super) fn reset_interactive_timing(&mut self) {
        self.fps_ema = 0.0;
        self.initialized = false;
        self.history.clear();
    }

    #[inline]
    pub(super) fn begin_frame(&mut self, dt: f32) {
        self.frame_triangles = 0;
        self.frame_draws = 0;
        let dt = if dt.is_finite() && dt > 0.000_001 {
            dt
        } else {
            1.0 / 60.0
        };
        let fps = 1.0 / dt;
        if self.initialized {
            self.fps_ema = self.fps_ema * 0.92 + fps * 0.08;
        } else {
            self.fps_ema = fps;
            self.initialized = true;
        }
    }

    #[inline]
    pub(super) fn record_indexed_triangles(&mut self, index_count: u32) {
        self.frame_triangles = self
            .frame_triangles
            .saturating_add((index_count / 3) as u64);
        self.frame_draws = self.frame_draws.saturating_add(1);
    }

    #[inline]
    pub(super) fn record_vertices_as_triangles(&mut self, vertex_count: u32) {
        self.frame_triangles = self
            .frame_triangles
            .saturating_add((vertex_count / 3) as u64);
        self.frame_draws = self.frame_draws.saturating_add(1);
    }

    #[inline]
    pub(super) fn record_graph_submit(&mut self, report: RenderGraphSubmitReport) {
        self.last_submit = Some(report);
    }

    #[inline]
    pub(super) fn record_view_report(&mut self, report: EngineViewDiagnostics) {
        self.view_report = Some(report);
    }

    #[inline]
    pub(super) fn record_backend_snapshot(&mut self, snapshot: &RenderDiagnosticsSnapshot) {
        self.resource_buffers = snapshot.resources.buffers;
        self.resource_textures = snapshot.resources.textures;
        self.resource_pipelines = snapshot.resources.pipelines;
        self.queued_upload_jobs = snapshot.queue.queued_upload_jobs;
        self.queued_upload_bytes = snapshot.queue.queued_upload_bytes;
        self.backend_notes = snapshot
            .notes
            .iter()
            .rev()
            .take(3)
            .cloned()
            .collect::<Vec<_>>();
        self.backend_notes.reverse();
    }

    pub(super) fn publish_debug_snapshot(&mut self, snapshot: RenderFrameDebugSnapshot) {
        let draw_calls = snapshot.draw_list_stats.iter().fold(0_u32, |acc, stats| {
            acc.saturating_add(stats.draw_calls)
                .saturating_add(stats.indexed_draw_calls)
        });
        let indexed_draw_calls = snapshot.draw_list_stats.iter().fold(0_u32, |acc, stats| {
            acc.saturating_add(stats.indexed_draw_calls)
        });
        let queued_upload_mb = snapshot.queued_upload_bytes as f32 / (1024.0 * 1024.0);

        self.history.push(RenderDebugChartSample {
            frame_index: snapshot.frame_index,
            fps: self.fps_ema,
            cpu_record_ms: snapshot.cpu_record_ms,
            gpu_submit_ms: snapshot.gpu_submit_ms,
            draw_calls,
            indexed_draw_calls,
            triangle_count: self.frame_triangles,
            queued_upload_mb,
        });
        if self.history.len() > DEBUG_HISTORY_CAPACITY {
            let overflow = self.history.len() - DEBUG_HISTORY_CAPACITY;
            self.history.drain(0..overflow);
        }
        self.latest_debug = Some(snapshot);
    }

    #[inline]
    pub(super) fn telemetry_snapshot(&self) -> RenderDebugTelemetry {
        RenderDebugTelemetry {
            latest: self.latest_debug.clone(),
            history: self.history.clone(),
        }
    }

    pub(super) fn overlay_text(&self) -> String {
        let mut lines = Vec::with_capacity(4);
        lines.push(format!(
            "FPS {:>5.1} | TRI {:>8} | DRAWS {:>4}",
            self.fps_ema, self.frame_triangles, self.frame_draws
        ));

        if let Some(view) = self.view_report.as_ref() {
            lines.push(format!(
                "VIEW {}/{} {} dom={:?} n={} lock={} gate={} blend={}{:.0}% events={}",
                view.active_director,
                view.active_mode,
                view.input_context,
                view.dominant_director,
                view.rendered_director_count,
                view.director_lock_input,
                view.gate_blocked,
                if view.frame_blend_active {
                    "on "
                } else {
                    "off "
                },
                view.frame_blend_alpha * 100.0,
                view.pending_event_count,
            ));
            if view.transition.phase != EngineViewTransitionPhase::Idle {
                lines.push(format!(
                    "VIEW transition {:?} {:.2}s target={:?}",
                    view.transition.phase, view.transition.elapsed_sec, view.target_entity,
                ));
            }
        }

        for note in &self.backend_notes {
            lines.push(format!("RENDER WARN {}", compact_note(note, 96)));
        }

        if let Some(report) = self.last_submit.as_ref() {
            lines.push(format!(
                "RG pass {}/{} | cpu {:.2}ms | gpu {:.2}ms",
                report.executed_passes,
                report.skipped_passes,
                report.cpu_record_ms,
                report.gpu_submit_ms,
            ));
            lines.push(format!(
                "RES b:{} t:{} p:{} | UP q:{} {:.2}MB",
                self.resource_buffers,
                self.resource_textures,
                self.resource_pipelines,
                self.queued_upload_jobs,
                self.queued_upload_bytes as f32 / (1024.0 * 1024.0),
            ));
            if !report.draw_list_stats.is_empty() {
                lines.push(format_draw_list_stats(report.draw_list_stats.as_slice()));
                let invalid_draws = report.draw_list_stats.iter().fold(0_u32, |acc, stats| {
                    acc.saturating_add(stats.invalid_draw_calls)
                });
                if invalid_draws > 0 {
                    lines.push(format!("WARN invalid replay draws: {}", invalid_draws));
                }
            }
        }

        lines.join("\n")
    }
}

fn format_draw_list_stats(stats: &[RecordedDrawListStats]) -> String {
    stats
        .iter()
        .map(|it| {
            format!(
                "{} {}/{}",
                compact_draw_list_label(it.draw_list.label()),
                it.draw_calls.saturating_add(it.indexed_draw_calls),
                it.recorded_commands,
            )
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

fn compact_draw_list_label(label: &str) -> &str {
    match label {
        "shadow_casters" => "shadow",
        "opaque_forward" => "opaque",
        "transparent" => "trans",
        "debug" => "debug",
        "ui" => "ui",
        other => other,
    }
}

fn compact_note(note: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for ch in note.chars().take(max_chars) {
        out.push(ch);
    }
    if note.chars().count() > max_chars {
        out.push_str("...");
    }
    out
}
