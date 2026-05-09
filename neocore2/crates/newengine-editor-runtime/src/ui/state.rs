#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::{Arc, Mutex};

use egui;
use newengine_assets::AssetServiceClient;
use newengine_ecs::EntityId;
use newengine_editor_core::{CommandCollisionBody, CommandCollisionShape, CommandDisplayMode, EditorState, TransformSnapshot};
use newengine_gizmo::egui::EguiGizmo;
use newengine_materials::MaterialId;
use newengine_scene_io::SceneIoClient;
use newengine_ui::markup::UiMarkupDoc;
use newengine_ui::{UiHub, UiInputFrame};
use newengine_viewport::Viewport;

use crate::material_pipeline::MaterialPipeline;
use crate::plugin_manager::PluginManagerUi;
use crate::scene_bridge::SceneBridge;
use crate::ui::commands::EditorCommandBus;
use crate::ui::dock::EditorDockTab;
use crate::ui::extension_abi;
use crate::ui::icons;
use crate::ui::schema::{AssetSpawnContract, EditorSchemaRegistry};
use crate::viewport_bridge::ViewportBridge;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorkspacePreset {
    Minimal,
    Editing,
    Debug,
}

impl WorkspacePreset {
    pub(crate) const ALL: [Self; 3] = [Self::Minimal, Self::Editing, Self::Debug];

    #[inline]
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Minimal => "Minimal",
            Self::Editing => "Editing",
            Self::Debug => "Debug",
        }
    }
}

impl Default for WorkspacePreset {
    #[inline]
    fn default() -> Self {
        Self::Editing
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ViewportMode {
    Lit,
    Unlit,
    Wireframe,
    Collision,
}

impl ViewportMode {
    pub(crate) const ALL: [Self; 4] = [Self::Lit, Self::Unlit, Self::Wireframe, Self::Collision];

    #[inline]
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Lit => "Lit",
            Self::Unlit => "Unlit",
            Self::Wireframe => "Wireframe",
            Self::Collision => "Collision",
        }
    }
}

impl Default for ViewportMode {
    #[inline]
    fn default() -> Self {
        Self::Lit
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TransformSnapSettings {
    pub(crate) translate_enabled: bool,
    pub(crate) rotate_enabled: bool,
    pub(crate) scale_enabled: bool,
    pub(crate) translate_step: f32,
    pub(crate) rotate_step_deg: f32,
    pub(crate) scale_step: f32,
}

impl Default for TransformSnapSettings {
    #[inline]
    fn default() -> Self {
        Self {
            // Start with smooth editing by default.
            // Snapping remains available from the viewport "Snap" menu when explicitly needed.
            translate_enabled: false,
            rotate_enabled: false,
            scale_enabled: false,
            translate_step: 10.0,
            rotate_step_deg: 10.0,
            scale_step: 0.1,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CameraSpeedSettings {
    pub(crate) preset_index: usize,
    pub(crate) scalar: f32,
}

impl CameraSpeedSettings {
    pub(crate) const PRESET_LABELS: [&'static str; 8] =
        ["0.25x", "0.5x", "1x", "2x", "4x", "8x", "16x", "32x"];
    pub(crate) const PRESET_VALUES: [f32; 8] = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0, 32.0];

    #[inline]
    pub(crate) fn active_label(&self) -> &'static str {
        Self::PRESET_LABELS
            .get(self.preset_index)
            .copied()
            .unwrap_or("1x")
    }

    #[inline]
    pub(crate) fn clamp_preset_index(&mut self) {
        if self.preset_index >= Self::PRESET_VALUES.len() {
            self.preset_index = 2;
        }
        self.scalar = Self::PRESET_VALUES[self.preset_index];
    }

    #[inline]
    pub(crate) fn step_up(&mut self) {
        if self.preset_index + 1 < Self::PRESET_VALUES.len() {
            self.preset_index += 1;
        }
        self.clamp_preset_index();
    }

    #[inline]
    pub(crate) fn step_down(&mut self) {
        if self.preset_index > 0 {
            self.preset_index -= 1;
        }
        self.clamp_preset_index();
    }
}

impl Default for CameraSpeedSettings {
    #[inline]
    fn default() -> Self {
        Self {
            preset_index: 2,
            scalar: 1.0,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CommandPaletteState {
    pub(crate) open: bool,
    pub(crate) query: String,
    pub(crate) selected_index: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct SceneLayerVisibility {
    pub(crate) actors: bool,
    pub(crate) lights: bool,
    pub(crate) buildings: bool,
    pub(crate) units: bool,
    pub(crate) foliage: bool,
    pub(crate) debug: bool,
}

impl Default for SceneLayerVisibility {
    #[inline]
    fn default() -> Self {
        Self {
            actors: true,
            lights: true,
            buildings: true,
            units: true,
            foliage: true,
            debug: true,
        }
    }
}

impl Default for CommandPaletteState {
    #[inline]
    fn default() -> Self {
        Self {
            open: false,
            query: String::new(),
            selected_index: 0,
        }
    }
}


/// Minimal editor UI: foundation-first.
///
/// Responsibilities:
/// - Own UI state (selection cache, panel toggles, gizmo state).
/// - Bridge input/intent to the renderer via `ViewportBridge`.
/// - Render editor panels.
///
/// Engine-style editor shell.
///
/// Responsibilities:
/// - Own UI state (selection cache, panel layout, gizmo state).
/// - Bridge input/intent to the renderer via `ViewportBridge`.
/// - Render a dockable editor workspace.
pub struct EditorUiBuild {
    pub(crate) schema_registry: Arc<parking_lot::RwLock<EditorSchemaRegistry>>,
    pub(crate) extension_registry: Arc<parking_lot::RwLock<extension_abi::EditorExtensionAbiRegistry>>,
    pub(crate) shared_doc: Arc<Mutex<Option<Arc<UiMarkupDoc>>>>,

    pub(crate) viewport: Viewport,
    pub(crate) last_viewport_extent: Option<(u32, u32)>,

    pub(crate) viewport_bridge: Arc<ViewportBridge>,
    pub(crate) scene_bridge: Arc<SceneBridge>,
    pub(crate) plugins_bridge: Arc<crate::plugin_manager::PluginManagerBridge>,
    pub(crate) plugin_manager: Arc<Mutex<PluginManagerUi>>,

    pub(crate) ui_hub: UiHub,

    pub(crate) markup_state: newengine_ui::markup::UiState,

    pub(crate) icons: icons::EditorIconLoader,

    pub(crate) material_pipeline: MaterialPipeline,

    pub(crate) dock_state: egui_dock::DockState<EditorDockTab>,
    pub(crate) saved_dock_layout: Option<egui_dock::DockState<EditorDockTab>>,
    pub(crate) workspace_preset: WorkspacePreset,
    pub(crate) viewport_mode: ViewportMode,
    pub(crate) transform_snap: TransformSnapSettings,
    pub(crate) camera_speed: CameraSpeedSettings,
    pub(crate) command_palette: CommandPaletteState,

    pub(crate) assets: Option<AssetServiceClient>,
    pub(crate) asset_ui: AssetManagerUiState,
    pub(crate) asset_spawn_request: Option<PendingAssetSpawn>,

    pub(crate) scene_io: Option<SceneIoClient>,
    pub(crate) scene_io_ui: SceneIoUiState,

    pub(crate) outliner_filter: String,
    pub(crate) details_filter: String,
    pub(crate) asset_browser_filter: String,
    pub(crate) console_filter: String,
    pub(crate) scene_layers: SceneLayerVisibility,
    pub(crate) frame_input: UiInputFrame,

    // Viewport navigation interaction (UI-driven, not via global input plugin).
    //
    // Track MMB orbit/pan and RMB free-fly separately.
    // Mixing them causes a first-frame delta spike when capture toggles.
    pub(crate) last_nav_drag_pos: Option<egui::Pos2>,
    pub(crate) last_fly_drag_pos: Option<egui::Pos2>,

    /// Latched RMB free-fly capture state.
    pub(crate) fly_latch: newengine_viewport::nav::FlyRmbLatch,

    pub(crate) console_open: bool,
    pub(crate) console_input: String,
    pub(crate) hierarchy_drag_source: Option<EntityId>,

    // Selection + inspector cache.
    pub(crate) selected_entity_cached: Option<EntityId>,
    pub(crate) insp_pos: [f32; 3],
    pub(crate) insp_rot_deg: [f32; 3],
    pub(crate) insp_scale: [f32; 3],
    pub(crate) insp_color: [f32; 4],
    pub(crate) insp_material: MaterialId,

    pub(crate) gizmo: EguiGizmo,

    pub(crate) command_bus: EditorCommandBus,
    pub(crate) editor: EditorState,
    pub(crate) gizmo_was_dragging: bool,
    pub(crate) gizmo_drag_begin: Option<(EntityId, TransformSnapshot)>,

    // Viewport picking is processed on render thread, but selection semantics (replace/add/toggle)
    // are decided by UI at click time.
    pub(crate) pending_pick: Option<PendingPick>,
}

#[derive(Debug, Clone)]
pub(crate) struct PendingAssetSpawn {
    pub(crate) contract: AssetSpawnContract,
    pub(crate) source: &'static str,
}

#[derive(Debug, Clone)]
pub(crate) struct AssetManagerUiState {
    pub(crate) open: bool,

    pub(crate) path: String,
    pub(crate) last_id: Option<String>,
    pub(crate) last_state: String,
    pub(crate) last_meta_json: String,
    pub(crate) last_trace_json: String,
    pub(crate) sources_json: String,
    pub(crate) formats_json: String,
    pub(crate) last_error: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SceneIoMode {
    Load,
    Save,
}

#[derive(Debug, Clone)]
pub(crate) struct SceneIoUiState {
    pub(crate) open: bool,
    pub(crate) mode: SceneIoMode,
    pub(crate) path: String,
    pub(crate) pretty: bool,
    pub(crate) include_empty_entities: bool,

    pub(crate) formats_json: String,
    pub(crate) last_status: String,
    pub(crate) last_error: String,
}

impl Default for SceneIoUiState {
    #[inline]
    fn default() -> Self {
        Self {
            open: false,
            mode: SceneIoMode::Load,
            path: "scenes/example.scene.json".to_string(),
            pretty: true,
            include_empty_entities: false,

            formats_json: String::new(),
            last_status: String::new(),
            last_error: String::new(),
        }
    }
}

impl Default for AssetManagerUiState {
    #[inline]
    fn default() -> Self {
        Self {
            open: false,
            path: String::new(),
            last_id: None,
            last_state: String::new(),
            last_meta_json: String::new(),
            last_trace_json: String::new(),
            sources_json: String::new(),
            formats_json: String::new(),
            last_error: String::new(),
        }
    }
}

#[inline]
pub(crate) fn to_command_display_mode(mode: crate::gameplay::DisplayMode) -> CommandDisplayMode {
    match mode {
        crate::gameplay::DisplayMode::Both => CommandDisplayMode::Both,
        crate::gameplay::DisplayMode::EditorOnly => CommandDisplayMode::EditorOnly,
        crate::gameplay::DisplayMode::GameOnly => CommandDisplayMode::GameOnly,
    }
}

#[inline]
pub(crate) fn from_command_display_mode(mode: CommandDisplayMode) -> crate::gameplay::DisplayMode {
    match mode {
        CommandDisplayMode::Both => crate::gameplay::DisplayMode::Both,
        CommandDisplayMode::EditorOnly => crate::gameplay::DisplayMode::EditorOnly,
        CommandDisplayMode::GameOnly => crate::gameplay::DisplayMode::GameOnly,
    }
}

#[inline]
pub(crate) fn to_command_collision_body(body: crate::gameplay::CollisionBody) -> CommandCollisionBody {
    CommandCollisionBody {
        shape: match body.shape {
            crate::gameplay::CollisionShape::Box { half_extents } => CommandCollisionShape::Box { half_extents },
            crate::gameplay::CollisionShape::Sphere { radius } => CommandCollisionShape::Sphere { radius },
            crate::gameplay::CollisionShape::Capsule { radius, half_height } => CommandCollisionShape::Capsule { radius, half_height },
        },
        dynamic: body.dynamic,
        is_trigger: body.is_trigger,
    }
}

#[inline]
pub(crate) fn from_command_collision_body(body: CommandCollisionBody) -> crate::gameplay::CollisionBody {
    crate::gameplay::CollisionBody {
        shape: match body.shape {
            CommandCollisionShape::Box { half_extents } => crate::gameplay::CollisionShape::Box { half_extents },
            CommandCollisionShape::Sphere { radius } => crate::gameplay::CollisionShape::Sphere { radius },
            CommandCollisionShape::Capsule { radius, half_height } => crate::gameplay::CollisionShape::Capsule { radius, half_height },
        },
        dynamic: body.dynamic,
        is_trigger: body.is_trigger,
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PendingPick {
    pub(crate) additive: bool,
    pub(crate) toggle: bool,
}


#[inline]
pub(crate) fn snap_scalar(value: f32, step: f32) -> f32 {
    if !value.is_finite() || !step.is_finite() || step <= 0.0 {
        return value;
    }
    (value / step).round() * step
}

#[inline]
pub(crate) fn push_json_escaped(out: &mut String, s: &str) {
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
}
