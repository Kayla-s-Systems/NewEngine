#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_assets_api::{
    assets_ui_method, AssetDocument, AssetDocumentRequest, AssetServiceClient,
    ENGINE_ASSETS_UI_SERVICE_ID,
};
use newengine_core::host_events::WindowInitSize;
use newengine_core::Resources;
use newengine_editor_command_api::{
    default_runtime_editor_commands, editor_command, EditorCommandContext, EditorCommandRegistry,
};
use newengine_runtime_session_api::{
    RuntimeSessionCommand, RuntimeSessionMode, RuntimeSessionState,
    RUNTIME_SESSION_COMMAND_SOURCE_CONSOLE, RUNTIME_SESSION_COMMAND_SOURCE_EDITOR,
    RUNTIME_SESSION_COMMAND_SOURCE_GAME,
};
use newengine_runtime_session_runtime::{
    advance_runtime_session, drain_external_runtime_session_commands,
    install_runtime_session_resources, submit_runtime_session_command,
};
use newengine_schema_api::ENGINE_SCHEMA_SERVICE_ID;
use newengine_ui_api::{
    EditorSelectionContext, EditorSelectionKind, UiComponentNode, UiDockLayoutState,
    UiDockPanelRuntimeState, UiEditorInspectorSnapshot, UiEditorRuntimeMode, UiEditorRuntimeState,
    UiEditorSceneSnapshot, UiEditorTransformMode, UiEditorTransformSpace,
    UiEditorViewportProjection, UiEditorViewportShading, UiEditorViewportState,
    UiEventDispatchFrame, UiGameGuiConfig, UiGameLayerCommandKind, UiGameLayerCommandQueue,
    UiGameLayerKind, UiGameLayerStackState, UiInGameEditorState, UiInputCaptureState,
    UiInputCaptureStateManager, UiInputFrame, UiNodeEventTrigger, UiNodeRequestSourceKind,
    UiNodeTone, UiNodeTreeRequest, UiPresentationFlowState, UiScreenInputFocusPolicy,
    UiScreenPanelDescriptor, UiScreenProfile, UiScreenProfileDescriptor, UiScreenProfileState,
    UiSurfaceAnchor, UiSurfaceNode, UiSurfaceStyle, UiToastSeverity, UiToastStack, UiViewportSlot,
    ENGINE_UI_NOTIFY_SERVICE_ID, UI_COMPONENT_INPUT, UI_COMPONENT_PANEL, UI_COMPONENT_ROW,
    UI_FONT_ASSET_EDITOR_DISPLAY, UI_FONT_ASSET_EDITOR_SANS, UI_SURFACE_EDITOR_SHELL,
    UI_SURFACE_GAME_PRESENTATION, UI_SURFACE_SCREEN_ROOT, UI_SURFACE_SYSTEM_NOTIFICATIONS,
    UI_THEME_ASSET_NORTHSTAR_EDITOR, UI_THEME_NORTHSTAR_DEFAULT, UI_THEME_NORTHSTAR_EDITOR,
};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

const SCREEN_PROFILE_CAPTURE_REASON: &str = "screen_profile.editor_shell";
const SCREEN_PROFILE_CAPTURE_OWNER: &str = "screen_profile.editor_shell";
const RIGHT_EDIT_WINDOW_OWNER: &str = "engine.ui.editor.right_edit_window";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ScreenProfileRefresh {
    pub(crate) game_ui: bool,
    pub(crate) shell_ui: bool,
}

impl ScreenProfileRefresh {
    #[inline]
    pub(crate) const fn all() -> Self {
        Self {
            game_ui: true,
            shell_ui: true,
        }
    }

    #[inline]
    pub(crate) const fn any(self) -> bool {
        self.game_ui || self.shell_ui
    }
}

#[inline]
fn editing_tools_available(resources: &Resources) -> bool {
    resources
        .get::<newengine_plugin_host::PluginsSnapshot>()
        .is_some_and(|snapshot| {
            snapshot.has_running_capability(newengine_plugin_api::CAPABILITY_ID_EDITING_TOOLS)
        })
}

#[inline]
fn in_game_editor_active(resources: &Resources) -> bool {
    resources
        .get::<UiInGameEditorState>()
        .is_some_and(|state| state.enabled)
}

const SCREEN_PROFILE_SOURCE: &str = "engine.ui.screen_profile";
const EDITOR_LAYOUT_ID: &str = "engine.ui.screen.editor.v1";
const GAME_LAYOUT_ID: &str = "engine.ui.screen.game.v1";
const DEFAULT_VIEWPORT_SURFACE: &str = "engine.render.viewport.primary";
const DEFAULT_EDITOR_SURFACE_SIZE_PX: [u32; 2] = [1600, 900];
const EDITOR_SHELL_NEUI_REF: &str = "assets/ui/editor/editor_shell.neui@surface";
const SCENE_TREE_NEUI_REF: &str = "assets/ui/editor/scene_tree.neui@surface";
const INSPECTOR_NEUI_REF: &str = "assets/ui/editor/inspector.neui@surface";
const ASSET_BROWSER_NEUI_REF: &str = "assets/ui/editor/content_browser.neui@editor.asset_browser";
const IMPORT_QUEUE_NEUI_REF: &str = "assets/ui/editor/import_queue.neui@surface";
const OUTPUT_LOG_NEUI_REF: &str = "assets/ui/editor/output_log.neui@surface";
const PROFILER_DIAGNOSTICS_NEUI_REF: &str = "assets/ui/editor/profiler_diagnostics.neui@surface";
const VIEWPORT_GIZMOS_NEUI_REF: &str = "assets/ui/editor/viewport_gizmos.neui@surface";

#[path = "screen_profile_parts/components.rs"]
mod components;
#[path = "screen_profile_parts/helpers.rs"]
mod helpers;
#[path = "screen_profile_parts/panels_and_tests.rs"]
mod panels_and_tests;
#[path = "screen_profile_parts/profiles.rs"]
mod profiles;
#[path = "screen_profile_parts/state.rs"]
mod state;
#[path = "screen_profile_parts/types.rs"]
mod types;

use self::components::*;
use self::helpers::*;
use self::panels_and_tests::*;
use self::profiles::*;
use self::types::*;

pub(crate) use self::types::ScreenProfileRuntimeState;
