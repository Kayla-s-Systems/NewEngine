#[path = "../windows_stub_ui/editor_geometry.rs"]
mod editor_geometry;
#[path = "win32.rs"]
mod win32;

use self::editor_geometry::EditorGeometry;
use self::win32::*;
use super::super::dialog_model::ModalDialogModel;
use super::super::menu_model::{self, MenuCommand};
use super::super::toolbar_model;
use std::fs;
use std::mem::zeroed;
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};
use std::sync::{Mutex, OnceLock};

use super::super::EditorStartupModel;
use crate::tool_runtime::{
    discover_self_describing_tools, routes_from_providers, run_tool_preview, ToolMountStore,
    ToolRouteDescriptor,
};
use crate::ytd_preview::{parse_ytd_inspect_entries, YtdTextureEntry};
use northstar_gui_editor_ui::editors::text_editor::{
    SyntaxRegistry, TextDocument, TextEditorWidget, TextSelection, TokenKind, TokenSpan,
};

#[path = "windows_parts/state.rs"]
mod state;
use state::*;
#[path = "windows_parts/layout.rs"]
mod layout;
use layout::*;
#[path = "windows_parts/window_runtime_parts/main_window_proc.rs"]
mod main_window_proc;
#[path = "windows_parts/window_runtime_parts/window_loop.rs"]
mod window_loop;
use main_window_proc::*;
#[path = "windows_parts/window_runtime_parts/paint_dispatch.rs"]
mod paint_dispatch;
use paint_dispatch::*;
#[path = "windows_parts/window_runtime_parts/input_dispatch.rs"]
mod input_dispatch;
use input_dispatch::*;
#[path = "windows_parts/window_runtime_parts/shell_paint_parts/shell_chrome.rs"]
mod shell_chrome;
use shell_chrome::*;
#[path = "windows_parts/window_runtime_parts/shell_paint_parts/workspace_tree.rs"]
mod workspace_tree;
use workspace_tree::*;
#[path = "windows_parts/window_runtime_parts/shell_paint_parts/asset_details.rs"]
mod asset_details;
use asset_details::*;
#[path = "windows_parts/window_runtime_parts/shell_paint_parts/file_table.rs"]
mod file_table;
use file_table::*;
#[path = "windows_parts/window_runtime_parts/shell_paint_parts/inspector.rs"]
mod inspector_panel;
use inspector_panel::*;
#[path = "windows_parts/window_runtime_parts/shell_paint_parts/preview_editors.rs"]
mod preview_editors;
use preview_editors::*;
#[path = "windows_parts/text_editor_bridge_parts/rendering.rs"]
mod text_rendering;
use text_rendering::*;
#[path = "windows_parts/text_editor_bridge_parts/editing.rs"]
mod text_editing;
use text_editing::*;
#[path = "windows_parts/asset_browser_parts/browser_layout.rs"]
mod browser_layout;
use browser_layout::*;
#[path = "windows_parts/asset_browser_parts/browser_commands.rs"]
mod browser_commands;
use browser_commands::*;
#[path = "windows_parts/asset_routes.rs"]
mod asset_routes;
use asset_routes::*;
#[path = "windows_parts/modal_actions.rs"]
mod modal_actions;
use modal_actions::*;
#[path = "windows_parts/modal_window.rs"]
mod modal_window;
use modal_window::*;
#[path = "windows_parts/ytd_preview.rs"]
mod ytd_preview;
use ytd_preview::*;
#[path = "windows_parts/modal_editor.rs"]
mod modal_editor;
use modal_editor::*;
#[path = "windows_parts/geometry_paint.rs"]
mod geometry_paint;
use geometry_paint::*;

pub(crate) fn run(startup: &EditorStartupModel) -> Result<(), String> {
    window_loop::run(startup)
}
