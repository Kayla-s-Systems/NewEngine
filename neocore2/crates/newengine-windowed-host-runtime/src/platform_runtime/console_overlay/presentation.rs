use newengine_ui_api::{
    UiComponentNode, UiNodeTone, UiSurfaceAnchor, UiSurfaceNode, UiSurfaceStyle,
    UI_COMPONENT_INPUT, UI_COMPONENT_LIST, UI_COMPONENT_PANEL, UI_COMPONENT_ROW,
    UI_SURFACE_ENGINE_CONSOLE, UI_THEME_NORTHSTAR_DEFAULT,
};

use super::state::{ConsoleLineKind, RuntimeConsoleOverlayState};

const VISIBLE_SUGGESTIONS: usize = 8;

#[inline]
fn width_for_prompt(surface_size_px: [u32; 2]) -> f32 {
    surface_size_px[0].max(320) as f32
}

pub(super) const CONSOLE_TOP_Z_ORDER: i32 = i32::MAX;

pub(super) fn surface_node(
    state: &RuntimeConsoleOverlayState,
    surface_size_px: [u32; 2],
) -> UiSurfaceNode {
    let width = surface_size_px[0].max(320) as f32;
    let height = surface_size_px[1].max(240) as f32;
    let console_max_height = (height * 0.62).clamp(300.0, 700.0);
    let suggestion_limit = if console_max_height < 420.0 {
        3
    } else {
        VISIBLE_SUGGESTIONS
    };
    let visible_suggestions = state.suggestions.items.len().min(suggestion_limit);
    let signature_rows = usize::from(!state.suggestions.signature.trim().is_empty());
    let reserved_chrome_px = 112.0 + (visible_suggestions + signature_rows) as f32 * 22.0;
    let output_max_height = (console_max_height - reserved_chrome_px).clamp(96.0, 390.0);

    // Output is a real retained scroll container. Never truncate history at the
    // presentation boundary: RuntimeConsoleOverlayState already owns a bounded
    // 256-line ring, so every resident line must remain reachable by scrolling.
    let output_children = state
        .output
        .iter()
        .enumerate()
        .map(|(index, line)| {
            let (tone, icon) = match line.kind {
                ConsoleLineKind::Info => (UiNodeTone::Normal, "fa-solid:circle-info"),
                ConsoleLineKind::Command => (UiNodeTone::Accent, "fa-solid:terminal"),
                ConsoleLineKind::Output => (UiNodeTone::Normal, "fa-solid:chevron-right"),
                ConsoleLineKind::Error => (UiNodeTone::Danger, "fa-solid:triangle-exclamation"),
            };
            UiComponentNode::text(format!("console.output.{index}"), line.text.clone())
                .with_tone(tone)
                .with_icon(icon)
                .tagged("console-output")
        })
        .collect::<Vec<_>>();
    let mut output_list = UiComponentNode {
        id: "console.output.scroll".to_owned(),
        component_id: UI_COMPONENT_LIST.to_owned(),
        children: output_children,
        ..UiComponentNode::default()
    };
    output_list = output_list
        .with_prop("scroll_y", serde_json::json!(true))
        .with_prop("max_height_px", serde_json::json!(output_max_height))
        .with_prop("stick_to_bottom", serde_json::json!(true))
        .with_prop("always_show_scrollbar", serde_json::json!(true))
        .tagged("console-output-scroll");
    let mut components = vec![output_list];

    let prompt_value = state.prompt_with_cursor();
    let mut prompt = UiComponentNode::text("console.prompt", ">")
        .with_value(prompt_value)
        .tagged("console-prompt")
        .with_prop("focus", serde_json::json!(true))
        .with_prop("monospace", serde_json::json!(true))
        .with_prop(
            "desired_width_px",
            serde_json::json!((width_for_prompt(surface_size_px) - 82.0).max(220.0)),
        )
        .with_prop("fill_rgba", serde_json::json!([14, 22, 32, 255]))
        .with_prop("text_rgba", serde_json::json!([248, 250, 252, 255]));
    prompt.component_id = UI_COMPONENT_INPUT.to_owned();
    let mut prompt_row = UiComponentNode {
        id: "console.prompt.row".to_owned(),
        component_id: UI_COMPONENT_ROW.to_owned(),
        children: vec![
            UiComponentNode::text("console.prompt.icon", "")
                .with_icon("fa-solid:terminal")
                .with_tone(UiNodeTone::Accent)
                .tagged("console-prompt-icon"),
            prompt,
        ],
        ..UiComponentNode::default()
    };
    prompt_row = prompt_row.tagged("console-prompt-row");
    components.push(prompt_row);

    if !state.suggestions.signature.trim().is_empty() {
        components.push(
            UiComponentNode::text(
                "console.signature",
                format!("signature: {}", state.suggestions.signature),
            )
            .with_icon("fa-solid:circle-info")
            .with_tone(UiNodeTone::Disabled)
            .tagged("console-signature"),
        );
    }
    components.extend(
        state
            .suggestions
            .items
            .iter()
            .take(suggestion_limit)
            .enumerate()
            .map(|(index, item)| {
                let mut text = format!("{}  [{}]", item.display, item.kind);
                if !item.help.trim().is_empty() {
                    text.push_str(" — ");
                    text.push_str(item.help.trim());
                }
                UiComponentNode::row(format!("console.suggest.{index}"), text)
                    .with_icon("fa-solid:chevron-right")
                    .with_detail(item.usage.clone())
                    .with_tone(if index == 0 {
                        UiNodeTone::Accent
                    } else {
                        UiNodeTone::Normal
                    })
                    .tagged("console-suggestion")
            }),
    );

    UiSurfaceNode {
        version: 1,
        surface_id: UI_SURFACE_ENGINE_CONSOLE.to_owned(),
        source: "newengine-windowed-host-runtime.console-overlay".to_owned(),
        visible: state.open,
        modal: state.open,
        z_order: CONSOLE_TOP_Z_ORDER,
        title: "North Star Runtime Console".to_owned(),
        subtitle: "engine.command | wheel/scrollbar history | Tab autocomplete | Up/Down history | Esc or ~ close".to_owned(),
        body_lines: Vec::new(),
        footer_lines: Vec::new(),
        style_tags: vec![
            "retained".to_owned(),
            "runtime-console".to_owned(),
            "developer-tool".to_owned(),
            "dock-top".to_owned(),
            "always-on-top".to_owned(),
            // The provider must paint this engine-owned surface on its first visible frame;
            // no retained-window sizing pass is allowed for the developer console.
            "immediate-first-frame".to_owned(),
        ],
        theme_id: UI_THEME_NORTHSTAR_DEFAULT.to_owned(),
        style_ref: None,
        component_id: UI_COMPONENT_PANEL.to_owned(),
        components,
        message: None,
        style: UiSurfaceStyle {
            anchor: UiSurfaceAnchor::TopLeft,
            panel_rgba: [5, 9, 14, 255],
            panel_header_rgba: [11, 18, 28, 255],
            text_rgba: [244, 247, 251, 255],
            text_muted_rgba: [190, 201, 216, 255],
            accent_rgba: [104, 183, 255, 255],
            danger_rgba: [255, 120, 120, 255],
            border_rgba: [96, 165, 250, 235],
            backdrop_rgba: [0, 0, 0, 176],
            min_size_px: [(width - 16.0).min(760.0).max(320.0), 300.0],
            max_size_px: [(width - 16.0).max(320.0), console_max_height],
            margin_px: [8.0, 8.0],
            padding_px: [12.0, 34.0, 12.0, 14.0],
            row_pitch_px: 21.0,
            corner_radius_px: 6.0,
            border_px: 1.0,
            shadow_alpha: 110,
            font: newengine_ui_api::UiFontStyle {
                stack: vec![
                    "Cascadia Mono".to_owned(),
                    "Consolas".to_owned(),
                    "Segoe UI".to_owned(),
                ],
                body_px: 13.5,
                title_px: 15.0,
                secondary_px: 11.5,
                line_height_px: 18.0,
                ..Default::default()
            },
            ..UiSurfaceStyle::default()
        },
        admission_policy: Default::default(),
        metrics: Default::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find_component<'a>(
        nodes: &'a [UiComponentNode],
        predicate: &impl Fn(&UiComponentNode) -> bool,
    ) -> Option<&'a UiComponentNode> {
        for node in nodes {
            if predicate(node) {
                return Some(node);
            }
            if let Some(found) = find_component(&node.children, predicate) {
                return Some(found);
            }
        }
        None
    }

    #[test]
    fn console_surface_is_modal_top_overlay_with_scroll_icons_and_input() {
        let mut state = RuntimeConsoleOverlayState::default();
        state.open = true;
        for index in 0..40 {
            state.push_line(ConsoleLineKind::Output, format!("line {index}"));
        }
        let node = surface_node(&state, [1920, 1080]);
        assert_eq!(node.surface_id, UI_SURFACE_ENGINE_CONSOLE);
        assert!(node.modal);
        assert_eq!(node.z_order, CONSOLE_TOP_Z_ORDER);
        assert_eq!(node.z_order, i32::MAX);
        assert_eq!(node.style.anchor, UiSurfaceAnchor::TopLeft);
        assert_eq!(node.style.panel_rgba[3], 255);
        assert!(node.style.backdrop_rgba[3] >= 160);

        let scroll = find_component(&node.components, &|component| {
            component.id == "console.output.scroll"
        })
        .expect("console output scroll container");
        assert_eq!(scroll.component_id, UI_COMPONENT_LIST);
        assert_eq!(scroll.props.get("scroll_y"), Some(&serde_json::json!(true)));
        assert_eq!(
            scroll.props.get("always_show_scrollbar"),
            Some(&serde_json::json!(true))
        );
        assert!(
            scroll.children.len() >= 41,
            "history must not be presentation-truncated"
        );
        assert!(scroll.children.iter().all(|line| line.icon.is_some()));

        let input = find_component(&node.components, &|component| {
            component.component_id == UI_COMPONENT_INPUT
        })
        .expect("console input component");
        assert!(input
            .value
            .as_deref()
            .is_some_and(|value| value.contains('>')));
        assert_eq!(
            input.props.get("fill_rgba"),
            Some(&serde_json::json!([14, 22, 32, 255]))
        );
        let prompt_icon = find_component(&node.components, &|component| {
            component.id == "console.prompt.icon"
        })
        .expect("console prompt icon");
        assert_eq!(prompt_icon.icon.as_deref(), Some("fa-solid:terminal"));
    }
}
