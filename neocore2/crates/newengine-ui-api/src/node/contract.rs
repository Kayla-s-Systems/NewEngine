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
    CodeEditor,
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
    fn default() -> Self {
        Self::Panel
    }
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
            Self::CodeEditor => UI_COMPONENT_CODE_EDITOR,
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
            UI_COMPONENT_CODE_EDITOR => Self::CodeEditor,
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
    fn default() -> Self {
        Self::RuntimeRequest
    }
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
    fn default() -> Self {
        Self::Click
    }
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
