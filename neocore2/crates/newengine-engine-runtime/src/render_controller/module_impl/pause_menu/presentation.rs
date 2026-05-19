#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ui_api::{UiPauseMenuItem, UiPauseMenuState, UiPauseMenuTheme};
use newengine_ui_navigation_api::MenuItem;

use super::*;

impl RenderPauseMenuRuntimeState {
    pub(super) fn build_ui_state(&self, visual_visible: bool) -> UiPauseMenuState {
        if !visual_visible {
            return UiPauseMenuState::hidden();
        }

        let document = self.menu.document();
        let current_page = self.menu.current_page();
        let page_title = current_page
            .map(|page| page.title.as_str())
            .filter(|title| !title.is_empty())
            .unwrap_or("Pause Menu");
        let page_subtitle = current_page
            .map(|page| page.subtitle.as_str())
            .filter(|subtitle| !subtitle.is_empty())
            .unwrap_or_else(|| document.subtitle.as_str());

        let mut footer_lines = current_page
            .and_then(|page| (!page.footer_lines.is_empty()).then(|| page.footer_lines.clone()))
            .unwrap_or_else(|| document.footer_lines.clone());
        if footer_lines.is_empty() {
            footer_lines = vec![
                "ESC / START - Resume or close pause menu".to_owned(),
                "ARROWS / DPAD - Navigate".to_owned(),
                "ENTER / A / CLICK - Confirm".to_owned(),
                "BACKSPACE / B - Back".to_owned(),
            ];
        }
        if let Some(pending) = self.awaiting_rebind.as_ref() {
            footer_lines.insert(0, format!("Listening for new input: {}", pending.label));
            footer_lines.insert(1, "Press a key, mouse button, or gamepad button".to_owned());
        }

        let a = ease_out_cubic(self.visual_alpha);
        UiPauseMenuState {
            version: 1,
            surface_id: document.surface_id.clone(),
            visible: visual_visible,
            paused: self.open,
            page: self.menu.current_page_id().to_owned(),
            title: if document.title.is_empty() { "PAUSE".to_owned() } else { document.title.clone() },
            subtitle: if page_subtitle.is_empty() {
                page_title.to_owned()
            } else {
                format!("{} / {}", page_subtitle, page_title)
            },
            items: self.items(),
            selected_index: self.menu.selected_index(),
            hovered_index: self.menu.hovered_index(),
            footer_lines,
            animation_alpha: a,
            backdrop_opacity: 0.94 * a,
            blur_radius_px: 22.0 * a,
            theme: UiPauseMenuTheme::default(),
            message: self.feedback.as_ref().map(PauseMenuEventFeedback::to_ui_message),
        }
    }

    fn items(&self) -> Vec<UiPauseMenuItem> {
        self.menu
            .current_items()
            .iter()
            .map(|item| self.item_to_ui(item))
            .collect()
    }

    fn item_to_ui(&self, item: &MenuItem) -> UiPauseMenuItem {
        let mut out = UiPauseMenuItem::new(item.id.clone(), item.label.clone())
            .emphasized(item.emphasized)
            .with_tone(tone_from_menu(item.tone));

        if let Some(value) = self.item_value(item) {
            out = out.with_value(value);
        }
        if let Some(detail) = self.item_detail(item) {
            out = out.with_detail(detail);
        }
        out
    }

    fn item_value(&self, item: &MenuItem) -> Option<String> {
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

    fn item_detail(&self, item: &MenuItem) -> Option<String> {
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
