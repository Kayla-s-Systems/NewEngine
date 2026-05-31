// Split from lib.rs to keep the UI API DTO surface navigable.
// This file is included flat from lib.rs to preserve the existing public API.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiNodeTone {
    Normal,
    Accent,
    Danger,
    Disabled,
}

impl Default for UiNodeTone {
    #[inline]
    fn default() -> Self { Self::Normal }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiNodeMessageSeverity {
    Info,
    Success,
    Warning,
    Danger,
}

impl Default for UiNodeMessageSeverity {
    #[inline]
    fn default() -> Self { Self::Info }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiNodeMessage {
    pub title: String,
    pub detail: String,
    pub severity: UiNodeMessageSeverity,
    pub age_sec: f32,
    pub ttl_sec: f32,
}

impl Default for UiNodeMessage {
    fn default() -> Self {
        Self {
            title: String::new(),
            detail: String::new(),
            severity: UiNodeMessageSeverity::Info,
            age_sec: 0.0,
            ttl_sec: 2.2,
        }
    }
}

impl UiNodeMessage {
    #[inline]
    pub fn new(title: impl Into<String>, detail: impl Into<String>, severity: UiNodeMessageSeverity) -> Self {
        Self { title: title.into(), detail: detail.into(), severity, ..Self::default() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiComponentNode {
    pub id: String,
    pub component_id: String,
    pub text: String,
    pub value: Option<String>,
    pub detail: Option<String>,
    pub icon: Option<String>,
    pub font_token: Option<String>,
    pub tone: UiNodeTone,
    pub state_tags: Vec<String>,
    pub action_id: Option<String>,
    pub props: BTreeMap<String, serde_json::Value>,
    pub children: Vec<UiComponentNode>,
}

impl Default for UiComponentNode {
    fn default() -> Self {
        Self {
            id: String::new(),
            component_id: UI_COMPONENT_TEXT.to_owned(),
            text: String::new(),
            value: None,
            detail: None,
            icon: None,
            font_token: None,
            tone: UiNodeTone::Normal,
            state_tags: Vec::new(),
            action_id: None,
            props: BTreeMap::new(),
            children: Vec::new(),
        }
    }
}

impl UiComponentNode {
    #[inline]
    pub fn text(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self { id: id.into(), text: text.into(), ..Self::default() }
    }

    #[inline]
    pub fn row(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self { id: id.into(), component_id: UI_COMPONENT_ROW.to_owned(), text: text.into(), ..Self::default() }
    }

    #[inline]
    pub fn action(id: impl Into<String>, text: impl Into<String>, action_id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            component_id: UI_COMPONENT_ACTION.to_owned(),
            text: text.into(),
            action_id: Some(action_id.into()),
            ..Self::default()
        }
    }

    #[inline]
    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    #[inline]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    #[inline]
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    #[inline]
    pub fn with_tone(mut self, tone: UiNodeTone) -> Self {
        self.tone = tone;
        self
    }

    #[inline]
    pub fn tagged(mut self, tag: impl Into<String>) -> Self {
        self.state_tags.push(tag.into());
        self
    }

    #[inline]
    pub fn with_prop(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.props.insert(key.into(), value);
        self
    }

    #[inline]
    pub fn with_tooltip(mut self, text: impl Into<String>) -> Self {
        self.props.insert("tooltip".to_owned(), serde_json::Value::String(text.into()));
        self
    }

    #[inline]
    pub fn with_child(mut self, child: UiComponentNode) -> Self {
        self.children.push(child);
        self
    }

    #[inline]
    pub fn with_children(mut self, children: Vec<UiComponentNode>) -> Self {
        self.children = children;
        self
    }
}

/// Provider-neutral UI node kind.
///
/// Inspired by mature editor systems such as Godot's Node/Control split, but
/// expressed as North Star data: a node is a declarative component request, not
/// a concrete widget class or a provider-owned object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiRuntimeNodeKind {
    Surface,
    Panel,
    Stack,
    Row,
    Column,
    Grid,
    Text,
    Action,
    Button,
    Input,
    Checkbox,
    Toggle,
    Slider,
    ScrollBar,
    Select,
    Separator,
    List,
    Tree,
    Split,
    Viewport,
    ExternalTexture,
    Spacer,
    Custom,
}

impl Default for UiRuntimeNodeKind {
    #[inline]
    fn default() -> Self { Self::Panel }
}

impl UiRuntimeNodeKind {
    #[inline]
    pub const fn default_component_id(self) -> &'static str {
        match self {
            Self::Surface => UI_COMPONENT_SURFACE,
            Self::Panel => UI_COMPONENT_PANEL,
            Self::Stack => UI_COMPONENT_STACK,
            Self::Row => UI_COMPONENT_ROW,
            Self::Column => UI_COMPONENT_COLUMN,
            Self::Grid => UI_COMPONENT_GRID,
            Self::Text => UI_COMPONENT_TEXT,
            Self::Action => UI_COMPONENT_ACTION,
            Self::Button => UI_COMPONENT_BUTTON,
            Self::Input => UI_COMPONENT_INPUT,
            Self::Checkbox => UI_COMPONENT_CHECKBOX,
            Self::Toggle => UI_COMPONENT_TOGGLE,
            Self::Slider => UI_COMPONENT_SLIDER,
            Self::ScrollBar => UI_COMPONENT_SCROLL_BAR,
            Self::Select => UI_COMPONENT_SELECT,
            Self::Separator => UI_COMPONENT_SEPARATOR,
            Self::List => UI_COMPONENT_LIST,
            Self::Tree => UI_COMPONENT_TREE,
            Self::Split => UI_COMPONENT_SPLIT,
            Self::Viewport => UI_COMPONENT_VIEWPORT,
            Self::ExternalTexture => UI_COMPONENT_EXTERNAL_TEXTURE,
            Self::Spacer => UI_COMPONENT_SPACER,
            Self::Custom => UI_COMPONENT_PANEL,
        }
    }

    #[inline]
    pub fn from_component_id(component_id: &str) -> Self {
        match component_id.trim() {
            UI_COMPONENT_SURFACE => Self::Surface,
            UI_COMPONENT_PANEL => Self::Panel,
            UI_COMPONENT_STACK => Self::Stack,
            UI_COMPONENT_ROW => Self::Row,
            UI_COMPONENT_COLUMN => Self::Column,
            UI_COMPONENT_GRID => Self::Grid,
            UI_COMPONENT_TEXT => Self::Text,
            UI_COMPONENT_ACTION => Self::Action,
            UI_COMPONENT_BUTTON => Self::Button,
            UI_COMPONENT_INPUT => Self::Input,
            UI_COMPONENT_CHECKBOX => Self::Checkbox,
            UI_COMPONENT_TOGGLE => Self::Toggle,
            UI_COMPONENT_SLIDER => Self::Slider,
            UI_COMPONENT_SCROLL_BAR => Self::ScrollBar,
            UI_COMPONENT_SELECT => Self::Select,
            UI_COMPONENT_SEPARATOR => Self::Separator,
            UI_COMPONENT_LIST => Self::List,
            UI_COMPONENT_TREE => Self::Tree,
            UI_COMPONENT_SPLIT => Self::Split,
            UI_COMPONENT_VIEWPORT => Self::Viewport,
            UI_COMPONENT_EXTERNAL_TEXTURE => Self::ExternalTexture,
            UI_COMPONENT_SPACER => Self::Spacer,
            _ => Self::Custom,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiNodeRequestSourceKind {
    AuthoredAsset,
    RuntimeRequest,
    Generated,
    Plugin,
    Tool,
    Script,
}

impl Default for UiNodeRequestSourceKind {
    #[inline]
    fn default() -> Self { Self::RuntimeRequest }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiNodeEventTrigger {
    HoverEnter,
    HoverExit,
    Press,
    Release,
    Click,
    DoubleClick,
    Focus,
    Blur,
    ValueChanged,
    DragStart,
    DragMove,
    DragEnd,
    ContextMenu,
}

impl Default for UiNodeEventTrigger {
    #[inline]
    fn default() -> Self { Self::Click }
}

impl UiNodeEventTrigger {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HoverEnter => "hover_enter",
            Self::HoverExit => "hover_exit",
            Self::Press => "press",
            Self::Release => "release",
            Self::Click => "click",
            Self::DoubleClick => "double_click",
            Self::Focus => "focus",
            Self::Blur => "blur",
            Self::ValueChanged => "value_changed",
            Self::DragStart => "drag_start",
            Self::DragMove => "drag_move",
            Self::DragEnd => "drag_end",
            Self::ContextMenu => "context_menu",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiNodeEventRoute {
    pub trigger: UiNodeEventTrigger,
    pub action_id: String,
    pub target_gateway: String,
    pub method: String,
    pub payload: serde_json::Value,
}

impl Default for UiNodeEventRoute {
    fn default() -> Self {
        Self {
            trigger: UiNodeEventTrigger::Click,
            action_id: String::new(),
            target_gateway: ENGINE_UI_SERVICE_ID.to_owned(),
            method: UI_SERVICE_METHOD_DISPATCH_ACTION_V1.to_owned(),
            payload: serde_json::Value::Null,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiNodeBindingRequest {
    pub property: String,
    pub source: String,
    pub path: String,
    pub mode: String,
    pub fallback: serde_json::Value,
}

impl Default for UiNodeBindingRequest {
    fn default() -> Self {
        Self {
            property: String::new(),
            source: String::new(),
            path: String::new(),
            mode: "read".to_owned(),
            fallback: serde_json::Value::Null,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiNodeLayoutRequest {
    pub slot: String,
    pub order: i32,
    pub x_px: Option<f32>,
    pub y_px: Option<f32>,
    pub w_px: Option<f32>,
    pub h_px: Option<f32>,
    pub min_size_px: [f32; 2],
    pub max_size_px: [f32; 2],
    pub grow: f32,
    pub shrink: f32,
    pub resizable: bool,
    pub detachable: bool,
}

impl Default for UiNodeLayoutRequest {
    fn default() -> Self {
        Self {
            slot: String::new(),
            order: 0,
            x_px: None,
            y_px: None,
            w_px: None,
            h_px: None,
            min_size_px: [0.0, 0.0],
            max_size_px: [0.0, 0.0],
            grow: 0.0,
            shrink: 1.0,
            resizable: false,
            detachable: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiNodeRequest {
    pub id: String,
    pub kind: UiRuntimeNodeKind,
    pub component_id: String,
    pub role: String,
    pub text: String,
    pub text_key: Option<String>,
    pub value: Option<String>,
    pub detail: Option<String>,
    pub icon: Option<String>,
    pub font_token: Option<String>,
    pub tooltip: Option<String>,
    pub tooltip_key: Option<String>,
    pub source_span: Option<UiSourceSpan>,
    pub enabled: bool,
    pub visible: bool,
    pub interactive: bool,
    pub tone: UiNodeTone,
    pub style_tags: Vec<String>,
    pub state_tags: Vec<String>,
    pub action_id: Option<String>,
    pub layout: UiNodeLayoutRequest,
    pub props: BTreeMap<String, serde_json::Value>,
    pub bindings: Vec<UiNodeBindingRequest>,
    pub events: Vec<UiNodeEventRoute>,
    pub children: Vec<UiNodeRequest>,
}

impl Default for UiNodeRequest {
    fn default() -> Self {
        Self {
            id: String::new(),
            kind: UiRuntimeNodeKind::Panel,
            component_id: String::new(),
            role: String::new(),
            text: String::new(),
            text_key: None,
            value: None,
            detail: None,
            icon: None,
            font_token: None,
            tooltip: None,
            tooltip_key: None,
            source_span: None,
            enabled: true,
            visible: true,
            interactive: false,
            tone: UiNodeTone::Normal,
            style_tags: Vec::new(),
            state_tags: Vec::new(),
            action_id: None,
            layout: UiNodeLayoutRequest::default(),
            props: BTreeMap::new(),
            bindings: Vec::new(),
            events: Vec::new(),
            children: Vec::new(),
        }
    }
}

impl UiNodeRequest {
    #[inline]
    pub fn new(id: impl Into<String>, kind: UiRuntimeNodeKind) -> Self {
        Self { id: id.into(), kind, ..Self::default() }
    }

    #[inline]
    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self
    }

    #[inline]
    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    #[inline]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    #[inline]
    pub fn with_action(mut self, action_id: impl Into<String>) -> Self {
        self.action_id = Some(action_id.into());
        self.interactive = true;
        self
    }

    #[inline]
    pub fn with_style_tag(mut self, tag: impl Into<String>) -> Self {
        self.style_tags.push(tag.into());
        self
    }

    #[inline]
    pub fn with_state_tag(mut self, tag: impl Into<String>) -> Self {
        self.state_tags.push(tag.into());
        self
    }

    #[inline]
    pub fn with_prop(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.props.insert(key.into(), value);
        self
    }

    #[inline]
    pub fn with_layout_rect(mut self, x_px: f32, y_px: f32, w_px: f32, h_px: f32) -> Self {
        self.layout.x_px = Some(x_px);
        self.layout.y_px = Some(y_px);
        self.layout.w_px = Some(w_px);
        self.layout.h_px = Some(h_px);
        self.props.insert("x_px".to_owned(), serde_json::json!(x_px));
        self.props.insert("y_px".to_owned(), serde_json::json!(y_px));
        self.props.insert("w_px".to_owned(), serde_json::json!(w_px));
        self.props.insert("h_px".to_owned(), serde_json::json!(h_px));
        self
    }

    #[inline]
    pub fn with_child(mut self, child: UiNodeRequest) -> Self {
        self.children.push(child);
        self
    }

    #[inline]
    pub fn with_children(mut self, children: Vec<UiNodeRequest>) -> Self {
        self.children = children;
        self
    }

    #[inline]
    pub fn from_component_node(component: &UiComponentNode) -> Self {
        Self {
            id: component.id.clone(),
            kind: UiRuntimeNodeKind::from_component_id(&component.component_id),
            component_id: component.component_id.clone(),
            text: component.text.clone(),
            value: component.value.clone(),
            detail: component.detail.clone(),
            icon: component.icon.clone(),
            font_token: component.font_token.clone(),
            enabled: !component.state_tags.iter().any(|tag| tag == "disabled"),
            interactive: component.action_id.is_some() || component.props.get("interactive").and_then(|it| it.as_bool()).unwrap_or(false),
            tone: component.tone,
            state_tags: component.state_tags.clone(),
            action_id: component.action_id.clone(),
            props: component.props.clone(),
            children: component.children.iter().map(Self::from_component_node).collect(),
            ..Self::default()
        }
    }

    #[inline]
    pub fn to_component_node(&self) -> UiComponentNode {
        let mut props = self.props.clone();
        if !self.role.is_empty() {
            props.insert("role".to_owned(), serde_json::Value::String(self.role.clone()));
        }
        if let Some(text_key) = self.text_key.as_ref().filter(|it| !it.is_empty()) {
            props.insert("text_key".to_owned(), serde_json::Value::String(text_key.clone()));
        }
        if let Some(tooltip) = self.tooltip.as_ref().filter(|it| !it.is_empty()) {
            props.insert("tooltip".to_owned(), serde_json::Value::String(tooltip.clone()));
        }
        if let Some(tooltip_key) = self.tooltip_key.as_ref().filter(|it| !it.is_empty()) {
            props.insert("tooltip_key".to_owned(), serde_json::Value::String(tooltip_key.clone()));
        }
        if let Some(source_span) = self.source_span.as_ref() {
            props.insert("source_span".to_owned(), serde_json::to_value(source_span).unwrap_or(serde_json::Value::Null));
        }
        props.insert("visible".to_owned(), serde_json::Value::Bool(self.visible));
        props.insert("enabled".to_owned(), serde_json::Value::Bool(self.enabled));
        props.insert("interactive".to_owned(), serde_json::Value::Bool(self.interactive));
        props.insert("layout".to_owned(), serde_json::to_value(&self.layout).unwrap_or(serde_json::Value::Null));
        if !self.bindings.is_empty() {
            props.insert("bindings".to_owned(), serde_json::to_value(&self.bindings).unwrap_or(serde_json::Value::Null));
        }
        if !self.events.is_empty() {
            props.insert("events".to_owned(), serde_json::to_value(&self.events).unwrap_or(serde_json::Value::Null));
        }

        let mut tags = self.state_tags.clone();
        tags.extend(self.style_tags.iter().cloned());
        tags.push(format!("kind:{}", self.kind.default_component_id()));
        if !self.enabled { tags.push("disabled".to_owned()); }
        if self.interactive { tags.push("interactive".to_owned()); }

        UiComponentNode {
            id: self.id.clone(),
            component_id: if self.component_id.trim().is_empty() {
                self.kind.default_component_id().to_owned()
            } else {
                self.component_id.clone()
            },
            text: self.text.clone(),
            value: self.value.clone(),
            detail: self.detail.clone(),
            icon: self.icon.clone(),
            font_token: self.font_token.clone(),
            tone: if self.enabled { self.tone } else { UiNodeTone::Disabled },
            state_tags: tags,
            action_id: self.action_id.clone(),
            props,
            children: self.children.iter().filter(|child| child.visible).map(UiNodeRequest::to_component_node).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiNodeTreeRequest {
    pub version: u32,
    pub request_id: String,
    pub surface_id: String,
    pub source: String,
    pub source_kind: UiNodeRequestSourceKind,
    pub theme_id: String,
    pub style_ref: Option<String>,
    pub visible: bool,
    pub modal: bool,
    pub z_order: i32,
    pub root: UiNodeRequest,
    /// Optional resolved surface style. Authored `.neui` and generated editor shells
    /// both travel as node-tree requests, but the provider still needs the stable
    /// surface-level style envelope for layout, hit-testing and paint.
    pub surface_style: Option<UiSurfaceStyle>,
    /// Optional admission policy carried by generated shell requests.
    pub admission_policy: Option<UiSurfaceAdmissionPolicy>,
    pub diagnostics: Vec<String>,
}

impl Default for UiNodeTreeRequest {
    fn default() -> Self {
        Self {
            version: 1,
            request_id: String::new(),
            surface_id: String::new(),
            source: String::new(),
            source_kind: UiNodeRequestSourceKind::RuntimeRequest,
            theme_id: UI_THEME_NORTHSTAR_DEFAULT.to_owned(),
            style_ref: None,
            visible: true,
            modal: false,
            z_order: 0,
            root: UiNodeRequest::new("root", UiRuntimeNodeKind::Panel),
            surface_style: None,
            admission_policy: None,
            diagnostics: Vec::new(),
        }
    }
}

impl UiNodeTreeRequest {
    #[inline]
    pub fn from_surface_node(node: &UiSurfaceNode, source_kind: UiNodeRequestSourceKind) -> Self {
        Self {
            version: node.version.max(1),
            request_id: format!("{}.frame", node.surface_id),
            surface_id: node.surface_id.clone(),
            source: node.source.clone(),
            source_kind,
            theme_id: node.theme_id.clone(),
            style_ref: node.style_ref.clone(),
            visible: node.visible,
            modal: node.modal,
            z_order: node.z_order,
            root: UiNodeRequest::new("root", UiRuntimeNodeKind::Surface)
                .with_text(node.title.clone())
                .with_detail(node.subtitle.clone())
                .with_children(node.components.iter().map(UiNodeRequest::from_component_node).collect()),
            surface_style: Some(node.style.clone()),
            admission_policy: Some(node.admission_policy.clone()),
            diagnostics: node.footer_lines.clone(),
        }
    }

    #[inline]
    pub fn to_surface_node(&self) -> UiSurfaceNode {
        let root_component = self.root.to_component_node();
        let surface_id = if self.surface_id.trim().is_empty() { self.root.id.clone() } else { self.surface_id.clone() };
        UiSurfaceNode {
            version: self.version.max(1),
            surface_id,
            source: if self.source.trim().is_empty() { "engine.ui.node_request".to_owned() } else { self.source.clone() },
            visible: self.visible,
            modal: self.modal,
            z_order: self.z_order,
            title: root_component.text.clone(),
            subtitle: self.diagnostics.first().cloned().unwrap_or_default(),
            body_lines: Vec::new(),
            footer_lines: self.diagnostics.clone(),
            style_tags: vec!["runtime-node-tree".to_owned(), format!("source:{:?}", self.source_kind).to_ascii_lowercase()],
            theme_id: self.theme_id.clone(),
            style_ref: self.style_ref.clone(),
            component_id: root_component.component_id.clone(),
            components: root_component.children.clone(),
            message: None,
            style: self.surface_style.clone().unwrap_or_else(|| UiSurfaceStyle { theme_id: self.theme_id.clone(), ..UiSurfaceStyle::default() }),
            admission_policy: self.admission_policy.clone().unwrap_or(UiSurfaceAdmissionPolicy::AcceptAll),
            metrics: BTreeMap::from([
                ("request_id".to_owned(), serde_json::json!(self.request_id)),
                ("root_id".to_owned(), serde_json::json!(self.root.id)),
                ("root_kind".to_owned(), serde_json::json!(format!("{:?}", self.root.kind))),
                ("source_kind".to_owned(), serde_json::json!(format!("{:?}", self.source_kind))),
            ]),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiNodeRequestAck {
    pub ok: bool,
    pub provider: Option<String>,
    pub surface_id: String,
    pub accepted_nodes: usize,
    pub warnings: Vec<String>,
}

impl Default for UiNodeRequestAck {
    fn default() -> Self {
        Self { ok: true, provider: None, surface_id: String::new(), accepted_nodes: 0, warnings: Vec::new() }
    }
}

impl UiNodeRequestAck {
    #[inline]
    pub fn accepted(provider: impl Into<String>, surface_id: impl Into<String>, accepted_nodes: usize) -> Self {
        Self { ok: true, provider: Some(provider.into()), surface_id: surface_id.into(), accepted_nodes, warnings: Vec::new() }
    }
}
