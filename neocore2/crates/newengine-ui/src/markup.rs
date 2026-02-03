#![forbid(unsafe_op_in_unsafe_fn)]

use std::time::{Duration, Instant};
use std::{borrow::Cow, thread};

use ahash::AHashMap;
use roxmltree::{Document, Node};

use newengine_assets::{AssetKey, AssetState, AssetStore, TextReader};

#[derive(Debug)]
pub enum UiMarkupError {
    Enqueue(String),
    Timeout { path: String },
    Failed(String),
    BlobMissing,
    TextRead(String),
    XmlParse(String),
    Invalid(String),
}

impl std::fmt::Display for UiMarkupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UiMarkupError::Enqueue(e) => write!(f, "ui: load enqueue failed: {e}"),
            UiMarkupError::Timeout { path } => write!(f, "ui: timeout while loading '{path}'"),
            UiMarkupError::Failed(msg) => write!(f, "ui: asset failed: {msg}"),
            UiMarkupError::BlobMissing => write!(f, "ui: asset Ready but blob missing"),
            UiMarkupError::TextRead(e) => write!(f, "ui: TextReader failed: {e}"),
            UiMarkupError::XmlParse(e) => write!(f, "ui: xml parse failed: {e}"),
            UiMarkupError::Invalid(e) => write!(f, "ui: markup invalid: {e}"),
        }
    }
}

impl std::error::Error for UiMarkupError {}

/// Runtime state for UI bindings and events.
#[derive(Debug, Default)]
pub struct UiState {
    pub strings: AHashMap<String, String>,
    pub clicked: AHashMap<String, bool>,
    pub vars: AHashMap<String, String>,
}

impl UiState {
    #[inline]
    pub fn take_clicked(&mut self, id: &str) -> bool {
        self.clicked.remove(id).unwrap_or(false)
    }

    #[inline]
    pub fn set_var(&mut self, k: impl Into<String>, v: impl Into<String>) {
        self.vars.insert(k.into(), v.into());
    }
}

/// Parsed UI document.
#[derive(Debug, Clone)]
pub struct UiMarkupDoc {
    root: UiNode,
}

impl UiMarkupDoc {
    /// Load UI markup via AssetStore (engine decides how to pump).
    ///
    /// `pump` should call your AssetManager::pump() (or any equivalent).
    pub fn load_from_store<P>(
        store: &AssetStore,
        mut pump: P,
        logical_path: &str,
        timeout: Duration,
    ) -> Result<Self, UiMarkupError>
    where
        P: FnMut(),
    {
        let key = AssetKey::new(logical_path, 0);

        let id = store
            .load(key)
            .map_err(|e| UiMarkupError::Enqueue(e.to_string()))?;

        let t0 = Instant::now();
        loop {
            pump();

            match store.state(id) {
                AssetState::Ready => break,
                AssetState::Failed(msg) => return Err(UiMarkupError::Failed(msg.parse().unwrap())),
                AssetState::Loading | AssetState::Unloaded => {}
            }

            if t0.elapsed() >= timeout {
                return Err(UiMarkupError::Timeout {
                    path: logical_path.to_string(),
                });
            }

            thread::yield_now();
        }

        let blob = store.get_blob(id).ok_or(UiMarkupError::BlobMissing)?;

        let doc = TextReader::from_blob_parts(&blob.meta_json, &blob.payload)
            .map_err(|e| UiMarkupError::TextRead(e.to_string()))?;

        let parsed =
            Document::parse(&doc.text).map_err(|e| UiMarkupError::XmlParse(e.to_string()))?;

        let ui = parse_ui_root(&parsed).map_err(UiMarkupError::Invalid)?;

        Ok(Self { root: ui })
    }

    pub fn render(&self, ctx: &egui::Context, state: &mut UiState) {
        self.root.render(ctx, state);
    }
}

#[derive(Debug, Clone)]
enum UiNode {
    Ui {
        children: Vec<UiNode>,
    },
    TopBar {
        children: Vec<UiNode>,
    },
    Window {
        title: String,
        open: bool,
        children: Vec<UiNode>,
    },
    Row {
        children: Vec<UiNode>,
    },
    Column {
        children: Vec<UiNode>,
    },

    Label {
        id: Option<String>,
        text: String,
    },
    Button {
        id: String,
        text: String,
    },
    TextBox {
        id: String,
        hint: String,
        bind: String,
        multiline: bool,
    },

    Spacer,

    Unknown {
        tag: String,
        children: Vec<UiNode>,
    },
}

impl UiNode {
    fn render(&self, ctx: &egui::Context, state: &mut UiState) {
        match self {
            UiNode::Ui { children } => {
                for c in children {
                    c.render(ctx, state);
                }
            }
            UiNode::TopBar { children } => {
                egui::TopBottomPanel::top("ui_topbar").show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        for c in children {
                            render_in_ui(c, ui, state);
                        }
                    });
                });
            }
            UiNode::Window {
                title,
                open,
                children,
            } => {
                let mut is_open = *open;
                egui::Window::new(title).open(&mut is_open).show(ctx, |ui| {
                    for c in children {
                        render_in_ui(c, ui, state);
                    }
                });
            }
            _ => {}
        }
    }
}

fn render_in_ui(node: &UiNode, ui: &mut egui::Ui, state: &mut UiState) {
    match node {
        UiNode::Row { children } => {
            ui.horizontal(|ui| {
                for c in children {
                    render_in_ui(c, ui, state);
                }
            });
        }
        UiNode::Column { children } => {
            ui.vertical(|ui| {
                for c in children {
                    render_in_ui(c, ui, state);
                }
            });
        }
        UiNode::Label { id: _, text } => {
            let s = substitute_vars(text, &state.vars);
            ui.label(s.as_ref());
        }
        UiNode::Button { id, text } => {
            let s = substitute_vars(text, &state.vars);
            if ui.button(s.as_ref()).clicked() {
                state.clicked.insert(id.clone(), true);
            }
        }
        UiNode::TextBox {
            id: _,
            hint,
            bind,
            multiline,
        } => {
            let entry = state.strings.entry(bind.clone()).or_default();
            let hint = substitute_vars(hint, &state.vars);

            if *multiline {
                ui.add(
                    egui::TextEdit::multiline(entry)
                        .hint_text(hint.as_ref())
                        .desired_width(f32::INFINITY),
                );
            } else {
                ui.add(
                    egui::TextEdit::singleline(entry)
                        .hint_text(hint.as_ref())
                        .desired_width(f32::INFINITY),
                );
            }
        }
        UiNode::Spacer => ui.add_space(8.0),
        UiNode::TopBar { children } => {
            ui.horizontal(|ui| {
                for c in children {
                    render_in_ui(c, ui, state);
                }
            });
        }
        UiNode::Window { .. } => {}
        UiNode::Ui { children } => {
            for c in children {
                render_in_ui(c, ui, state);
            }
        }
        UiNode::Unknown { children, .. } => {
            for c in children {
                render_in_ui(c, ui, state);
            }
        }
    }
}

fn substitute_vars<'a>(src: &'a str, vars: &AHashMap<String, String>) -> Cow<'a, str> {
    if !src.contains('$') {
        return Cow::Borrowed(src);
    }

    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    let b = src.as_bytes();

    while i < b.len() {
        if b[i] == b'$' {
            i += 1;
            let start = i;
            while i < b.len() && is_var_char(b[i]) {
                i += 1;
            }
            let key = &src[start..i];
            if let Some(v) = vars.get(key) {
                out.push_str(v);
            } else {
                out.push('$');
                out.push_str(key);
            }
        } else {
            out.push(b[i] as char);
            i += 1;
        }
    }

    Cow::Owned(out)
}

#[inline]
fn is_var_char(c: u8) -> bool {
    matches!(c, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'.' | b'-')
}

fn parse_ui_root(doc: &Document) -> Result<UiNode, String> {
    let root = doc.root_element();

    let tag = root.tag_name().name();
    if tag != "ui" {
        return Err(format!("root tag must be <ui>, got <{tag}>"));
    }

    Ok(UiNode::Ui {
        children: parse_children(root)?,
    })
}

fn parse_children(parent: Node) -> Result<Vec<UiNode>, String> {
    let mut out = Vec::new();
    for n in parent.children().filter(|n| n.is_element()) {
        out.push(parse_node(n)?);
    }
    Ok(out)
}

fn parse_node(n: Node) -> Result<UiNode, String> {
    let tag = n.tag_name().name();
    match tag {
        "topbar" => Ok(UiNode::TopBar {
            children: parse_children(n)?,
        }),
        "window" => {
            let title = attr(n, "title").unwrap_or_else(|| "Window".to_string());
            let open = attr(n, "open")
                .map(|v| v == "true" || v == "1" || v == "yes")
                .unwrap_or(true);

            Ok(UiNode::Window {
                title,
                open,
                children: parse_children(n)?,
            })
        }
        "row" | "div" => {
            if tag == "div" {
                let class = attr(n, "class").unwrap_or_default();
                if !class.split_whitespace().any(|c| c == "row") {
                    return Ok(UiNode::Unknown {
                        tag: tag.to_string(),
                        children: parse_children(n)?,
                    });
                }
            }
            Ok(UiNode::Row {
                children: parse_children(n)?,
            })
        }
        "col" | "column" => Ok(UiNode::Column {
            children: parse_children(n)?,
        }),
        "label" => Ok(UiNode::Label {
            id: attr_opt(n, "id"),
            text: attr(n, "text").unwrap_or_default(),
        }),
        "button" => {
            let id = attr(n, "id").ok_or_else(|| "button requires id".to_string())?;
            let text = attr(n, "text").unwrap_or_else(|| "Button".to_string());
            Ok(UiNode::Button { id, text })
        }
        "textbox" | "input" => {
            let id = attr(n, "id").unwrap_or_else(|| "textbox".to_string());
            let bind = attr(n, "bind").unwrap_or_else(|| id.clone());
            let hint = attr(n, "hint").unwrap_or_default();
            let multiline = attr(n, "multiline")
                .map(|v| v == "true" || v == "1" || v == "yes")
                .unwrap_or(false);

            Ok(UiNode::TextBox {
                id,
                hint,
                bind,
                multiline,
            })
        }
        "spacer" => Ok(UiNode::Spacer),
        _ => Ok(UiNode::Unknown {
            tag: tag.to_string(),
            children: parse_children(n)?,
        }),
    }
}

fn attr(n: Node, key: &str) -> Option<String> {
    n.attribute(key).map(|s| s.to_string())
}

fn attr_opt(n: Node, key: &str) -> Option<String> {
    n.attribute(key).map(|s| s.to_string())
}
