use serde::{Deserialize, Serialize};

/// Per-panel dock state for the editor shell.
///
/// It is intentionally a DTO: panels remain UI compositions, while consumers can
/// observe whether a panel is visible, collapsed, floating-ready or disabled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiDockPanelRuntimeState {
    pub slot_id: String,
    pub visible: bool,
    pub collapsed: bool,
    pub detachable: bool,
    pub resizable: bool,
    pub active: bool,
    pub hovered: bool,
    pub disabled: bool,
}

impl Default for UiDockPanelRuntimeState {
    fn default() -> Self {
        Self {
            slot_id: String::new(),
            visible: true,
            collapsed: false,
            detachable: true,
            resizable: true,
            active: false,
            hovered: false,
            disabled: false,
        }
    }
}

/// Aggregate dock layout state published by the editor shell.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiDockLayoutState {
    pub version: u32,
    pub frame_index: u64,
    pub panels: Vec<UiDockPanelRuntimeState>,
}

impl UiDockLayoutState {
    #[inline]
    pub fn panel_visible(&self, slot_id: &str) -> bool {
        self.panels
            .iter()
            .find(|panel| panel.slot_id == slot_id)
            .map(|panel| panel.visible && !panel.collapsed && !panel.disabled)
            .unwrap_or(true)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiToastSeverity {
    Info,
    Success,
    Warning,
    Error,
}

impl Default for UiToastSeverity {
    #[inline]
    fn default() -> Self {
        Self::Info
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiToastNotification {
    pub id: String,
    pub title: String,
    pub detail: String,
    pub progress_permille: Option<u16>,
    pub severity: UiToastSeverity,
    pub source: String,
}

impl Default for UiToastNotification {
    fn default() -> Self {
        Self {
            id: String::new(),
            title: String::new(),
            detail: String::new(),
            progress_permille: None,
            severity: UiToastSeverity::Info,
            source: "engine.ui".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiToastStack {
    pub version: u32,
    pub frame_index: u64,
    pub notifications: Vec<UiToastNotification>,
}
