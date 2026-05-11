#![forbid(unsafe_op_in_unsafe_fn)]

#[derive(Clone, Copy, Debug)]
pub(super) struct RuntimeOverlayMetrics {
    pub(super) frame_triangles: u64,
    pub(super) frame_draws: u32,
    fps_ema: f32,
    initialized: bool,
}

impl RuntimeOverlayMetrics {
    #[inline]
    pub(super) const fn new() -> Self {
        Self {
            frame_triangles: 0,
            frame_draws: 0,
            fps_ema: 0.0,
            initialized: false,
        }
    }

    #[inline]
    pub(super) fn begin_frame(&mut self, dt: f32) {
        self.frame_triangles = 0;
        self.frame_draws = 0;
        let dt = if dt.is_finite() && dt > 0.000_001 { dt } else { 1.0 / 60.0 };
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
        self.frame_triangles = self.frame_triangles.saturating_add((index_count / 3) as u64);
        self.frame_draws = self.frame_draws.saturating_add(1);
    }

    #[inline]
    pub(super) fn record_vertices_as_triangles(&mut self, vertex_count: u32) {
        self.frame_triangles = self.frame_triangles.saturating_add((vertex_count / 3) as u64);
        self.frame_draws = self.frame_draws.saturating_add(1);
    }

    #[inline]
    pub(super) fn overlay_text(&self) -> String {
        format!(
            "FPS {:>5.1} | TRI {:>8} | DRAWS {:>4}",
            self.fps_ema,
            self.frame_triangles,
            self.frame_draws
        )
    }
}
