use serde::{Deserialize, Serialize};

use super::{
    logo_layout::layout_logo_rects,
    profile::{LoadingVisualRole, ResolvedLoadingAssignment},
};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BootViewport {
    pub width: f32,
    pub height: f32,
    pub scale: f32,
}

impl Default for BootViewport {
    #[inline]
    fn default() -> Self {
        Self {
            width: 1600.0,
            height: 900.0,
            scale: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColorRgba8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl ColorRgba8 {
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BootRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl BootRect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BootTextRun {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub size_px: f32,
    pub color: ColorRgba8,
}

impl BootTextRun {
    pub fn new(text: impl Into<String>, x: f32, y: f32, size_px: f32, color: ColorRgba8) -> Self {
        Self {
            text: text.into(),
            x,
            y,
            size_px,
            color,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BootDrawCommand {
    Clear {
        color: ColorRgba8,
    },
    Rect {
        rect: BootRect,
        color: ColorRgba8,
    },
    Text {
        run: BootTextRun,
    },
    Image {
        role: LoadingVisualRole,
        texture_ref: String,
        rect: BootRect,
        alpha: f32,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoadingProgressSnapshot {
    pub progress_01: f32,
    pub spinner_phase: u32,
    pub status: String,
    pub detail: String,
}

impl LoadingProgressSnapshot {
    pub fn new(
        progress_01: f32,
        spinner_phase: u32,
        status: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            progress_01: progress_01.clamp(0.0, 1.0),
            spinner_phase,
            status: status.into(),
            detail: detail.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BootFrameDto {
    pub assignment: ResolvedLoadingAssignment,
    pub viewport: BootViewport,
    pub clear: ColorRgba8,
    pub commands: Vec<BootDrawCommand>,
    pub progress: LoadingProgressSnapshot,
    pub diagnostics: Vec<String>,
}

impl BootFrameDto {
    #[allow(clippy::too_many_arguments)]
    pub fn from_status(
        assignment: ResolvedLoadingAssignment,
        viewport: BootViewport,
        title: impl Into<String>,
        status: impl Into<String>,
        detail: impl Into<String>,
        progress_01: f32,
        spinner_phase: u32,
    ) -> Self {
        let title = normalize_text(title.into(), "NORTH STAR ENGINE");
        let status = normalize_text(status.into(), "Preparing runtime...");
        let detail = normalize_text(detail.into(), "Boot-safe loading presenter is active.");
        let progress = LoadingProgressSnapshot::new(
            progress_01,
            spinner_phase,
            status.clone(),
            detail.clone(),
        );
        let clear = ColorRgba8::rgba(8, 12, 20, 255);
        let track = ColorRgba8::rgba(35, 45, 62, 255);
        let fill = ColorRgba8::rgba(96, 165, 250, 255);
        let text = ColorRgba8::rgba(235, 245, 255, 255);
        let muted = ColorRgba8::rgba(148, 163, 184, 255);

        let safe_w = finite_positive(viewport.width, 1.0);
        let safe_h = finite_positive(viewport.height, 1.0);
        let viewport = BootViewport {
            width: safe_w,
            height: safe_h,
            scale: finite_positive(viewport.scale, 1.0),
        };

        let bar_w_max = (safe_w - 24.0).clamp(1.0, 780.0);
        let bar_w_min = 480.0_f32.min(bar_w_max);
        let bar_w = (safe_w * 0.42).clamp(bar_w_min, bar_w_max);
        let bar_h = safe_h.clamp(1.0, 8.0);
        let bar_x = ((safe_w - bar_w) * 0.5).max(0.0);
        let bar_y_max = (safe_h - 90.0).max(0.0);
        let bar_y_min = 420.0_f32.min(bar_y_max);
        let bar_y = (safe_h * 0.72).clamp(bar_y_min, bar_y_max);
        let fill_w = bar_w * progress.progress_01.clamp(0.0, 1.0);

        let shortest_side = safe_w.min(safe_h);
        let spinner_size = shortest_side.clamp(1.0, 64.0);

        let mut commands = vec![BootDrawCommand::Clear { color: clear }];

        if let Some(texture_ref) = assignment
            .visuals
            .background
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            commands.push(BootDrawCommand::Image {
                role: LoadingVisualRole::Background,
                texture_ref: texture_ref.to_owned(),
                rect: BootRect::new(0.0, 0.0, safe_w, safe_h),
                alpha: 1.0,
            });
        }

        let logo_refs = assignment.visuals.logo_refs();
        let logo_rects = layout_logo_rects(viewport, logo_refs.len());
        for (texture_ref, rect) in logo_refs.into_iter().zip(logo_rects) {
            commands.push(BootDrawCommand::Image {
                role: LoadingVisualRole::Logo,
                texture_ref: texture_ref.to_owned(),
                rect,
                alpha: 1.0,
            });
        }

        if let Some(texture_ref) = assignment
            .visuals
            .spinner
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            commands.push(BootDrawCommand::Image {
                role: LoadingVisualRole::Spinner,
                texture_ref: texture_ref.to_owned(),
                rect: BootRect::new(
                    (safe_w - spinner_size) * 0.5,
                    bar_y + 26.0,
                    spinner_size,
                    spinner_size,
                ),
                alpha: 1.0,
            });
        }

        commands.extend([
            BootDrawCommand::Text {
                run: BootTextRun::new(title, bar_x, bar_y - 92.0, 24.0, text),
            },
            BootDrawCommand::Text {
                run: BootTextRun::new(status, bar_x, bar_y - 54.0, 16.0, text),
            },
            BootDrawCommand::Text {
                run: BootTextRun::new(detail, bar_x, bar_y - 28.0, 14.0, muted),
            },
            BootDrawCommand::Rect {
                rect: BootRect::new(bar_x, bar_y, bar_w, bar_h),
                color: track,
            },
            BootDrawCommand::Rect {
                rect: BootRect::new(bar_x, bar_y, fill_w, bar_h),
                color: fill,
            },
        ]);

        let diagnostics = vec![assignment.visuals.diagnostic_summary()];

        Self {
            assignment,
            viewport,
            clear,
            commands,
            progress,
            diagnostics,
        }
    }

    #[inline]
    pub fn diagnostic_summary(&self) -> String {
        format!(
            "boot_frame assignment='{}' phase='{}' commands={} image_layers={} viewport={:.0}x{:.0} progress={:.0}%",
            self.assignment.assignment_id,
            self.assignment.phase.as_str(),
            self.commands.len(),
            self.assignment.visuals.image_layer_count(),
            self.viewport.width,
            self.viewport.height,
            self.progress.progress_01 * 100.0
        )
    }
}

#[inline]
fn finite_positive(value: f32, fallback: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        fallback
    }
}

#[inline]
fn normalize_text(value: String, fallback: &'static str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback.to_owned()
    } else {
        trimmed.to_owned()
    }
}
