#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ui_api::{
    UiComponentNode, UiNodeMessage, UiSurfaceNode, UI_COMPONENT_PANEL,
    UI_THEME_NORTHSTAR_DEFAULT,
};
use newengine_ui_navigation_api::UiNodeNavigationItem;

use super::*;

impl RenderUiNodeSurfaceState {
    pub(super) fn build_ui_state(&self, visual_visible: bool) -> UiSurfaceNode {
        if !visual_visible {
            return UiSurfaceNode::hidden(newengine_ui_api::UI_SURFACE_ENGINE_PRIMARY, "engine.ui.primary");
        }

        let Some(navigation) = self.navigation.as_ref() else {
            return self.build_unavailable_ui_state(visual_visible);
        };

        let document = navigation.document();
        let current_page = navigation.current_page();
        let page_title = current_page
            .map(|page| page.title.as_str())
            .filter(|title| !title.is_empty())
            .unwrap_or("UI Surface");
        let page_subtitle = current_page
            .map(|page| page.subtitle.as_str())
            .filter(|subtitle| !subtitle.is_empty())
            .unwrap_or_else(|| document.subtitle.as_str());

        let selected_index = navigation.selected_index();
        let hovered_index = navigation.hovered_index();
        let mut components = navigation
            .current_items()
            .iter()
            .enumerate()
            .map(|(idx, item)| {
                let mut component = self.item_to_component(item);
                if idx == selected_index {
                    component = component.tagged("selected");
                }
                if Some(idx) == hovered_index {
                    component = component.tagged("hovered");
                }
                component
            })
            .collect::<Vec<_>>();
        if components.is_empty() {
            components.push(UiComponentNode::text("ui.empty", "No UI entries declared"));
        }
        let mut body_lines = components.iter().map(component_line).collect::<Vec<_>>();
        if body_lines.is_empty() {
            body_lines.push("No UI entries declared".to_owned());
        }

        let mut footer_lines = current_page
            .and_then(|page| (!page.footer_lines.is_empty()).then(|| page.footer_lines.clone()))
            .unwrap_or_else(|| document.footer_lines.clone());
        if footer_lines.is_empty() {
            footer_lines = vec![
                "ESC / START - Resume or close UI surface".to_owned(),
                "ARROWS / DPAD - Navigate".to_owned(),
                "ENTER / A / CLICK - Confirm".to_owned(),
                "BACKSPACE / B - Back".to_owned(),
            ];
        }
        if let Some(pending) = self.awaiting_rebind.as_ref() {
            footer_lines.insert(0, format!("Listening for new input: {}", pending.label));
            footer_lines.insert(1, "Press a key, mouse button, or gamepad button".to_owned());
        }

        let mut metrics = std::collections::BTreeMap::new();
        metrics.insert("page".to_owned(), serde_json::json!(navigation.current_page_id()));
        metrics.insert("selected_index".to_owned(), serde_json::json!(selected_index));
        metrics.insert("hovered_index".to_owned(), serde_json::json!(hovered_index));
        metrics.insert("document".to_owned(), serde_json::json!(document.id));

        UiSurfaceNode {
            version: 1,
            surface_id: newengine_ui_api::UI_SURFACE_ENGINE_PRIMARY.to_owned(),
            source: "engine.ui.primary".to_owned(),
            visible: visual_visible,
            modal: true,
            z_order: 950,
            title: page_title.to_owned(),
            subtitle: page_subtitle.to_owned(),
            body_lines,
            footer_lines,
            style_tags: vec!["retained".to_owned(), "node".to_owned()],
            theme_id: UI_THEME_NORTHSTAR_DEFAULT.to_owned(),
            component_id: UI_COMPONENT_PANEL.to_owned(),
            components,
            message: self.feedback.as_ref().map(UiNodeSurfaceEventFeedback::to_ui_message),
            style: ui_surface_style(),
            metrics,
        }
    }

    fn build_unavailable_ui_state(&self, visual_visible: bool) -> UiSurfaceNode {
        let detail = self
            .document_load_error
            .as_deref()
            .unwrap_or("UI document is waiting for engine.assets/VFS availability")
            .to_owned();
        let message = self.feedback.as_ref().map(UiNodeSurfaceEventFeedback::to_ui_message).or_else(|| {
            Some(UiNodeMessage::new(
                "UI surface document unavailable",
                detail.clone(),
                UiNodeMessageSeverity::Warning,
            ))
        });
        let lines = vec![
            "UI document unavailable".to_owned(),
            detail.clone(),
            "No embedded fallback is allowed for runtime UI assets".to_owned(),
            "Check AssetManager mount roots and package/VFS configuration".to_owned(),
        ];
        UiSurfaceNode {
            version: 1,
            surface_id: newengine_ui_api::UI_SURFACE_ENGINE_PRIMARY.to_owned(),
            source: "engine.ui.primary".to_owned(),
            visible: visual_visible,
            modal: true,
            z_order: 950,
            title: "UI".to_owned(),
            subtitle: "Declarative UI document is loaded through engine.assets/VFS".to_owned(),
            body_lines: lines.clone(),
            footer_lines: vec!["ESC closes".to_owned()],
            style_tags: vec!["retained".to_owned(), "error".to_owned()],
            theme_id: UI_THEME_NORTHSTAR_DEFAULT.to_owned(),
            component_id: UI_COMPONENT_PANEL.to_owned(),
            components: lines
                .iter()
                .enumerate()
                .map(|(idx, line)| UiComponentNode::text(format!("unavailable.{idx}"), line.clone()))
                .collect(),
            message,
            style: ui_surface_style(),
            metrics: std::collections::BTreeMap::from([("error".to_owned(), serde_json::json!(detail))]),
        }
    }

    fn item_to_component(&self, item: &UiNodeNavigationItem) -> UiComponentNode {
        let mut out = UiComponentNode::row(item.id.clone(), item.label.clone())
            .with_tone(tone_from_navigation(item.tone));
        if item.emphasized {
            out = out.tagged("emphasized");
        }
        if let Some(value) = self.item_value(item) {
            out = out.with_value(value);
        }
        if let Some(detail) = self.item_detail(item) {
            out = out.with_detail(detail);
        }
        if let Some(route) = item.action.as_ref() {
            out.action_id = Some(route.id.clone());
        }
        out
    }

    fn item_value(&self, item: &UiNodeNavigationItem) -> Option<String> {
        match item.dynamic_value.as_deref() {
            Some(DYNAMIC_INPUT_DEVICE_PREFERENCE) => {
                Some(device_preference_label(self.profile.device_preference).to_owned())
            }
            Some(DYNAMIC_INPUT_BINDING_LABEL) => item
                .action
                .as_ref()
                .and_then(|route| route.payload_str("action_id"))
                .map(|action_id| self.profile.primary_binding_label(action_id)),
            _ => item.value.clone(),
        }
    }

    fn item_detail(&self, item: &UiNodeNavigationItem) -> Option<String> {
        let awaiting_action = self.awaiting_rebind.as_ref().map(|pending| pending.action_id.as_str());
        let item_action = item
            .action
            .as_ref()
            .and_then(|route| route.payload_str("action_id"));
        if awaiting_action.is_some() && awaiting_action == item_action {
            Some("Press a new key or button now".to_owned())
        } else {
            item.detail.clone()
        }
    }
}

fn component_line(component: &UiComponentNode) -> String {
    let selector = if component.state_tags.iter().any(|tag| tag == "selected" || tag == "hovered") { ">" } else { " " };
    let mut line = format!("{selector} {}", component.text);
    if let Some(value) = component.value.as_deref().filter(|value| !value.trim().is_empty()) {
        line.push_str(" = ");
        line.push_str(value);
    }
    if let Some(detail) = component.detail.as_deref().filter(|detail| !detail.trim().is_empty()) {
        line.push_str("  - ");
        line.push_str(detail);
    }
    line
}
