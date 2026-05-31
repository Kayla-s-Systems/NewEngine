// Split from lib.rs to keep the UI API DTO surface navigable.
// This file is included flat from lib.rs to preserve the existing public API.

/// Provider-neutral retained layout box.
///
/// This is the shared geometry record that must be consumed by paint,
/// hit-testing and interaction dispatch. A widget is not allowed to be visible
/// at one rectangle and clickable at another one.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiLayoutBox {
    pub node_id: String,
    pub surface_id: String,
    /// Global rectangle in physical pixels: [x, y, w, h].
    pub rect: [f32; 4],
    /// Inner content rectangle in physical pixels: [x, y, w, h].
    pub content_rect: [f32; 4],
    /// Effective clip rectangle in physical pixels: [x, y, w, h].
    pub clip_rect: [f32; 4],
    pub scroll_offset: [f32; 2],
    pub z_index: i32,
    pub visible: bool,
    pub interactive: bool,
    /// Provider-resolved style/layout metadata. This remains JSON so the API
    /// does not hard-bind itself to one renderer/provider style object.
    pub computed_style: serde_json::Value,
}

impl Default for UiLayoutBox {
    fn default() -> Self {
        Self {
            node_id: String::new(),
            surface_id: String::new(),
            rect: [0.0, 0.0, 0.0, 0.0],
            content_rect: [0.0, 0.0, 0.0, 0.0],
            clip_rect: [0.0, 0.0, 0.0, 0.0],
            scroll_offset: [0.0, 0.0],
            z_index: 0,
            visible: true,
            interactive: false,
            computed_style: serde_json::Value::Null,
        }
    }
}

impl UiLayoutBox {
    #[inline]
    pub fn contains_global_point(&self, point: (f32, f32)) -> bool {
        rect_contains_point(self.rect, point) && rect_contains_point(self.clip_rect, point)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiLayoutFrame {
    pub version: u32,
    pub surface_id: String,
    pub boxes: Vec<UiLayoutBox>,
    pub diagnostics: Vec<String>,
}

impl Default for UiLayoutFrame {
    fn default() -> Self {
        Self {
            version: 1,
            surface_id: String::new(),
            boxes: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}

#[inline]
pub fn rect_contains_point(rect: [f32; 4], point: (f32, f32)) -> bool {
    let (x, y) = point;
    x >= rect[0] && x <= rect[0] + rect[2] && y >= rect[1] && y <= rect[1] + rect[3]
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct UiSurfaceNodeLayout {
    pub screen_w: f32,
    pub screen_h: f32,
    pub panel_x: f32,
    pub panel_y: f32,
    pub panel_w: f32,
    pub panel_h: f32,
    pub body_x: f32,
    pub body_y: f32,
    pub body_w: f32,
    pub body_line_pitch: f32,
    pub footer_y: f32,
    pub large_panel: bool,
}

impl UiSurfaceNodeLayout {
    #[inline]
    pub fn hit_body_line_index(self, mouse_pos: Option<(f32, f32)>) -> Option<usize> {
        let (mx, my) = mouse_pos?;
        if mx < self.body_x || mx > self.body_x + self.body_w || my < self.body_y {
            return None;
        }
        if my > self.panel_y + self.panel_h - 56.0 {
            return None;
        }
        let idx = ((my - self.body_y) / self.body_line_pitch).floor() as isize;
        if idx < 0 { None } else { Some(idx as usize) }
    }

    #[inline]
    pub fn hit_item_index_after_header(self, mouse_pos: Option<(f32, f32)>, header_lines: usize, item_count: usize) -> Option<usize> {
        let line = self.hit_body_line_index(mouse_pos)?;
        let item = line.checked_sub(header_lines)?;
        if item < item_count { Some(item) } else { None }
    }
}

#[inline]
pub fn ui_surface_node_layout(
    surface_size_px: [u32; 2],
    style_tags: &[String],
    style: &UiSurfaceStyle,
    body_line_count: usize,
    footer_line_count: usize,
) -> UiSurfaceNodeLayout {
    let w = surface_size_px[0].max(1) as f32;
    let h = surface_size_px[1].max(1) as f32;
    let style = style.clone().normalized();

    let docked = style_tags.iter().any(|tag| tag == "dock-bottom" || tag == "dock-left" || tag == "dock-right" || tag == "dock-top" || tag == "docked-panel");
    let workspace = !docked && style_tags.iter().any(|tag| tag == "workspace" || tag == "fullscreen");
    let available_w = (w - style.margin_px[0] * 2.0).max(style.min_size_px[0]);
    let available_h = (h - style.margin_px[1] * 2.0).max(style.min_size_px[1]);
    let line_count = body_line_count.max(1) + footer_line_count + 2;
    let content_h = line_count as f32 * style.row_pitch_px.max(style.font.line_height_px).max(24.0)
        + style.padding_px[1] + style.padding_px[3] + 10.0;
    let panel_w = if workspace {
        available_w
    } else {
        style.max_size_px[0].min(available_w).max(style.min_size_px[0])
    };
    let panel_h = if workspace {
        available_h
    } else {
        style.max_size_px[1].min(available_h).max(style.min_size_px[1]).max(content_h.min(available_h))
    };

    let panel_x = match style.anchor {
        UiSurfaceAnchor::TopRight | UiSurfaceAnchor::BottomRight => (w - panel_w - style.margin_px[0]).max(style.margin_px[0]),
        UiSurfaceAnchor::Center => ((w - panel_w) * 0.5).max(style.margin_px[0]),
        UiSurfaceAnchor::TopLeft | UiSurfaceAnchor::BottomLeft => style.margin_px[0],
    };
    let panel_y = match style.anchor {
        UiSurfaceAnchor::BottomLeft | UiSurfaceAnchor::BottomRight => (h - panel_h - style.margin_px[1]).max(style.margin_px[1]),
        UiSurfaceAnchor::Center => ((h - panel_h) * 0.5).max(style.margin_px[1]),
        UiSurfaceAnchor::TopLeft | UiSurfaceAnchor::TopRight => style.margin_px[1],
    };

    let raw_line_h = if style.font.line_height_px > 0.0 { style.font.line_height_px } else { 24.0 };
    let line_pitch = style.row_pitch_px.max(raw_line_h + 2.0);
    let large_panel = panel_h >= 360.0 || panel_w >= 420.0;
    UiSurfaceNodeLayout {
        screen_w: w,
        screen_h: h,
        panel_x,
        panel_y,
        panel_w,
        panel_h,
        body_x: panel_x + style.padding_px[0],
        body_y: panel_y + style.padding_px[1],
        body_w: (panel_w - style.padding_px[0] - style.padding_px[2]).max(32.0),
        body_line_pitch: line_pitch,
        footer_y: panel_y + panel_h - style.padding_px[3] + 4.0,
        large_panel,
    }
}
