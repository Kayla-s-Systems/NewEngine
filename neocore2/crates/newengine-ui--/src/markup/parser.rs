#![forbid(unsafe_op_in_unsafe_fn)]

use roxmltree::{Document, Node};
use smallvec::SmallVec;

use crate::markup::actions::parse_actions_for;
use crate::markup::state::UiEventKind;
use crate::markup::theme::{UiDensity, UiThemeDesc, UiVisuals};
use crate::markup::ui_node::UiNode;

pub(crate) fn parse_ui_root(doc: &Document) -> Result<UiNode, String> {
    let root = doc.root_element();

    let tag = root.tag_name().name();
    if tag != "ui" {
        return Err(format!("root tag must be <ui>, got <{tag}>"));
    }

    Ok(UiNode::Ui {
        children: parse_children(root)?,
    })
}

pub(crate) fn parse_theme(doc: &Document) -> UiThemeDesc {
    let root = doc.root_element();

    let mut theme = UiThemeDesc::default();

    let visuals = attr_any(root, &["visuals", "theme"]).unwrap_or("auto");
    theme.visuals = match visuals.trim().to_ascii_lowercase().as_str() {
        "dark" => UiVisuals::Dark,
        "light" => UiVisuals::Light,
        _ => UiVisuals::Auto,
    };

    theme.scale = attr_f32(root, "scale").unwrap_or(1.0).clamp(0.25, 4.0);
    theme.font_size = attr_f32(root, "font_size").unwrap_or(14.0).clamp(8.0, 40.0);

    let density = attr_str(root, "density").unwrap_or("default");
    theme.density = match density.trim().to_ascii_lowercase().as_str() {
        "compact" => UiDensity::Compact,
        "dense" => UiDensity::Dense,
        "tight" => UiDensity::Tight,
        _ => UiDensity::Default,
    };

    theme
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

            let mut on_click = SmallVec::<[String; 2]>::new();
            parse_actions_for(&n, UiEventKind::Click, &mut on_click);

            Ok(UiNode::Button { id, text, on_click })
        }
        "textbox" | "input" => {
            let id = attr(n, "id").unwrap_or_else(|| "textbox".to_string());
            let bind = attr(n, "bind").unwrap_or_else(|| id.clone());
            let hint = attr(n, "hint").unwrap_or_default();
            let multiline = attr(n, "multiline")
                .map(|v| v == "true" || v == "1" || v == "yes")
                .unwrap_or(false);

            let mut on_change = SmallVec::<[String; 2]>::new();
            let mut on_submit = SmallVec::<[String; 2]>::new();
            parse_actions_for(&n, UiEventKind::Change, &mut on_change);
            parse_actions_for(&n, UiEventKind::Submit, &mut on_submit);

            Ok(UiNode::TextBox {
                id,
                hint,
                bind,
                multiline,
                on_change,
                on_submit,
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

#[inline]
fn attr_str<'a>(n: Node<'a, 'a>, key: &str) -> Option<&'a str> {
    n.attribute(key).map(|s| s.trim()).filter(|s| !s.is_empty())
}

#[inline]
fn attr_any<'a>(n: Node<'a, 'a>, keys: &[&str]) -> Option<&'a str> {
    for k in keys {
        if let Some(v) = attr_str(n, k) {
            return Some(v);
        }
    }
    None
}

#[inline]
fn attr_f32(n: Node<'_, '_>, key: &str) -> Option<f32> {
    attr_str(n, key).and_then(|s| s.parse::<f32>().ok())
}