use super::*;
use serde::{Deserialize, Serialize};

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
