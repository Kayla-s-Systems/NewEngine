#![forbid(unsafe_op_in_unsafe_fn)]

use smallvec::SmallVec;

use crate::markup::state::UiEventKind;

pub(crate) fn parse_actions_for(
    node: &roxmltree::Node<'_, '_>,
    kind: UiEventKind,
    out: &mut SmallVec<[String; 2]>,
) {
    match kind {
        UiEventKind::Click => {
            if let Some(v) = node.attribute("on_click") {
                split_actions_into(v, out);
            }
        }
        UiEventKind::Change => {
            if let Some(v) = node.attribute("on_change") {
                split_actions_into(v, out);
            }
        }
        UiEventKind::Submit => {
            if let Some(v) = node.attribute("on_submit") {
                split_actions_into(v, out);
            }
        }
    }

    if let Some(v) = node.attribute("on") {
        for chunk in v.split(';') {
            let chunk = chunk.trim();
            if chunk.is_empty() {
                continue;
            }
            let Some((ev, acts)) = chunk.split_once(':') else {
                continue;
            };

            let ev = ev.trim().to_ascii_lowercase();
            let acts = acts.trim();

            let match_kind = match ev.as_str() {
                "click" | "on_click" => UiEventKind::Click,
                "change" | "on_change" => UiEventKind::Change,
                "submit" | "on_submit" => UiEventKind::Submit,
                _ => continue,
            };

            if match_kind == kind {
                split_actions_into(acts, out);
            }
        }
    }
}

#[inline]
fn split_actions_into(s: &str, out: &mut SmallVec<[String; 2]>) {
    for part in s.split(|c| c == ',' || c == '|') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        out.push(p.to_string());
    }
}