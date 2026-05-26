#![forbid(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const ENGINE_PRIMARY_UI_DOCUMENT_ID: &str = "engine.ui.primary";
/// Canonical runtime UI source. The primary retained UI surface is authored as `.neui` and must be
/// compiled by `engine.assets.ui` before `engine.ui` mounts it. Runtime JSON navigation
/// assets are intentionally not supported as compatibility fallback.
pub const ENGINE_PRIMARY_UI_SURFACE_REF: &str = "assets/ui/engine/primary.neui@surface";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiNodeNavigationTone {
    Normal,
    Accent,
    Danger,
    Disabled,
}

impl Default for UiNodeNavigationTone {
    #[inline]
    fn default() -> Self {
        Self::Normal
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiNodeNavigationDocument {
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
    pub pages: Vec<UiNodeNavigationPage>,
}

impl UiNodeNavigationDocument {
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
    pub fn page(&self, page_id: &str) -> Option<&UiNodeNavigationPage> {
        self.pages.iter().find(|page| page.id == page_id)
    }

    #[inline]
    pub fn root(&self) -> Option<&UiNodeNavigationPage> {
        self.page(&self.root_page)
    }

    #[inline]
    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() {
            return Err("ui navigation document id is empty".to_owned());
        }
        if self.surface_id.is_empty() {
            return Err(format!("ui navigation document '{}' surface_id is empty", self.id));
        }
        if self.root_page.is_empty() {
            return Err(format!("ui navigation document '{}' root_page is empty", self.id));
        }
        if self.root().is_none() {
            return Err(format!(
                "ui navigation document '{}' root_page '{}' is missing",
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
pub struct UiNodeNavigationPage {
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
    pub items: Vec<UiNodeNavigationItem>,
    #[serde(default)]
    pub back_route: Option<UiNodeActionRoute>,
}

impl UiNodeNavigationPage {
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

    fn validate(&self, document: &UiNodeNavigationDocument) -> Result<(), String> {
        if self.id.is_empty() {
            return Err(format!("ui navigation document '{}' contains page with empty id", document.id));
        }
        if let Some(parent) = self.parent_page.as_deref() {
            if document.page(parent).is_none() {
                return Err(format!(
                    "ui navigation page '{}' references missing parent_page '{}'",
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
pub struct UiNodeNavigationItem {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub emphasized: bool,
    #[serde(default)]
    pub tone: UiNodeNavigationTone,
    #[serde(default)]
    pub dynamic_value: Option<String>,
    #[serde(default)]
    pub action: Option<UiNodeActionRoute>,
    #[serde(default)]
    pub nav_left: Option<UiNodeActionRoute>,
    #[serde(default)]
    pub nav_right: Option<UiNodeActionRoute>,
}

impl UiNodeNavigationItem {
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

    fn validate(&self, _document: &UiNodeNavigationDocument, page: &UiNodeNavigationPage) -> Result<(), String> {
        if self.id.is_empty() {
            return Err(format!("ui navigation page '{}' contains item with empty id", page.id));
        }
        if self.label.is_empty() {
            return Err(format!("ui navigation item '{}' has empty label", self.id));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiNodeActionRoute {
    pub id: String,
    pub source: String,
    pub target: String,
    pub event: String,
    #[serde(default)]
    pub payload: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub transition: Option<UiNodeTransition>,
    #[serde(default)]
    pub feedback: Option<UiNodeFeedbackEvent>,
    #[serde(default)]
    pub audio: Option<String>,
}

impl UiNodeActionRoute {
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
pub enum UiNodeTransitionKind {
    None,
    OpenPage,
    Back,
    Close,
}

impl Default for UiNodeTransitionKind {
    #[inline]
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiNodeTransition {
    #[serde(default)]
    pub kind: UiNodeTransitionKind,
    #[serde(default)]
    pub page: Option<String>,
    #[serde(default = "default_true")]
    pub reset_selection: bool,
}

impl UiNodeTransition {
    #[inline]
    pub fn close() -> Self {
        Self { kind: UiNodeTransitionKind::Close, page: None, reset_selection: true }
    }

    #[inline]
    pub fn open_page(page: impl Into<String>) -> Self {
        Self { kind: UiNodeTransitionKind::OpenPage, page: Some(page.into()), reset_selection: true }
    }

    fn canonicalize_in_place(&mut self) {
        self.page = self.page.take().and_then(|value| normalize_optional_string(&value));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiNodeFeedbackSeverity {
    Info,
    Success,
    Warning,
    Danger,
}

impl Default for UiNodeFeedbackSeverity {
    #[inline]
    fn default() -> Self {
        Self::Info
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiNodeFeedbackEvent {
    pub title: String,
    #[serde(default)]
    pub detail: String,
    #[serde(default)]
    pub severity: UiNodeFeedbackSeverity,
    #[serde(default = "default_feedback_ttl_sec")]
    pub ttl_sec: f32,
}

impl UiNodeFeedbackEvent {
    #[inline]
    pub fn new(
        title: impl Into<String>,
        detail: impl Into<String>,
        severity: UiNodeFeedbackSeverity,
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
pub struct UiNodeSelectionState {
    pub page: String,
    #[serde(default)]
    pub selected_index: usize,
    #[serde(default)]
    pub hovered_index: Option<usize>,
}


#[derive(Debug, Clone, Default)]
pub struct UiNodeHitTestState {
    pub hovered_index: Option<usize>,
    pub pointer_primary_pressed: bool,
}

#[derive(Debug, Clone, Default)]
pub struct UiNodeNavigationInput {
    pub nav_x: i8,
    pub nav_y: i8,
    pub accept: bool,
    pub back: bool,
    pub hit_test: Option<UiNodeHitTestState>,
}

#[derive(Debug, Clone)]
pub struct UiNodeRouteDispatch {
    pub route: UiNodeActionRoute,
    pub source_item_id: Option<String>,
    pub source_label: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct UiNodeNavigationOutput {
    pub selection_changed: bool,
    pub route_dispatches: Vec<UiNodeRouteDispatch>,
    pub feedback: Vec<UiNodeFeedbackEvent>,
    pub transition: Option<UiNodeTransition>,
    pub close_requested: bool,
}

#[derive(Debug, Clone)]
pub struct UiNodeNavigationRuntime {
    document: UiNodeNavigationDocument,
    current_page: String,
    selected_by_page: BTreeMap<String, usize>,
    hovered_index: Option<usize>,
}

impl UiNodeNavigationRuntime {
    pub fn new(document: UiNodeNavigationDocument) -> Result<Self, String> {
        document.validate()?;
        let current_page = document.root_page.clone();
        Ok(Self {
            document,
            current_page,
            selected_by_page: BTreeMap::new(),
            hovered_index: None,
        })
    }

    #[inline]
    pub fn document(&self) -> &UiNodeNavigationDocument {
        &self.document
    }

    #[inline]
    pub fn reset_to_root(&mut self) {
        self.current_page = self.document.root_page.clone();
        self.hovered_index = None;
    }

    #[inline]
    pub fn current_page_id(&self) -> &str {
        &self.current_page
    }

    #[inline]
    pub fn current_page(&self) -> Option<&UiNodeNavigationPage> {
        self.document.page(&self.current_page)
    }

    #[inline]
    pub fn current_items(&self) -> &[UiNodeNavigationItem] {
        self.current_page().map(|page| page.items.as_slice()).unwrap_or(&[])
    }

    #[inline]
    pub fn hovered_index(&self) -> Option<usize> {
        self.hovered_index
    }

    #[inline]
    pub fn selected_index(&self) -> usize {
        *self.selected_by_page.get(&self.current_page).unwrap_or(&0)
    }

    #[inline]
    pub fn selection_state(&self) -> UiNodeSelectionState {
        UiNodeSelectionState {
            page: self.current_page.clone(),
            selected_index: self.selected_index(),
            hovered_index: self.hovered_index,
        }
    }

    pub fn handle_input(&mut self, input: UiNodeNavigationInput) -> UiNodeNavigationOutput {
        let mut output = UiNodeNavigationOutput::default();
        let item_count = self.current_items().len();
        if item_count == 0 {
            return output;
        }

        if input.back {
            self.activate_back(&mut output);
            return output;
        }

        if let Some(hit) = input.hit_test {
            self.hovered_index = hit.hovered_index.filter(|idx| *idx < item_count);
            if let Some(hovered) = self.hovered_index {
                if hovered != self.selected_index() {
                    self.set_selected_index(hovered);
                    output.selection_changed = true;
                }
            }
            if hit.pointer_primary_pressed && self.hovered_index.is_some() {
                self.activate_selected(&mut output);
                return output;
            }
        } else {
            self.hovered_index = None;
        }

        if input.nav_y != 0 {
            let dir = if input.nav_y > 0 { 1 } else { -1 };
            if self.move_selection(dir) {
                output.selection_changed = true;
            }
        }

        if input.nav_x < 0 {
            self.dispatch_selected_nav_route(UiNodeNavigationDirection::Left, &mut output);
            return output;
        }
        if input.nav_x > 0 {
            self.dispatch_selected_nav_route(UiNodeNavigationDirection::Right, &mut output);
            return output;
        }

        if input.accept {
            self.activate_selected(&mut output);
        }

        output
    }

    fn activate_selected(&mut self, output: &mut UiNodeNavigationOutput) {
        let Some(item) = self.current_items().get(self.selected_index()).cloned() else { return; };
        let Some(route) = item.action.clone() else { return; };
        self.dispatch_route(route, Some(item), output);
    }

    fn dispatch_selected_nav_route(&mut self, direction: UiNodeNavigationDirection, output: &mut UiNodeNavigationOutput) {
        let Some(item) = self.current_items().get(self.selected_index()).cloned() else { return; };
        let route = match direction {
            UiNodeNavigationDirection::Left => item.nav_left.clone(),
            UiNodeNavigationDirection::Right => item.nav_right.clone(),
        };
        let Some(route) = route else { return; };
        self.dispatch_route(route, Some(item), output);
    }

    fn activate_back(&mut self, output: &mut UiNodeNavigationOutput) {
        if let Some(route) = self.current_page().and_then(|page| page.back_route.clone()) {
            self.dispatch_route(route, None, output);
            return;
        }
        let transition = if self.current_page_id() == self.document.root_page {
            UiNodeTransition::close()
        } else if let Some(parent) = self.current_page().and_then(|page| page.parent_page.clone()) {
            UiNodeTransition::open_page(parent)
        } else {
            UiNodeTransition::close()
        };
        self.apply_transition(&transition, output);
    }

    fn dispatch_route(
        &mut self,
        route: UiNodeActionRoute,
        item: Option<UiNodeNavigationItem>,
        output: &mut UiNodeNavigationOutput,
    ) {
        if let Some(feedback) = route.feedback.clone() {
            output.feedback.push(feedback);
        }
        if let Some(transition) = route.transition.clone() {
            self.apply_transition(&transition, output);
        }
        output.route_dispatches.push(UiNodeRouteDispatch {
            route,
            source_item_id: item.as_ref().map(|item| item.id.clone()),
            source_label: item.as_ref().map(|item| item.label.clone()),
        });
    }

    fn apply_transition(&mut self, transition: &UiNodeTransition, output: &mut UiNodeNavigationOutput) {
        output.transition = Some(transition.clone());
        match transition.kind {
            UiNodeTransitionKind::None => {}
            UiNodeTransitionKind::OpenPage => {
                if let Some(page) = transition.page.as_deref() {
                    if self.document.page(page).is_some() {
                        self.current_page = page.to_owned();
                        self.hovered_index = None;
                        if transition.reset_selection {
                            self.set_selected_index(0);
                        }
                    }
                }
            }
            UiNodeTransitionKind::Back => {
                if let Some(parent) = self.current_page().and_then(|page| page.parent_page.clone()) {
                    if self.document.page(&parent).is_some() {
                        self.current_page = parent;
                        self.hovered_index = None;
                    }
                } else {
                    output.close_requested = true;
                }
            }
            UiNodeTransitionKind::Close => {
                output.close_requested = true;
                self.current_page = self.document.root_page.clone();
                self.hovered_index = None;
                if transition.reset_selection {
                    self.set_selected_index(0);
                }
            }
        }
    }

    fn move_selection(&mut self, delta: i32) -> bool {
        let len = self.current_items().len();
        if len == 0 {
            return false;
        }
        let current = self.selected_index() as i32;
        let next = (current + delta).rem_euclid(len as i32) as usize;
        if next == self.selected_index() {
            return false;
        }
        self.set_selected_index(next);
        true
    }

    #[inline]
    fn set_selected_index(&mut self, value: usize) {
        let max = self.current_items().len().saturating_sub(1);
        self.selected_by_page
            .insert(self.current_page.clone(), value.min(max));
    }
}

#[derive(Debug, Clone, Copy)]
enum UiNodeNavigationDirection {
    Left,
    Right,
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
        let doc = UiNodeNavigationDocument::from_json_str(r#"{
          "id":"engine.ui.primary",
          "surface_id":"engine.ui.primary",
          "root_page":"root",
          "pages":[{"id":"root","items":[{"id":"resume","label":"Resume"}]}]
        }"#).unwrap();
        doc.validate().unwrap();
        assert_eq!(doc.root().unwrap().items[0].label, "Resume");
    }
}
