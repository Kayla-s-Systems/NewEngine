// Split from lib.rs to keep the UI API DTO surface navigable.
// This file is included flat from lib.rs to preserve the existing public API.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiSurfaceAdmissionPolicy {
    /// Default retained-UI behavior: this surface does not block other surfaces
    /// from being created or opened.
    AcceptAll,
    /// While this surface is visible, the active UI provider must reject new
    /// visible surfaces with a different surface id. This is an explicit UI-node
    /// policy, not a provider-specific hardcoded branch.
    AcceptAllButThis,
}

impl Default for UiSurfaceAdmissionPolicy {
    #[inline]
    fn default() -> Self { Self::AcceptAll }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiSurfaceNode {
    pub version: u32,
    pub surface_id: String,
    pub source: String,
    pub visible: bool,
    pub modal: bool,
    pub z_order: i32,
    pub title: String,
    pub subtitle: String,
    pub body_lines: Vec<String>,
    pub footer_lines: Vec<String>,
    pub style_tags: Vec<String>,
    pub theme_id: String,
    pub style_ref: Option<String>,
    pub component_id: String,
    pub components: Vec<UiComponentNode>,
    pub message: Option<UiNodeMessage>,
    pub style: UiSurfaceStyle,
    pub admission_policy: UiSurfaceAdmissionPolicy,
    pub metrics: BTreeMap<String, serde_json::Value>,
}

impl Default for UiSurfaceNode {
    fn default() -> Self {
        Self {
            version: 1,
            surface_id: String::new(),
            source: String::new(),
            visible: true,
            modal: false,
            z_order: 0,
            title: String::new(),
            subtitle: String::new(),
            body_lines: Vec::new(),
            footer_lines: Vec::new(),
            style_tags: Vec::new(),
            theme_id: UI_THEME_NORTHSTAR_DEFAULT.to_owned(),
            style_ref: None,
            component_id: UI_COMPONENT_PANEL.to_owned(),
            components: Vec::new(),
            message: None,
            style: UiSurfaceStyle::default(),
            admission_policy: UiSurfaceAdmissionPolicy::default(),
            metrics: BTreeMap::new(),
        }
    }
}

impl UiSurfaceNode {
    #[inline]
    pub fn new(surface_id: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            surface_id: surface_id.into(),
            source: source.into(),
            ..Self::default()
        }
    }

    #[inline]
    pub fn hidden(surface_id: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            visible: false,
            surface_id: surface_id.into(),
            source: source.into(),
            ..Self::default()
        }
    }

    #[inline]
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    #[inline]
    pub fn with_subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = subtitle.into();
        self
    }

    #[inline]
    pub fn with_body_lines(mut self, lines: Vec<String>) -> Self {
        self.body_lines = lines;
        self
    }

    #[inline]
    pub fn with_footer_lines(mut self, lines: Vec<String>) -> Self {
        self.footer_lines = lines;
        self
    }

    #[inline]
    pub fn with_theme(mut self, theme_id: impl Into<String>) -> Self {
        self.theme_id = theme_id.into();
        self.style.theme_id = self.theme_id.clone();
        self
    }

    #[inline]
    pub fn with_component(mut self, component_id: impl Into<String>) -> Self {
        self.component_id = component_id.into();
        self
    }

    #[inline]
    pub fn with_style_ref(mut self, style_ref: impl Into<String>) -> Self {
        self.style_ref = Some(style_ref.into());
        self
    }

    #[inline]
    pub fn with_components(mut self, components: Vec<UiComponentNode>) -> Self {
        self.components = components;
        self
    }

    #[inline]
    pub fn with_message(mut self, message: UiNodeMessage) -> Self {
        self.message = Some(message);
        self
    }

    #[inline]
    pub fn with_style(mut self, style: UiSurfaceStyle) -> Self {
        self.style = style.normalized();
        self.theme_id = self.style.theme_id.clone();
        self
    }

    #[inline]
    pub fn with_admission_policy(mut self, policy: UiSurfaceAdmissionPolicy) -> Self {
        self.admission_policy = policy;
        self
    }

    #[inline]
    pub fn with_metric(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metrics.insert(key.into(), value);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct UiRegistryLoadRequest {
    pub registry_ref: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiMountSurfaceRequest {
    pub surface_id: String,
    pub document: UiCompiledDocument,
    pub visible: bool,
}
impl Default for UiMountSurfaceRequest { fn default() -> Self { Self { surface_id: String::new(), document: UiCompiledDocument::default(), visible: true } } }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct UiSurfaceRequest {
    pub surface_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiSurfaceVisibilityRequest {
    pub surface_id: String,
    pub visible: bool,
}
impl Default for UiSurfaceVisibilityRequest { fn default() -> Self { Self { surface_id: String::new(), visible: true } } }

/// Input frame request for the retained UI interaction dispatcher.
///
/// This is intentionally generic: it is not an Asset Browser request, toolbar
/// request or pause-menu request. The active UI provider receives the current
/// input snapshot plus viewport metrics, resolves the active top-layer/modal
/// surfaces, performs hit-testing/focus/capture, and returns a
/// `UiEventDispatchFrame`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiDispatchInputRequest {
    pub version: u32,
    pub frame_index: u64,
    /// Optional surface hint. Empty means dispatch across visible modal/top
    /// surfaces by provider z-order.
    pub surface_id: String,
    pub input: UiInputFrame,
    pub surface_size_px: [u32; 2],
    pub pixels_per_point: f32,
    /// Legacy extension field kept as payload metadata, not as the dispatch
    /// model. New callers should use `input`.
    pub event: String,
    pub payload: serde_json::Value,
}
impl Default for UiDispatchInputRequest {
    fn default() -> Self {
        Self {
            version: 1,
            frame_index: 0,
            surface_id: String::new(),
            input: UiInputFrame::default(),
            surface_size_px: [1280, 720],
            pixels_per_point: 1.0,
            event: String::new(),
            payload: serde_json::Value::Null,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiDispatchActionRequest {
    pub surface_id: String,
    pub action_id: String,
    pub payload: serde_json::Value,
}
impl Default for UiDispatchActionRequest { fn default() -> Self { Self { surface_id: String::new(), action_id: String::new(), payload: serde_json::Value::Null } } }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct UiNavigateRequest {
    pub surface_id: String,
    pub target: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiDebugTreeResponse {
    pub version: u32,
    pub provider: String,
    pub surface_id: String,
    pub source: String,
    pub node_contract: String,
    pub layout_contract: String,
    pub nodes: Vec<UiDebugNode>,
    pub layout_frames: Vec<UiLayoutFrame>,
    pub overlays: UiDevToolsOverlayFrame,
    pub diagnostics: Vec<String>,
}
impl Default for UiDebugTreeResponse {
    fn default() -> Self {
        Self {
            version: 2,
            provider: String::new(),
            surface_id: String::new(),
            source: String::new(),
            node_contract: "UiDebugNode/v1".to_owned(),
            layout_contract: "UiLayoutBox/v1".to_owned(),
            nodes: Vec::new(),
            layout_frames: Vec::new(),
            overlays: UiDevToolsOverlayFrame::default(),
            diagnostics: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiDebugBindingsResponse {
    pub version: u32,
    pub surface_id: String,
    pub bindings: Vec<UiBindingEdge>,
    pub actions: Vec<UiActionEdge>,
}
impl Default for UiDebugBindingsResponse { fn default() -> Self { Self { version: 1, surface_id: String::new(), bindings: Vec::new(), actions: Vec::new() } } }
