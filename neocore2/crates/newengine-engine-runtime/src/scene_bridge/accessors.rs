use super::*;
use newengine_bounds::Bounds;
use newengine_ui_api::{
    UiEditorInspectorSnapshot, UiEditorInspectorTransformSnapshot, UiEditorSceneEntitySnapshot,
    UiEditorSceneSnapshot, UiEventDispatchFrame, UiNodeEventTrigger, UiStatePatch,
};

const EDITOR_INSPECTOR_SURFACE_ID: &str = "engine.ui.editor.inspector";
const GAME_HUD_SURFACE_ID: &str = "game.hud";
const INSPECTOR_CONTRACT: &str = "newengine.scene.selected_entity_inspector.snapshot.v1";
const IN_GAME_EDITOR_CONTRACT: &str = "newengine.scene.ingame_editor.state.v1";
const IN_GAME_EDITOR_TOGGLE_ACTION: &str = "game.editor.toggle";
const IN_GAME_EDITOR_CLOSE_ACTION: &str = "game.editor.close";
const IN_GAME_EDITOR_TRANSFORM_PREFIX: &str = "game.editor.transform.";

#[path = "accessors/core.rs"]
mod core;
#[path = "accessors/editor.rs"]
mod editor;
#[path = "accessors/inspector.rs"]
mod inspector;

use inspector::*;

#[cfg(test)]
include!("accessors/tests.rs");
