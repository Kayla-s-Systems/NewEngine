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
    fn default() -> Self {
        Self::Normal
    }
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
    fn default() -> Self {
        Self::Info
    }
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
    pub fn new(
        title: impl Into<String>,
        detail: impl Into<String>,
        severity: UiNodeMessageSeverity,
    ) -> Self {
        Self {
            title: title.into(),
            detail: detail.into(),
            severity,
            ..Self::default()
        }
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
        Self {
            id: id.into(),
            text: text.into(),
            ..Self::default()
        }
    }

    #[inline]
    pub fn row(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            component_id: UI_COMPONENT_ROW.to_owned(),
            text: text.into(),
            ..Self::default()
        }
    }

    #[inline]
    pub fn action(
        id: impl Into<String>,
        text: impl Into<String>,
        action_id: impl Into<String>,
    ) -> Self {
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
        self.props
            .insert("tooltip".to_owned(), serde_json::Value::String(text.into()));
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
