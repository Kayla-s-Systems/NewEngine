#![forbid(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const ENGINE_PAUSE_MENU_DOCUMENT_ID: &str = "engine.pause_menu";
/// Canonical runtime UI source. The pause menu is authored as `.neui` and must be
/// compiled by `engine.assets.ui` before `engine.ui` mounts it. Runtime JSON menu
/// assets are intentionally not supported as compatibility fallback.
pub const ENGINE_PAUSE_MENU_SURFACE_REF: &str = "assets/ui/engine/pause_menu.neui@surface";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MenuItemTone {
    Normal,
    Accent,
    Danger,
    Disabled,
}

impl Default for MenuItemTone {
    #[inline]
    fn default() -> Self {
        Self::Normal
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MenuDocument {
    pub id: String,
    #[serde(default = "default_version")]
    pub version: u32,
    pub surface_id: String,
    #[serde(default = "default_root_page")]
    pub root_page: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub subtitle: String,
    #[serde(default)]
    pub footer_lines: Vec<String>,
    #[serde(default)]
    pub pages: Vec<MenuPage>,
}

impl MenuDocument {
    #[inline]
    pub fn from_json_str(src: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str::<Self>(src).map(Self::canonicalized)
    }

    pub fn canonicalized(mut self) -> Self {
        self.id = self.id.trim().to_owned();
        self.surface_id = self.surface_id.trim().to_owned();
        self.root_page = self.root_page.trim().to_owned();
        self.title = self.title.trim().to_owned();
        self.subtitle = self.subtitle.trim().to_owned();
        self.footer_lines = normalize_non_empty_strings(self.footer_lines);
        for page in &mut self.pages {
            page.canonicalize_in_place();
        }
        self
    }

    #[inline]
    pub fn page(&self, page_id: &str) -> Option<&MenuPage> {
        self.pages.iter().find(|page| page.id == page_id)
    }

    #[inline]
    pub fn root(&self) -> Option<&MenuPage> {
        self.page(&self.root_page)
    }

    #[inline]
    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() {
            return Err("menu document id is empty".to_owned());
        }
        if self.surface_id.is_empty() {
            return Err(format!("menu document '{}' surface_id is empty", self.id));
        }
        if self.root_page.is_empty() {
            return Err(format!("menu document '{}' root_page is empty", self.id));
        }
        if self.root().is_none() {
            return Err(format!(
                "menu document '{}' root_page '{}' is missing",
                self.id, self.root_page
            ));
        }
        for page in &self.pages {
            page.validate(self)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MenuPage {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub subtitle: String,
    #[serde(default)]
    pub parent_page: Option<String>,
    #[serde(default)]
    pub footer_lines: Vec<String>,
    #[serde(default)]
    pub items: Vec<MenuItem>,
    #[serde(default)]
    pub back_route: Option<MenuActionRoute>,
}

impl MenuPage {
    fn canonicalize_in_place(&mut self) {
        self.id = self.id.trim().to_owned();
        self.title = self.title.trim().to_owned();
        self.subtitle = self.subtitle.trim().to_owned();
        self.parent_page = self.parent_page.take().and_then(|value| normalize_optional_string(&value));
        self.footer_lines = normalize_non_empty_strings(std::mem::take(&mut self.footer_lines));
        if let Some(route) = &mut self.back_route {
            route.canonicalize_in_place();
        }
        for item in &mut self.items {
            item.canonicalize_in_place();
        }
    }

    fn validate(&self, document: &MenuDocument) -> Result<(), String> {
        if self.id.is_empty() {
            return Err(format!("menu document '{}' contains page with empty id", document.id));
        }
        if let Some(parent) = self.parent_page.as_deref() {
            if document.page(parent).is_none() {
                return Err(format!(
                    "menu page '{}' references missing parent_page '{}'",
                    self.id, parent
                ));
            }
        }
        for item in &self.items {
            item.validate(document, self)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MenuItem {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub emphasized: bool,
    #[serde(default)]
    pub tone: MenuItemTone,
    #[serde(default)]
    pub dynamic_value: Option<String>,
    #[serde(default, alias = "route")]
    pub action: Option<MenuActionRoute>,
    #[serde(default)]
    pub nav_left: Option<MenuActionRoute>,
    #[serde(default)]
    pub nav_right: Option<MenuActionRoute>,
}

impl MenuItem {
    fn canonicalize_in_place(&mut self) {
        self.id = self.id.trim().to_owned();
        self.label = self.label.trim().to_owned();
        self.value = self.value.take().and_then(|value| normalize_optional_string(&value));
        self.detail = self.detail.take().and_then(|value| normalize_optional_string(&value));
        self.dynamic_value = self.dynamic_value.take().and_then(|value| normalize_optional_string(&value));
        if let Some(route) = &mut self.action {
            route.canonicalize_in_place();
        }
        if let Some(route) = &mut self.nav_left {
            route.canonicalize_in_place();
        }
        if let Some(route) = &mut self.nav_right {
            route.canonicalize_in_place();
        }
    }

    fn validate(&self, _document: &MenuDocument, page: &MenuPage) -> Result<(), String> {
        if self.id.is_empty() {
            return Err(format!("menu page '{}' contains item with empty id", page.id));
        }
        if self.label.is_empty() {
            return Err(format!("menu item '{}' has empty label", self.id));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MenuActionRoute {
    pub id: String,
    pub source: String,
    pub target: String,
    pub event: String,
    #[serde(default)]
    pub payload: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub transition: Option<MenuTransition>,
    #[serde(default)]
    pub feedback: Option<MenuFeedbackEvent>,
    #[serde(default)]
    pub audio: Option<String>,
}

impl MenuActionRoute {
    fn canonicalize_in_place(&mut self) {
        self.id = self.id.trim().to_owned();
        self.source = self.source.trim().to_owned();
        self.target = self.target.trim().to_owned();
        self.event = self.event.trim().to_owned();
        self.audio = self.audio.take().and_then(|value| normalize_optional_string(&value));
        if let Some(transition) = &mut self.transition {
            transition.canonicalize_in_place();
        }
        if let Some(feedback) = &mut self.feedback {
            feedback.canonicalize_in_place();
        }
    }

    #[inline]
    pub fn payload_str(&self, key: &str) -> Option<&str> {
        self.payload.get(key).and_then(serde_json::Value::as_str)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MenuTransitionKind {
    None,
    OpenPage,
    Back,
    Close,
}

impl Default for MenuTransitionKind {
    #[inline]
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MenuTransition {
    #[serde(default)]
    pub kind: MenuTransitionKind,
    #[serde(default)]
    pub page: Option<String>,
    #[serde(default = "default_true")]
    pub reset_selection: bool,
}

impl MenuTransition {
    #[inline]
    pub fn close() -> Self {
        Self { kind: MenuTransitionKind::Close, page: None, reset_selection: true }
    }

    #[inline]
    pub fn open_page(page: impl Into<String>) -> Self {
        Self { kind: MenuTransitionKind::OpenPage, page: Some(page.into()), reset_selection: true }
    }

    fn canonicalize_in_place(&mut self) {
        self.page = self.page.take().and_then(|value| normalize_optional_string(&value));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MenuFeedbackSeverity {
    Info,
    Success,
    Warning,
    Danger,
}

impl Default for MenuFeedbackSeverity {
    #[inline]
    fn default() -> Self {
        Self::Info
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MenuFeedbackEvent {
    pub title: String,
    #[serde(default)]
    pub detail: String,
    #[serde(default)]
    pub severity: MenuFeedbackSeverity,
    #[serde(default = "default_feedback_ttl_sec")]
    pub ttl_sec: f32,
}

impl MenuFeedbackEvent {
    #[inline]
    pub fn new(
        title: impl Into<String>,
        detail: impl Into<String>,
        severity: MenuFeedbackSeverity,
    ) -> Self {
        Self {
            title: title.into(),
            detail: detail.into(),
            severity,
            ttl_sec: default_feedback_ttl_sec(),
        }
    }

    fn canonicalize_in_place(&mut self) {
        self.title = self.title.trim().to_owned();
        self.detail = self.detail.trim().to_owned();
        self.ttl_sec = self.ttl_sec.clamp(0.05, 30.0);
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MenuSelectionState {
    pub page: String,
    #[serde(default)]
    pub selected_index: usize,
    #[serde(default)]
    pub hovered_index: Option<usize>,
}

fn normalize_optional_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

fn normalize_non_empty_strings(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .filter_map(|value| normalize_optional_string(&value))
        .collect()
}

#[inline]
fn default_version() -> u32 {
    1
}

#[inline]
fn default_root_page() -> String {
    "root".to_owned()
}

#[inline]
fn default_true() -> bool {
    true
}

#[inline]
fn default_feedback_ttl_sec() -> f32 {
    2.25
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_document() {
        let doc = MenuDocument::from_json_str(r#"{
          "id":"engine.pause_menu",
          "surface_id":"engine.pause_menu",
          "root_page":"root",
          "pages":[{"id":"root","items":[{"id":"resume","label":"Resume"}]}]
        }"#).unwrap();
        doc.validate().unwrap();
        assert_eq!(doc.root().unwrap().items[0].label, "Resume");
    }
}
