#![forbid(unsafe_op_in_unsafe_fn)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::field_reassign_with_default)]

//! Asset Browser retained UI projection over engine.assets data.
//!
//! This crate is deliberately not a backend domain, gateway or capability. It is
//! a product/profile UI composition module: it reads reusable backend data from
//! `engine.assets` and publishes a generic `UiSurfaceNode` through `engine.ui`.
//! Rendering remains owned by the selected `engine.ui` provider.
use newengine_assets_api::{
    AssetDecodeRequest, AssetDocumentAction, AssetFileManifest, AssetPatchResult, AssetService,
    AssetServiceClient, ASSET_LIST_FILE_MANIFEST_OUTPUT,
};
use newengine_core::host_events::WindowInitSize;
use newengine_core::lifecycle_events::EngineReadinessKey;
use newengine_core::{EngineResult, Module, ModuleCtx, Resources};
use newengine_input_actions_api::{
    engine_action, InputActionDefinition, InputActionDispatchMode, InputActionEffect,
    InputActionFrame, InputFrameSource,
};
use newengine_input_api::{engine_default_keybind, key_code, key_identity};
use newengine_input_bindings_api::{InputBinding, InputBindingRegistration, InputKeyRegistration};
use newengine_plugin_api::HostApiV1;
use std::collections::BTreeSet;

use newengine_ui_api::{
    ui_surface_node_layout, EditorSelectionContext, UiActionDispatch, UiComponentNode,
    UiDockLayoutState, UiEventDispatchFrame, UiHitTestResult, UiInputCaptureState,
    UiInputCaptureStateManager, UiInputFrame, UiNodeEventTrigger, UiNodeMessage,
    UiNodeMessageSeverity, UiNodeTone, UiSurfaceAnchor, UiSurfaceNode, UiSurfaceStyle,
    ENGINE_UI_SERVICE_ID, UI_COMPONENT_GRID, UI_COMPONENT_INPUT, UI_COMPONENT_LIST,
    UI_COMPONENT_PANEL, UI_COMPONENT_TREE, UI_FONT_ASSET_EDITOR_SANS,
    UI_SERVICE_METHOD_SURFACE_NODE_V1, UI_THEME_ASSET_NORTHSTAR_EDITOR, UI_THEME_NORTHSTAR_EDITOR,
};
use serde_json::{json, Value};

mod entry_presentation;
mod path;
mod pipeline_status;
mod value_helpers;

use entry_presentation::*;
use path::*;
use pipeline_status::*;
use value_helpers::*;

pub const ASSETS_CATALOG_UI_OWNER: &str = "app.asset_browser";
const ASSETS_CATALOG_SURFACE_ID: &str = "ui.assets.catalog";
const ASSETS_CATALOG_INPUT_LISTENER: &str = "asset-browser-ui";
const ASSETS_CATALOG_THEME_ID: &str = UI_THEME_NORTHSTAR_EDITOR;
pub(crate) const ASSET_BROWSER_ICON_FOLDER: &str = "textures/ui/icons/assetBrowser.ytd@folder";
pub(crate) const ASSET_BROWSER_ICON_TEXTURE: &str = "textures/ui/icons/assetBrowser.ytd@texture";
pub(crate) const ASSET_BROWSER_ICON_GENERIC: &str = "textures/ui/icons/assetBrowser.ytd@generic";
pub(crate) const MAX_VISIBLE_ENTRIES: usize = 64;
const UI_SCROLLBAR_DRAG_ACTION: &str = "ui.scrollbar.drag";
const UI_SCROLL_WHEEL_ACTION: &str = "ui.scroll.wheel";
const DEFAULT_SURFACE_SIZE_PX: [u32; 2] = [1600, 900];

mod catalog_state;
pub(crate) use catalog_state::*;
mod runtime;
pub use runtime::AssetsCatalogUiRuntimeModule;
mod preview_bridge;
pub(crate) use preview_bridge::*;
mod catalog_actions;
pub(crate) use catalog_actions::*;
mod catalog_model;
pub(crate) use catalog_model::*;
mod asset_tree;
pub(crate) use asset_tree::*;
mod asset_grid;
pub(crate) use asset_grid::*;
mod diagnostics;
pub(crate) use diagnostics::*;
mod selection;
pub(crate) use selection::*;
#[path = "runtime_parts/action_dispatch.rs"]
mod runtime_action_dispatch;
#[path = "runtime_parts/lifecycle.rs"]
mod runtime_lifecycle;
#[path = "runtime_parts/preview_bridge_impl.rs"]
mod runtime_preview_bridge_impl;
#[path = "runtime_parts/selection_impl.rs"]
mod runtime_selection_impl;
