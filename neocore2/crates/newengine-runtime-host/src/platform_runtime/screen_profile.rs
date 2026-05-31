#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_assets_api::{AssetDocument, AssetDocumentRequest, AssetServiceClient};
use newengine_schema_api::ENGINE_SCHEMA_SERVICE_ID;
use newengine_core::host_events::WindowInitSize;
use newengine_core::Resources;
use newengine_ui_api::{
    EditorSelectionContext, EditorSelectionKind, UiComponentNode, UiEditorRuntimeMode,
    UiEditorRuntimeState, UiEventDispatchFrame, UiInputCaptureState, UiInputCaptureStateManager,
    UiDockLayoutState, UiDockPanelRuntimeState, UiNodeEventTrigger, UiNodeRequestSourceKind,
    UiNodeTone, UiNodeTreeRequest, UiScreenInputFocusPolicy,
    UiScreenPanelDescriptor, UiScreenProfile, UiScreenProfileDescriptor, UiScreenProfileState,
    UiSurfaceAnchor, UiSurfaceNode, UiSurfaceStyle, UiToastNotification, UiToastSeverity,
    UiToastStack, UiViewportSlot, UI_COMPONENT_PANEL, UI_COMPONENT_ROW,
    UI_SURFACE_EDITOR_SHELL, UI_SURFACE_GAME_PRESENTATION, UI_SURFACE_SCREEN_ROOT,
    UI_THEME_NORTHSTAR_EDITOR, UI_THEME_ASSET_NORTHSTAR_EDITOR, UI_FONT_ASSET_EDITOR_SANS, UI_FONT_ASSET_EDITOR_DISPLAY,
};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

const SCREEN_PROFILE_CAPTURE_REASON: &str = "screen_profile.editor_shell";
const SCREEN_PROFILE_CAPTURE_OWNER: &str = "screen_profile.editor_shell";
const RIGHT_EDIT_WINDOW_OWNER: &str = "engine.ui.editor.right_edit_window";

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

// Keep screen_profile.rs as the ownership index only. Large editor-shell code lives
// in same-scope include parts so private constants/types stay local while the
// large-module gate can track owner-sized files.
include!("screen_profile_parts/types.rs");
include!("screen_profile_parts/state.rs");
include!("screen_profile_parts/helpers.rs");
include!("screen_profile_parts/profiles.rs");
include!("screen_profile_parts/components.rs");
include!("screen_profile_parts/panels_and_tests.rs");
