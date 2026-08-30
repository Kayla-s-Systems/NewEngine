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
        Self {
            id: id.into(),
            kind,
            ..Self::default()
        }
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
        self.props
            .insert("x_px".to_owned(), serde_json::json!(x_px));
        self.props
            .insert("y_px".to_owned(), serde_json::json!(y_px));
        self.props
            .insert("w_px".to_owned(), serde_json::json!(w_px));
        self.props
            .insert("h_px".to_owned(), serde_json::json!(h_px));
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
            interactive: component.action_id.is_some()
                || component
                    .props
                    .get("interactive")
                    .and_then(|it| it.as_bool())
                    .unwrap_or(false),
            tone: component.tone,
            state_tags: component.state_tags.clone(),
            action_id: component.action_id.clone(),
            props: component.props.clone(),
            children: component
                .children
                .iter()
                .map(Self::from_component_node)
                .collect(),
            ..Self::default()
        }
    }

    #[inline]
    pub fn to_component_node(&self) -> UiComponentNode {
        let mut props = self.props.clone();
        if !self.role.is_empty() {
            props.insert(
                "role".to_owned(),
                serde_json::Value::String(self.role.clone()),
            );
        }
        if let Some(text_key) = self.text_key.as_ref().filter(|it| !it.is_empty()) {
            props.insert(
                "text_key".to_owned(),
                serde_json::Value::String(text_key.clone()),
            );
        }
        if let Some(tooltip) = self.tooltip.as_ref().filter(|it| !it.is_empty()) {
            props.insert(
                "tooltip".to_owned(),
                serde_json::Value::String(tooltip.clone()),
            );
        }
        if let Some(tooltip_key) = self.tooltip_key.as_ref().filter(|it| !it.is_empty()) {
            props.insert(
                "tooltip_key".to_owned(),
                serde_json::Value::String(tooltip_key.clone()),
            );
        }
        if let Some(source_span) = self.source_span.as_ref() {
            props.insert(
                "source_span".to_owned(),
                serde_json::to_value(source_span).unwrap_or(serde_json::Value::Null),
            );
        }
        props.insert("visible".to_owned(), serde_json::Value::Bool(self.visible));
        props.insert("enabled".to_owned(), serde_json::Value::Bool(self.enabled));
        props.insert(
            "interactive".to_owned(),
            serde_json::Value::Bool(self.interactive),
        );
        props.insert(
            "layout".to_owned(),
            serde_json::to_value(&self.layout).unwrap_or(serde_json::Value::Null),
        );
        if !self.bindings.is_empty() {
            props.insert(
                "bindings".to_owned(),
                serde_json::to_value(&self.bindings).unwrap_or(serde_json::Value::Null),
            );
        }
        if !self.events.is_empty() {
            props.insert(
                "events".to_owned(),
                serde_json::to_value(&self.events).unwrap_or(serde_json::Value::Null),
            );
        }

        let mut tags = self.state_tags.clone();
        tags.extend(self.style_tags.iter().cloned());
        tags.push(format!("kind:{}", self.kind.default_component_id()));
        if !self.enabled {
            tags.push("disabled".to_owned());
        }
        if self.interactive {
            tags.push("interactive".to_owned());
        }

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
            tone: if self.enabled {
                self.tone
            } else {
                UiNodeTone::Disabled
            },
            state_tags: tags,
            action_id: self.action_id.clone(),
            props,
            // Retained UI must preserve authored hidden subtrees. Visibility is a
            // paint/hit-test state, not a structural admission rule: a node that
            // starts hidden may become visible later through a state binding.
            children: self
                .children
                .iter()
                .map(UiNodeRequest::to_component_node)
                .collect(),
        }
    }
}


#[cfg(test)]
mod retained_visibility_tests {
    use super::*;

    #[test]
    fn hidden_child_survives_component_materialization() {
        let mut hidden = UiNodeRequest::new("character.window", UiRuntimeNodeKind::Panel);
        hidden.visible = false;
        hidden = hidden.with_child(UiNodeRequest::new(
            "character.title",
            UiRuntimeNodeKind::Text,
        ).with_text("Character Menu"));
        let root = UiNodeRequest::new("game.hud", UiRuntimeNodeKind::Panel).with_child(hidden);

        let component = root.to_component_node();
        assert_eq!(component.children.len(), 1);
        let window = &component.children[0];
        assert_eq!(window.id, "character.window");
        assert_eq!(window.props.get("visible").and_then(serde_json::Value::as_bool), Some(false));
        assert_eq!(window.children.len(), 1);
        assert_eq!(window.children[0].id, "character.title");
    }
}
