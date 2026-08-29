use super::*;

impl RuntimeRenderController {
    pub(super) fn trace_render_begin(&self, trace_frame: bool, w: u32, h: u32) {
        if !trace_frame {
            return;
        }
        let (requested_vp_w, requested_vp_h) = self.bridges.viewport.read_extent();
        let direct_surface_viewport = requested_vp_w == 0 && requested_vp_h == 0 && w > 0 && h > 0;
        let (vp_w, vp_h) = if direct_surface_viewport {
            (w, h)
        } else {
            (requested_vp_w, requested_vp_h)
        };
        newengine_ulog_api::ulog::debug!(
            "render controller: render begin next_frame={} window={}x{} viewport={}x{} direct_surface={}",
            self.frame.frame_index.saturating_add(1),
            w,
            h,
            vp_w,
            vp_h,
            direct_surface_viewport
        );
        newengine_core::crash::record_breadcrumb(format!(
            "render controller: render begin next_frame={} window={}x{} viewport={}x{}",
            self.frame.frame_index.saturating_add(1),
            w,
            h,
            vp_w,
            vp_h
        ));
    }

    pub(super) fn trace_begin_frame(&self, trace_frame: bool, vp_w: u32, vp_h: u32) {
        if !trace_frame {
            return;
        }
        newengine_ulog_api::ulog::debug!(
            "render controller: begin_frame next_frame={} clear={:.3},{:.3},{:.3},{:.3} viewport={}x{}",
            self.frame.frame_index.saturating_add(1),
            self.viewport.clear_color[0],
            self.viewport.clear_color[1],
            self.viewport.clear_color[2],
            self.viewport.clear_color[3],
            vp_w,
            vp_h
        );
    }

    pub(super) fn trace_begin_frame_done(&self, trace_frame: bool) {
        if !trace_frame {
            return;
        }
        newengine_ulog_api::ulog::debug!(
            "render controller: begin_frame completed frame={}",
            self.frame.frame_index.saturating_add(1)
        );
        newengine_core::crash::record_breadcrumb(format!(
            "render controller: begin_frame completed frame={}",
            self.frame.frame_index.saturating_add(1)
        ));
    }

    pub(super) fn trace_render_diagnostics(
        &self,
        r: &mut dyn newengine_core::render::RenderApi,
        trace_frame: bool,
    ) {
        if !trace_frame {
            return;
        }
        if let Ok(diag) = r.diagnostics_snapshot() {
            newengine_ulog_api::ulog::debug!(
                "render diagnostics: frame={} begin_ms={:.3} end_ms={:.3} upload_ms={:.3} pipeline_ms={:.3} buffers={} textures={} pipelines={} upload_jobs={} upload_mb={:.2} queued_uploads={} queued_mb={:.2}",
                diag.frame.frame_index,
                diag.frame.last_begin_frame_ms,
                diag.frame.last_end_frame_ms,
                diag.frame.last_blocking_upload_ms,
                diag.frame.last_pipeline_build_ms,
                diag.resources.buffers,
                diag.resources.textures,
                diag.resources.pipelines,
                diag.queue.blocking_upload_jobs,
                diag.queue.blocking_upload_bytes as f32 / (1024.0 * 1024.0),
                diag.queue.queued_upload_jobs,
                diag.queue.queued_upload_bytes as f32 / (1024.0 * 1024.0),
            );
        }
    }
}
