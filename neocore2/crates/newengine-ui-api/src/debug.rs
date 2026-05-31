// Split from lib.rs to keep the UI API DTO surface navigable.
// This file is included flat from lib.rs to preserve the existing public API.

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiRuntimeDebugOverlayTelemetry {
    pub version: u32,
    pub surface_id: String,
    pub source: String,
    pub frame_index: u64,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub lines: Vec<String>,
    #[serde(default)]
    pub metrics: BTreeMap<String, serde_json::Value>,
}


impl Default for UiRuntimeDebugOverlayTelemetry {
    fn default() -> Self {
        Self {
            version: 1,
            surface_id: UI_SURFACE_RUNTIME_DEBUG_OVERLAY.to_owned(),
            source: String::new(),
            frame_index: 0,
            text: String::new(),
            lines: Vec::new(),
            metrics: BTreeMap::new(),
        }
    }
}

impl UiRuntimeDebugOverlayTelemetry {
    #[inline]
    pub fn new(frame_index: u64, text: impl Into<String>) -> Self {
        let text = text.into();
        let lines = text.lines().map(str::to_owned).collect();
        Self {
            version: 1,
            surface_id: UI_SURFACE_RUNTIME_DEBUG_OVERLAY.to_owned(),
            source: "engine.runtime.debug".to_owned(),
            frame_index,
            text,
            lines,
            metrics: BTreeMap::new(),
        }
    }

    #[inline]
    pub fn with_metric(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metrics.insert(key.into(), value);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiDebugTreeRequest {
    pub version: u32,
    /// Empty means all surfaces. Otherwise the provider filters to this surface.
    pub surface_id: String,
    /// Optional node selected by a DevTools surface. Empty means "use hovered/focused".
    pub selected_node_id: String,
    /// Optional pointer position used by inspectors that want an explicit hit-test.
    pub pointer_pos: Option<[f32; 2]>,
    pub surface_size_px: [u32; 2],
    pub pixels_per_point: f32,
    pub include_invisible: bool,
    pub include_bindings: bool,
    pub include_draw_cost: bool,
    pub include_overlays: bool,
    pub include_style_cascade: bool,
    pub include_action_log: bool,
    pub include_atlas: bool,
}

impl Default for UiDebugTreeRequest {
    fn default() -> Self {
        Self {
            version: 1,
            surface_id: String::new(),
            selected_node_id: String::new(),
            pointer_pos: None,
            surface_size_px: [1280, 720],
            pixels_per_point: 1.0,
            include_invisible: false,
            include_bindings: true,
            include_draw_cost: true,
            include_overlays: true,
            include_style_cascade: true,
            include_action_log: true,
            include_atlas: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiDebugDrawCost {
    pub draw_cmds: u32,
    pub vertices: u32,
    pub indices: u32,
    pub estimated_quads: u32,
    pub texture_sets: u32,
    pub texture_patches: u32,
    pub texture_frees: u32,
}

impl Default for UiDebugDrawCost {
    fn default() -> Self {
        Self {
            draw_cmds: 0,
            vertices: 0,
            indices: 0,
            estimated_quads: 0,
            texture_sets: 0,
            texture_patches: 0,
            texture_frees: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiDebugNode {
    pub node_id: String,
    pub surface_id: String,
    pub parent_id: Option<String>,
    pub kind: String,
    pub role: String,
    pub source_span: Option<UiSourceSpan>,
    pub rect: [f32; 4],
    pub content_rect: [f32; 4],
    pub clip_rect: [f32; 4],
    pub scroll_offset: [f32; 2],
    pub z_index: i32,
    pub visible: bool,
    pub interactive: bool,
    pub computed_style: serde_json::Value,
    pub hovered: bool,
    pub focused: bool,
    pub pressed: bool,
    pub captured: bool,
    pub action_id: Option<String>,
    pub style_tags: Vec<String>,
    pub style_classes: Vec<String>,
    pub theme_id: String,
    pub density: String,
    pub top_layer: bool,
    pub top_layer_reason: String,
    pub z_path: Vec<String>,
    pub bindings: Vec<UiBindingEdge>,
    pub actions: Vec<UiActionEdge>,
    pub children: Vec<String>,
    pub draw_cost: UiDebugDrawCost,
}

impl Default for UiDebugNode {
    fn default() -> Self {
        Self {
            node_id: String::new(),
            surface_id: String::new(),
            parent_id: None,
            kind: String::new(),
            role: String::new(),
            source_span: None,
            rect: [0.0, 0.0, 0.0, 0.0],
            content_rect: [0.0, 0.0, 0.0, 0.0],
            clip_rect: [0.0, 0.0, 0.0, 0.0],
            scroll_offset: [0.0, 0.0],
            z_index: 0,
            visible: true,
            interactive: false,
            computed_style: serde_json::Value::Null,
            hovered: false,
            focused: false,
            pressed: false,
            captured: false,
            action_id: None,
            style_tags: Vec::new(),
            style_classes: Vec::new(),
            theme_id: String::new(),
            density: String::new(),
            top_layer: false,
            top_layer_reason: String::new(),
            z_path: Vec::new(),
            bindings: Vec::new(),
            actions: Vec::new(),
            children: Vec::new(),
            draw_cost: UiDebugDrawCost::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiDebugZOrderEntry {
    pub surface_id: String,
    pub node_id: String,
    pub z_index: i32,
    pub top_layer: bool,
    pub top_layer_reason: String,
    pub modal: bool,
    pub modal_reason: String,
    pub visible: bool,
    pub rect: [f32; 4],
}

impl Default for UiDebugZOrderEntry {
    fn default() -> Self {
        Self {
            surface_id: String::new(),
            node_id: String::new(),
            z_index: 0,
            top_layer: false,
            top_layer_reason: String::new(),
            modal: false,
            modal_reason: String::new(),
            visible: true,
            rect: [0.0, 0.0, 0.0, 0.0],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiDebugTextureAtlasPage {
    pub atlas_id: String,
    pub texture_id: u32,
    pub size_px: [u32; 2],
    pub glyph_cell_px: [u32; 2],
    pub page_kind: String,
    pub active_text_backend: String,
    pub face_ref: String,
    pub glyph_count: u32,
    pub real_glyphs: u32,
    pub synthetic_fallback_glyphs: u32,
    pub provider_owned: bool,
    pub diagnostics: Vec<String>,
}

impl Default for UiDebugTextureAtlasPage {
    fn default() -> Self {
        Self {
            atlas_id: String::new(),
            texture_id: 0,
            size_px: [0, 0],
            glyph_cell_px: [0, 0],
            page_kind: String::new(),
            active_text_backend: String::new(),
            face_ref: String::new(),
            glyph_count: 0,
            real_glyphs: 0,
            synthetic_fallback_glyphs: 0,
            provider_owned: true,
            diagnostics: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiDebugInputEvent {
    pub frame_index: u64,
    pub event_kind: String,
    pub surface_id: String,
    pub node_id: String,
    pub action_id: Option<String>,
    pub pointer: [f32; 2],
    pub button: Option<u32>,
    pub wheel: [f32; 2],
    pub reason: String,
}

impl Default for UiDebugInputEvent {
    fn default() -> Self {
        Self {
            frame_index: 0,
            event_kind: String::new(),
            surface_id: String::new(),
            node_id: String::new(),
            action_id: None,
            pointer: [0.0, 0.0],
            button: None,
            wheel: [0.0, 0.0],
            reason: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiDevToolsOverlayFrame {
    pub version: u32,
    pub selected_node_id: String,
    pub selected_node: Option<UiDebugNode>,
    pub selected_layout_box: Option<UiLayoutBox>,
    pub selected_style: serde_json::Value,
    pub layout_boxes: Vec<UiLayoutBox>,
    pub hit_test: Option<UiHitTestResult>,
    pub hit_test_stack: Vec<UiHitTestResult>,
    pub last_pointer_event: Option<UiDebugInputEvent>,
    pub focus_chain: Vec<UiHitTestResult>,
    pub z_order: Vec<UiDebugZOrderEntry>,
    pub style_cascade: Vec<serde_json::Value>,
    pub binding_graph: Vec<UiBindingPlan>,
    pub action_dispatch_log: Vec<UiActionDispatch>,
    pub input_capture: UiPointerCaptureState,
    pub draw_cost: UiDebugDrawCost,
    pub active_text_backend: String,
    pub texture_atlas: Vec<UiDebugTextureAtlasPage>,
    pub atlas_diagnostics: Vec<String>,
    pub diagnostics: Vec<String>,
}

impl Default for UiDevToolsOverlayFrame {
    fn default() -> Self {
        Self {
            version: 1,
            selected_node_id: String::new(),
            selected_node: None,
            selected_layout_box: None,
            selected_style: serde_json::Value::Null,
            layout_boxes: Vec::new(),
            hit_test: None,
            hit_test_stack: Vec::new(),
            last_pointer_event: None,
            focus_chain: Vec::new(),
            z_order: Vec::new(),
            style_cascade: Vec::new(),
            binding_graph: Vec::new(),
            action_dispatch_log: Vec::new(),
            input_capture: UiPointerCaptureState::default(),
            draw_cost: UiDebugDrawCost::default(),
            active_text_backend: String::new(),
            texture_atlas: Vec::new(),
            atlas_diagnostics: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}
