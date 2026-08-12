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
                .with_children(
                    node.components
                        .iter()
                        .map(UiNodeRequest::from_component_node)
                        .collect(),
                ),
            surface_style: Some(node.style.clone()),
            admission_policy: Some(node.admission_policy),
            diagnostics: node.footer_lines.clone(),
        }
    }

    #[inline]
    pub fn to_surface_node(&self) -> UiSurfaceNode {
        let root_component = self.root.to_component_node();
        let surface_id = if self.surface_id.trim().is_empty() {
            self.root.id.clone()
        } else {
            self.surface_id.clone()
        };
        UiSurfaceNode {
            version: self.version.max(1),
            surface_id,
            source: if self.source.trim().is_empty() {
                "engine.ui.node_request".to_owned()
            } else {
                self.source.clone()
            },
            visible: self.visible,
            modal: self.modal,
            z_order: self.z_order,
            title: root_component.text.clone(),
            subtitle: self.diagnostics.first().cloned().unwrap_or_default(),
            body_lines: Vec::new(),
            footer_lines: self.diagnostics.clone(),
            style_tags: vec![
                "runtime-node-tree".to_owned(),
                format!("source:{:?}", self.source_kind).to_ascii_lowercase(),
            ],
            theme_id: self.theme_id.clone(),
            style_ref: self.style_ref.clone(),
            component_id: root_component.component_id.clone(),
            components: root_component.children.clone(),
            message: None,
            style: self
                .surface_style
                .clone()
                .unwrap_or_else(|| UiSurfaceStyle {
                    theme_id: self.theme_id.clone(),
                    ..UiSurfaceStyle::default()
                }),
            admission_policy: self
                .admission_policy
                .unwrap_or(UiSurfaceAdmissionPolicy::AcceptAll),
            metrics: BTreeMap::from([
                ("request_id".to_owned(), serde_json::json!(self.request_id)),
                ("root_id".to_owned(), serde_json::json!(self.root.id)),
                (
                    "root_kind".to_owned(),
                    serde_json::json!(format!("{:?}", self.root.kind)),
                ),
                (
                    "source_kind".to_owned(),
                    serde_json::json!(format!("{:?}", self.source_kind)),
                ),
            ]),
        }
    }
}
