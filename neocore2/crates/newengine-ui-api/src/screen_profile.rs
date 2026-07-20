// Split from lib.rs to keep the UI API DTO surface navigable.
// This file is included flat from lib.rs to preserve the existing public API.

/// UI composition profile selected by product/editor configuration.
///
/// This is a UI-domain profile, not a backend domain. Selecting a screen profile
/// chooses which `engine.ui` layout tree is published; it must not select or
/// mutate render, scene, world, ECS or asset providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiScreenProfile {
    /// Full editor shell: viewport slot plus editor panels and diagnostics.
    Editor,
    /// Clean runtime presentation: game viewport plus game-authored UI roots only.
    Game,
    /// No visual presentation; useful for future server/test runners.
    Headless,
}

impl Default for UiScreenProfile {
    #[inline]
    fn default() -> Self {
        Self::Game
    }
}

impl UiScreenProfile {
    #[inline]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Editor => "editor",
            Self::Game => "game",
            Self::Headless => "headless",
        }
    }

    #[inline]
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "editor" | "editor_screen" | "editor-screen" => Some(Self::Editor),
            "game" | "game_screen" | "game-screen" | "runtime" => Some(Self::Game),
            "headless" | "server" | "none" => Some(Self::Headless),
            _ => None,
        }
    }
}

/// Editor toolbar runtime mode.
///
/// This lives in `newengine-ui-api` because it is a UI/editor intent DTO,
/// not a concrete gameplay system or renderer command. Runtime modules may
/// consume it to decide whether the editor viewport is a paused preview,
/// a simulation preview, or a full play-in-editor session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiEditorRuntimeMode {
    Edit,
    Simulate,
    Play,
}

impl Default for UiEditorRuntimeMode {
    #[inline]
    fn default() -> Self {
        Self::Edit
    }
}

impl UiEditorRuntimeMode {
    #[inline]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Edit => "edit",
            Self::Simulate => "simulate",
            Self::Play => "play",
        }
    }
}

/// Resource published by the editor shell each frame.
///
/// The render/world runtime consumes this as an intent boundary: edit mode keeps
/// simulation and direct player control stopped; simulate mode runs world
/// simulation without player possession; play mode enables play-in-editor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiEditorRuntimeState {
    pub version: u32,
    pub frame_index: u64,
    pub mode: UiEditorRuntimeMode,
    pub source_surface: String,
    pub reason: String,
}

impl Default for UiEditorRuntimeState {
    fn default() -> Self {
        Self {
            version: 1,
            frame_index: 0,
            mode: UiEditorRuntimeMode::Edit,
            source_surface: UI_SURFACE_EDITOR_SHELL.to_owned(),
            reason: "editor default: simulation stopped until Simulate or Play".to_owned(),
        }
    }
}

/// Pixel-space viewport slot published by the editor shell.
///
/// The renderer consumes this DTO to render the world into an offscreen viewport
/// target, and the UI compositor samples that target inside the declared block.
/// This prevents the editor from drawing chrome over a full-screen game frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiViewportSlot {
    pub version: u32,
    pub frame_index: u64,
    pub surface_id: String,
    pub x_px: f32,
    pub y_px: f32,
    pub w_px: f32,
    pub h_px: f32,
    pub input_enabled: bool,
    pub simulation_enabled: bool,
    pub runtime_mode: UiEditorRuntimeMode,
}

impl Default for UiViewportSlot {
    fn default() -> Self {
        Self {
            version: 1,
            frame_index: 0,
            surface_id: "engine.render.viewport.primary".to_owned(),
            x_px: 0.0,
            y_px: 0.0,
            w_px: 0.0,
            h_px: 0.0,
            input_enabled: false,
            simulation_enabled: false,
            runtime_mode: UiEditorRuntimeMode::Edit,
        }
    }
}

impl UiViewportSlot {
    #[inline]
    pub fn extent_px(&self) -> (u32, u32) {
        (
            self.w_px.max(0.0).round() as u32,
            self.h_px.max(0.0).round() as u32,
        )
    }

    #[inline]
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x_px && x <= self.x_px + self.w_px && y >= self.y_px && y <= self.y_px + self.h_px
    }
}

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

/// Input focus policy attached to a screen profile.
///
/// Runtime systems use this as a provider-safe DTO. It never exposes native
/// window handles, renderer objects or raw ECS storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiScreenInputFocusPolicy {
    /// Editor shell owns high-level input focus. Gameplay movement is gated;
    /// editor tools may still route viewport navigation through their own policy.
    EditorShell,
    /// An authored UI surface owns focus, for example a main menu or frontend.
    /// Gameplay movement and possession remain gated until the presentation flow
    /// transitions to a viewport-owned state.
    UiSurface,
    /// Game viewport owns input focus. Editor panels are absent.
    GameViewport,
    /// No screen focus is active.
    Headless,
}

impl Default for UiScreenInputFocusPolicy {
    #[inline]
    fn default() -> Self {
        Self::GameViewport
    }
}

/// Generic product presentation-flow state shared between runtime-host, UI,
/// scene bootstrap and renderer readiness.
///
/// State ids, documents and action routes remain product-authored data. Engine
/// modules consume only the policy flags and readiness signal, so adding a new
/// frontend or replacing the main menu does not require renderer/scene branches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiPresentationFlowState {
    pub version: u32,
    pub frame_index: u64,
    pub flow_id: String,
    pub state_id: String,
    pub active_surface_id: Option<String>,
    pub blocks_world_bootstrap: bool,
    pub blocks_gameplay_input: bool,
    pub runtime_ready: bool,
    pub reason: String,
}

impl Default for UiPresentationFlowState {
    fn default() -> Self {
        Self {
            version: 1,
            frame_index: 0,
            flow_id: String::new(),
            state_id: String::new(),
            active_surface_id: None,
            blocks_world_bootstrap: false,
            blocks_gameplay_input: false,
            runtime_ready: false,
            reason: "presentation flow disabled".to_owned(),
        }
    }
}

impl UiPresentationFlowState {
    #[inline]
    pub const fn allows_world_bootstrap(&self) -> bool {
        !self.blocks_world_bootstrap
    }

    #[inline]
    pub const fn allows_gameplay_input(&self) -> bool {
        !self.blocks_gameplay_input && self.runtime_ready
    }

    #[inline]
    pub fn mark_runtime_ready(&mut self, frame_index: u64, reason: impl Into<String>) {
        self.frame_index = frame_index;
        self.runtime_ready = true;
        self.reason = reason.into();
    }
}

/// Declarative slot/panel request produced by a screen profile.
///
/// `data_contract` is the important field: editor panels consume readonly DTOs,
/// snapshots and opaque handles. They must not receive native `EntityId`, raw
/// `World`, renderer-private handles, or provider-owned objects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct UiScreenPanelDescriptor {
    pub slot_id: String,
    pub label: String,
    pub surface_id: String,
    pub source_gateway: String,
    pub data_contract: String,
    pub required: bool,
    pub debug_only: bool,
    pub tags: Vec<String>,
}
/// Runtime/editor screen profile descriptor.
///
/// This DTO is intentionally generic so more products can define their own
/// profile layouts without adding `engine.editor_screen` or any other god-domain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiScreenProfileDescriptor {
    pub version: u32,
    pub profile: UiScreenProfile,
    pub layout_id: String,
    pub surface_id: String,
    pub viewport_surface_id: String,
    pub game_ui_root_surface_id: Option<String>,
    pub game_ui_document_ref: Option<String>,
    pub input_focus_policy: UiScreenInputFocusPolicy,
    pub panels: Vec<UiScreenPanelDescriptor>,
    pub diagnostics: Vec<String>,
}

impl Default for UiScreenProfileDescriptor {
    fn default() -> Self {
        Self {
            version: 1,
            profile: UiScreenProfile::Game,
            layout_id: "engine.ui.screen.game.v1".to_owned(),
            surface_id: UI_SURFACE_SCREEN_ROOT.to_owned(),
            viewport_surface_id: "engine.render.viewport.primary".to_owned(),
            game_ui_root_surface_id: None,
            game_ui_document_ref: None,
            input_focus_policy: UiScreenInputFocusPolicy::GameViewport,
            panels: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}

/// Resource published by the host each frame so systems and diagnostics can see
/// the active UI composition profile without querying concrete providers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiScreenProfileState {
    pub version: u32,
    pub frame_index: u64,
    pub descriptor: UiScreenProfileDescriptor,
}

impl Default for UiScreenProfileState {
    fn default() -> Self {
        Self {
            version: 1,
            frame_index: 0,
            descriptor: UiScreenProfileDescriptor::default(),
        }
    }
}

pub const UI_SURFACE_SCREEN_ROOT: &str = "engine.ui.screen";
pub const UI_SURFACE_EDITOR_SHELL: &str = "engine.ui.screen.editor";
pub const UI_SURFACE_GAME_PRESENTATION: &str = "engine.ui.screen.game";

#[cfg(test)]
mod presentation_flow_tests {
    use super::*;

    #[test]
    fn presentation_flow_gates_bootstrap_and_input_independently() {
        let state = UiPresentationFlowState {
            blocks_world_bootstrap: true,
            blocks_gameplay_input: true,
            ..UiPresentationFlowState::default()
        };
        assert!(!state.allows_world_bootstrap());
        assert!(!state.allows_gameplay_input());
    }

    #[test]
    fn runtime_ready_signal_is_provider_neutral() {
        let mut state = UiPresentationFlowState::default();
        state.mark_runtime_ready(42, "launch gate released");
        assert!(state.runtime_ready);
        assert_eq!(state.frame_index, 42);
        assert_eq!(state.reason, "launch gate released");
    }
}
