// Split from lib.rs to keep the UI API DTO surface navigable.
// This file is included flat from lib.rs to preserve the existing public API.

/// Global editor selection routed through editor surfaces.
///
/// Content Browser, scene outliner, viewport picking and future material graph
/// panels publish this DTO. The right edit window consumes it and chooses the
/// correct backend route (`engine.entity`, `engine.assets.inspect`, etc.) without
/// becoming a format parser or an entity-only inspector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditorSelectionKind {
    None,
    Entity,
    Asset,
    AssetEntry,
    World,
    Material,
}

impl Default for EditorSelectionKind {
    #[inline]
    fn default() -> Self { Self::None }
}

impl EditorSelectionKind {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Entity => "entity",
            Self::Asset => "asset",
            Self::AssetEntry => "asset_entry",
            Self::World => "world",
            Self::Material => "material",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EditorSelectionContext {
    pub version: u32,
    pub kind: EditorSelectionKind,
    #[serde(rename = "ref")]
    pub reference: String,
    pub label: String,
    pub source_surface: String,
    pub source_node: String,
    pub semantic_gateway: String,
    pub data_contract: String,
}

impl Default for EditorSelectionContext {
    fn default() -> Self { Self::none() }
}

impl EditorSelectionContext {
    #[inline]
    pub fn none() -> Self {
        Self {
            version: 1,
            kind: EditorSelectionKind::None,
            reference: String::new(),
            label: String::new(),
            source_surface: String::new(),
            source_node: String::new(),
            semantic_gateway: String::new(),
            data_contract: "newengine.editor.selection_context.v1".to_owned(),
        }
    }

    #[inline]
    pub fn asset(reference: impl Into<String>, label: impl Into<String>, source_surface: impl Into<String>, semantic_gateway: impl Into<String>) -> Self {
        let reference = reference.into();
        Self {
            version: 1,
            kind: if reference.contains('@') { EditorSelectionKind::AssetEntry } else { EditorSelectionKind::Asset },
            reference,
            label: label.into(),
            source_surface: source_surface.into(),
            source_node: String::new(),
            semantic_gateway: semantic_gateway.into(),
            data_contract: "newengine.assets.document.v1".to_owned(),
        }
    }
}

/// Generic UI event model emitted by `engine.ui` hit-testing/focus logic.
///
/// This is not tied to a concrete UI provider. Product/UI compositions can use
/// the same DTO shape for hover/click/drag/wheel, focus routing, modal stack
/// diagnostics and z-order visibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiPointerEventKind {
    Hover,
    Press,
    Release,
    Click,
    DoubleClick,
    DragStart,
    DragMove,
    DragEnd,
    Wheel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiHitTestTarget {
    pub surface_id: String,
    pub node_id: String,
    pub action_id: Option<String>,
    pub z_order: i32,
    pub rect_px: [f32; 4],
    pub tags: Vec<String>,
}

impl Default for UiHitTestTarget {
    fn default() -> Self {
        Self {
            surface_id: String::new(),
            node_id: String::new(),
            action_id: None,
            z_order: 0,
            rect_px: [0.0, 0.0, 0.0, 0.0],
            tags: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiPointerEvent {
    pub kind: UiPointerEventKind,
    pub target: Option<UiHitTestTarget>,
    pub position_px: Option<(f32, f32)>,
    pub delta_px: (f32, f32),
    pub wheel_delta: (f32, f32),
    pub button: Option<u32>,
}

impl Default for UiPointerEvent {
    fn default() -> Self {
        Self {
            kind: UiPointerEventKind::Hover,
            target: None,
            position_px: None,
            delta_px: (0.0, 0.0),
            wheel_delta: (0.0, 0.0),
            button: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiFocusGraphState {
    pub active_surface: Option<String>,
    pub focused_node: Option<String>,
    pub hovered_node: Option<String>,
    pub capture_reason: String,
}

impl Default for UiFocusGraphState {
    fn default() -> Self {
        Self {
            active_surface: None,
            focused_node: None,
            hovered_node: None,
            capture_reason: "none".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiModalStackEntry {
    pub surface_id: String,
    pub modal: bool,
    pub z_order: i32,
    pub reason: String,
}

impl Default for UiModalStackEntry {
    fn default() -> Self {
        Self {
            surface_id: String::new(),
            modal: false,
            z_order: 0,
            reason: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct UiEventModelFrame {
    pub focus: UiFocusGraphState,
    pub modal_stack: Vec<UiModalStackEntry>,
    pub pointer_events: Vec<UiPointerEvent>,
    pub hit_test: Option<UiHitTestTarget>,
}

/// Event route phase used by the retained interaction dispatcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiEventPhase {
    Capture,
    Target,
    Bubble,
}

impl Default for UiEventPhase {
    #[inline]
    fn default() -> Self { Self::Target }
}

/// Provider-neutral hit-test result produced by `ui.dispatch_input_v1`.
///
/// This represents a framework node hit, not a product-specific rectangle. The
/// provider is free to compute the layout box from authored `.neui`, runtime
/// `UiSurfaceNode` data, or generated node trees, but every consumer receives
/// the same shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiHitTestResult {
    pub surface_id: String,
    pub node_id: String,
    pub role: String,
    pub action_id: Option<String>,
    pub local_pos: [f32; 2],
    pub global_rect: [f32; 4],
    pub z_path: Vec<String>,
    pub event_phase: UiEventPhase,
    /// Authored/generated event routes attached to the hit node.
    ///
    /// This remains provider-neutral data: the retained dispatcher emits these
    /// routes as `UiActionDispatch` records; runtime decides delivery through
    /// `engine.*` gateways.
    pub event_routes: Vec<UiNodeEventRoute>,
}

impl Default for UiHitTestResult {
    fn default() -> Self {
        Self {
            surface_id: String::new(),
            node_id: String::new(),
            role: String::new(),
            action_id: None,
            local_pos: [0.0, 0.0],
            global_rect: [0.0, 0.0, 0.0, 0.0],
            z_path: Vec::new(),
            event_phase: UiEventPhase::Target,
            event_routes: Vec::new(),
        }
    }
}

/// Action emitted by the retained interaction dispatcher.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiActionDispatch {
    pub surface_id: String,
    pub node_id: String,
    pub action_id: String,
    pub trigger: UiNodeEventTrigger,
    pub target_gateway: String,
    pub method: String,
    pub payload: serde_json::Value,
}

impl Default for UiActionDispatch {
    fn default() -> Self {
        Self {
            surface_id: String::new(),
            node_id: String::new(),
            action_id: String::new(),
            trigger: UiNodeEventTrigger::Click,
            target_gateway: ENGINE_UI_SERVICE_ID.to_owned(),
            method: UI_SERVICE_METHOD_DISPATCH_ACTION_V1.to_owned(),
            payload: serde_json::Value::Null,
        }
    }
}

/// Resolved provider-owned pointer capture state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiPointerCaptureState {
    pub active: bool,
    pub owner_surface_id: String,
    pub owner_node_id: String,
    pub button: Option<u32>,
    pub reason: String,
}

impl Default for UiPointerCaptureState {
    fn default() -> Self {
        Self {
            active: false,
            owner_surface_id: String::new(),
            owner_node_id: String::new(),
            button: None,
            reason: "none".to_owned(),
        }
    }
}

/// Result frame for `ui.dispatch_input_v1`.
///
/// This is the retained UI framework spine: hit-testing, hover/focus, pointer
/// capture, event route, action dispatch and optional state patches are reported
/// in one DTO. Product UI should consume this instead of calculating private
/// rectangles and per-widget mouse state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiEventDispatchFrame {
    pub version: u32,
    pub frame_index: u64,
    pub hovered_node: Option<UiHitTestResult>,
    pub focused_node: Option<UiHitTestResult>,
    pub captured_pointer_owner: Option<UiHitTestResult>,
    pub actions: Vec<UiActionDispatch>,
    pub state_patches: Vec<UiStatePatch>,
    pub capture_state: UiPointerCaptureState,
    pub diagnostics: Vec<String>,
}

impl Default for UiEventDispatchFrame {
    fn default() -> Self {
        Self {
            version: 1,
            frame_index: 0,
            hovered_node: None,
            focused_node: None,
            captured_pointer_owner: None,
            actions: Vec::new(),
            state_patches: Vec::new(),
            capture_state: UiPointerCaptureState::default(),
            diagnostics: Vec::new(),
        }
    }
}
