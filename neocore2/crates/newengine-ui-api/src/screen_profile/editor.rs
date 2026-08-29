use super::*;
use serde::{Deserialize, Serialize};

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
    /// Runtime presentation. Editing tools, when installed, are plugin overlays on this same live world.
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
            Self::Game => "game",
            Self::Headless => "headless",
        }
    }

    #[inline]
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
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

/// Projection preset selected by the editor viewport chrome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiEditorViewportProjection {
    Perspective,
    Top,
    Front,
    Side,
}

impl Default for UiEditorViewportProjection {
    fn default() -> Self {
        Self::Perspective
    }
}

impl UiEditorViewportProjection {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Perspective => "Perspective",
            Self::Top => "Top",
            Self::Front => "Front",
            Self::Side => "Side",
        }
    }
}

/// View-mode intent for the editor viewport. Render backends may map these
/// generic modes to their own debug/material pipelines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiEditorViewportShading {
    Lit,
    Unlit,
    Wireframe,
}

impl Default for UiEditorViewportShading {
    fn default() -> Self {
        Self::Lit
    }
}

impl UiEditorViewportShading {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Lit => "Lit",
            Self::Unlit => "Unlit",
            Self::Wireframe => "Wireframe",
        }
    }
}

/// Active transform tool in the editor viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiEditorTransformMode {
    Select,
    Translate,
    Rotate,
    Scale,
}

impl Default for UiEditorTransformMode {
    fn default() -> Self {
        Self::Translate
    }
}

impl UiEditorTransformMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Translate => "Move",
            Self::Rotate => "Rotate",
            Self::Scale => "Scale",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiEditorTransformSpace {
    World,
    Local,
}

impl Default for UiEditorTransformSpace {
    fn default() -> Self {
        Self::World
    }
}

impl UiEditorTransformSpace {
    pub const fn label(self) -> &'static str {
        match self {
            Self::World => "World",
            Self::Local => "Local",
        }
    }
}

/// Provider-safe editor viewport intent published each frame. UI owns the
/// interaction state; camera, scene and render gateways consume the DTO without
/// exposing backend objects to the editor shell.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiEditorViewportState {
    pub version: u32,
    pub frame_index: u64,
    pub projection: UiEditorViewportProjection,
    pub shading: UiEditorViewportShading,
    pub transform_mode: UiEditorTransformMode,
    pub transform_space: UiEditorTransformSpace,
    pub show_grid: bool,
    pub show_collision: bool,
    pub show_bounds: bool,
    pub gizmo_visible: bool,
    pub translation_snap_enabled: bool,
    pub translation_snap_units: f32,
    pub rotation_snap_enabled: bool,
    pub rotation_snap_degrees: f32,
    pub scale_snap_enabled: bool,
    pub scale_snap_percent: f32,
}

impl Default for UiEditorViewportState {
    fn default() -> Self {
        Self {
            version: 1,
            frame_index: 0,
            projection: UiEditorViewportProjection::Perspective,
            shading: UiEditorViewportShading::Lit,
            transform_mode: UiEditorTransformMode::Translate,
            transform_space: UiEditorTransformSpace::World,
            show_grid: true,
            show_collision: false,
            show_bounds: false,
            gizmo_visible: true,
            translation_snap_enabled: true,
            translation_snap_units: 10.0,
            rotation_snap_enabled: true,
            rotation_snap_degrees: 10.0,
            scale_snap_enabled: false,
            scale_snap_percent: 1.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiEditorSceneEntitySnapshot {
    pub entity_key: u64,
    pub parent_key: Option<u64>,
    pub name: String,
    pub kind: String,
    pub selected: bool,
    pub components: Vec<String>,
}

impl Default for UiEditorSceneEntitySnapshot {
    fn default() -> Self {
        Self {
            entity_key: 0,
            parent_key: None,
            name: String::new(),
            kind: "Actor".to_owned(),
            selected: false,
            components: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiEditorSceneSnapshot {
    pub version: u32,
    pub frame_index: u64,
    pub entities: Vec<UiEditorSceneEntitySnapshot>,
    pub selected_keys: Vec<u64>,
}

impl Default for UiEditorSceneSnapshot {
    fn default() -> Self {
        Self {
            version: 1,
            frame_index: 0,
            entities: Vec::new(),
            selected_keys: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct UiEditorInspectorTransformSnapshot {
    pub position: [f32; 3],
    pub rotation_degrees: [f32; 3],
    pub scale: [f32; 3],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiEditorInspectorSnapshot {
    pub version: u32,
    pub frame_index: u64,
    pub entity_key: Option<u64>,
    pub name: String,
    pub kind: String,
    pub selection_count: usize,
    pub transform: Option<UiEditorInspectorTransformSnapshot>,
    pub components: Vec<String>,
}

impl Default for UiEditorInspectorSnapshot {
    fn default() -> Self {
        Self {
            version: 1,
            frame_index: 0,
            entity_key: None,
            name: String::new(),
            kind: String::new(),
            selection_count: 0,
            transform: None,
            components: Vec::new(),
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
    pub paused: bool,
    pub source_surface: String,
    pub reason: String,
}

impl Default for UiEditorRuntimeState {
    fn default() -> Self {
        Self {
            version: 1,
            frame_index: 0,
            mode: UiEditorRuntimeMode::Edit,
            paused: false,
            source_surface: UI_SURFACE_EDITOR_SHELL.to_owned(),
            reason: "editor default: simulation stopped until Simulate or Play".to_owned(),
        }
    }
}

/// Shared activation contract for the live in-game world editor.
///
/// This is intentionally a UI/runtime intent DTO: the editing-tools plugin and
/// engine runtime own the behavior, while screen composition consumes only this
/// state to show or hide editor chrome. Free-fly and noclip are part of the
/// unified Editor Mode so the user never has to discover separate debug toggles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiInGameEditorState {
    pub version: u32,
    pub frame_index: u64,
    pub enabled: bool,
    pub free_fly: bool,
    pub noclip: bool,
    pub save_available: bool,
    pub dirty_placements: usize,
    pub pending_creates: usize,
    pub pending_deletes: usize,
    pub last_save_succeeded: Option<bool>,
    pub last_save_message: String,
}

impl Default for UiInGameEditorState {
    fn default() -> Self {
        Self {
            version: 1,
            frame_index: 0,
            enabled: false,
            free_fly: true,
            noclip: true,
            save_available: true,
            dirty_placements: 0,
            pending_creates: 0,
            pending_deletes: 0,
            last_save_succeeded: None,
            last_save_message: String::new(),
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
    pub paused: bool,
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
            paused: false,
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
