#![forbid(unsafe_op_in_unsafe_fn)]

use std::borrow::Cow;

use newengine_editor_core::ToolId;
use newengine_primitives::PrimitiveId;

use crate::EditorPlayMode;

use super::{dock::EditorDockTab, schema, CameraSpeedSettings, EditorUiBuild, SceneIoMode, ViewportMode, WorkspacePreset};

#[derive(Debug, Clone)]
pub(crate) enum UiAction {
    NewScene,
    OpenScene(SceneIoMode),
    OpenAssetManager,
    OpenCommandPalette,
    QuitStub,
    Undo,
    Redo,
    Deselect,
    ToggleConsole,
    TogglePlugins,
    FrameSelection,
    FrameAll,
    SetWorkspacePreset(WorkspacePreset),
    SetViewportMode(ViewportMode),
    SetTool(ToolId),
    SetPlayMode(EditorPlayMode),
    StopRuntime,
    OpenDockTab(EditorDockTab),
    CameraSpeedUp,
    CameraSpeedDown,
    SetCameraSpeedPreset(usize),
    SpawnPlayer,
    SpawnPrimitive {
        id: PrimitiveId,
        name: String,
    },
    SpawnDirectionalLight,
    SpawnPointLight,
    AddCollisionToSelection,
    RemoveCollisionFromSelection,
    SpawnPendingAsset,
    ToggleCollisionOverlay,
}


#[derive(Debug, Clone)]
pub(crate) struct UiActionDescriptor {
    pub(crate) label: Cow<'static, str>,
    pub(crate) keywords: Cow<'static, str>,
    pub(crate) action: UiAction,
    pub(crate) enabled: bool,
    pub(crate) selected: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct ChoiceDescriptor<T: Copy> {
    pub(crate) value: T,
    pub(crate) label: &'static str,
    pub(crate) enabled: bool,
    pub(crate) selected: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct UiMenuDescriptor {
    pub(crate) label: &'static str,
    pub(crate) entries: Vec<UiMenuEntry>,
}

#[derive(Debug, Clone)]
pub(crate) enum UiMenuEntry {
    Action(UiActionDescriptor),
    Separator,
    Info(Cow<'static, str>),
}

#[derive(Debug, Clone)]
pub(crate) struct CreateGroupDescriptor {
    pub(crate) label: &'static str,
    pub(crate) actions: Vec<UiActionDescriptor>,
}

#[inline]
fn action(
    label: impl Into<Cow<'static, str>>,
    keywords: impl Into<Cow<'static, str>>,
    action: UiAction,
    enabled: bool,
    selected: bool,
) -> UiActionDescriptor {
    UiActionDescriptor {
        label: label.into(),
        keywords: keywords.into(),
        action,
        enabled,
        selected,
    }
}

pub(crate) fn workspace_preset_choices(me: &EditorUiBuild) -> Vec<ChoiceDescriptor<WorkspacePreset>> {
    WorkspacePreset::ALL
        .iter()
        .copied()
        .map(|preset| ChoiceDescriptor {
            value: preset,
            label: preset.label(),
            enabled: true,
            selected: me.workspace_preset == preset,
        })
        .collect()
}

pub(crate) fn viewport_mode_choices(me: &EditorUiBuild) -> Vec<ChoiceDescriptor<ViewportMode>> {
    ViewportMode::ALL
        .iter()
        .copied()
        .map(|mode| ChoiceDescriptor {
            value: mode,
            label: mode.label(),
            enabled: true,
            selected: me.viewport_mode == mode,
        })
        .collect()
}

pub(crate) fn camera_speed_choices(me: &EditorUiBuild) -> Vec<ChoiceDescriptor<usize>> {
    CameraSpeedSettings::PRESET_LABELS
        .iter()
        .enumerate()
        .map(|(index, label)| ChoiceDescriptor {
            value: index,
            label: *label,
            enabled: true,
            selected: me.camera_speed.preset_index == index,
        })
        .collect()
}

#[inline]
fn dock_panel_actions(me: &EditorUiBuild) -> Vec<UiMenuEntry> {
    EditorDockTab::ALL
        .into_iter()
        .map(|tab| {
            UiMenuEntry::Action(action(
                format!("Open {}", tab.title()),
                "dock window panel layout editor shell",
                UiAction::OpenDockTab(tab),
                true,
                me.dock_state.find_tab(&tab).is_some(),
            ))
        })
        .collect()
}

pub(crate) fn create_menu_groups(me: &EditorUiBuild) -> Vec<CreateGroupDescriptor> {
    let mut primitive_actions = Vec::new();
    for (name, id) in me.scene_bridge.primitives_snapshot() {
        primitive_actions.push(action(
            Cow::Owned(name.clone()),
            "spawn primitive actor mesh",
            UiAction::SpawnPrimitive { id, name },
            true,
            false,
        ));
    }

    vec![
        CreateGroupDescriptor {
            label: "Actors",
            actions: vec![action(
                "Player",
                "spawn player actor pawn",
                UiAction::SpawnPlayer,
                true,
                false,
            )],
        },
        CreateGroupDescriptor {
            label: "Primitives",
            actions: primitive_actions,
        },
        CreateGroupDescriptor {
            label: "Lights",
            actions: vec![
                action(
                    "Directional Light",
                    "spawn directional light sun",
                    UiAction::SpawnDirectionalLight,
                    true,
                    false,
                ),
                action(
                    "Point Light",
                    "spawn point light",
                    UiAction::SpawnPointLight,
                    true,
                    false,
                ),
            ],
        },
    ]
}

pub(crate) fn file_toolbar_actions(me: &EditorUiBuild) -> Vec<UiActionDescriptor> {
    let has_scene_io = me.scene_io.is_some();
    vec![
        action("New", "new scene file", UiAction::NewScene, true, false),
        action(
            "Open",
            "open scene load",
            UiAction::OpenScene(SceneIoMode::Load),
            has_scene_io,
            false,
        ),
        action(
            "Save",
            "save scene",
            UiAction::OpenScene(SceneIoMode::Save),
            has_scene_io,
            false,
        ),
        action(
            "Actions",
            "command palette quick actions",
            UiAction::OpenCommandPalette,
            true,
            false,
        ),
    ]
}

pub(crate) fn tool_actions(me: &EditorUiBuild) -> Vec<UiActionDescriptor> {
    [
        ("Select", "tool select", ToolId::Select),
        ("Move", "tool move translate", ToolId::Translate),
        ("Rotate", "tool rotate", ToolId::Rotate),
        ("Scale", "tool scale", ToolId::Scale),
    ]
        .iter()
        .map(|(label, keywords, tool)| {
            action(
                *label,
                *keywords,
                UiAction::SetTool(*tool),
                true,
                me.editor.active_tool == *tool,
            )
        })
        .collect()
}

pub(crate) fn viewport_mode_actions(me: &EditorUiBuild) -> Vec<UiActionDescriptor> {
    viewport_mode_choices(me)
        .into_iter()
        .map(|choice| {
            action(
                choice.label,
                format!("viewport {} mode", choice.label.to_ascii_lowercase()),
                UiAction::SetViewportMode(choice.value),
                choice.enabled,
                choice.selected,
            )
        })
        .collect()
}

pub(crate) fn runtime_actions(me: &EditorUiBuild) -> Vec<UiActionDescriptor> {
    let play_mode = me.scene_bridge.play_mode();
    vec![
        action(
            "Play",
            "mode play runtime",
            UiAction::SetPlayMode(EditorPlayMode::Play),
            true,
            play_mode == EditorPlayMode::Play,
        ),
        action(
            "Simulate",
            "mode simulate runtime",
            UiAction::SetPlayMode(EditorPlayMode::Simulate),
            true,
            play_mode == EditorPlayMode::Simulate,
        ),
        action(
            "Stop",
            "stop runtime edit mode",
            UiAction::StopRuntime,
            play_mode != EditorPlayMode::Edit,
            false,
        ),
    ]
}

pub(crate) fn viewport_overlay_quick_actions(_me: &EditorUiBuild) -> Vec<UiActionDescriptor> {
    vec![
        action(
            "Frame",
            "frame selection focus",
            UiAction::FrameSelection,
            true,
            false,
        ),
        action("Frame All", "frame all focus", UiAction::FrameAll, true, false),
        action(
            "Cam -",
            "camera speed slower",
            UiAction::CameraSpeedDown,
            true,
            false,
        ),
        action(
            "Cam +",
            "camera speed faster",
            UiAction::CameraSpeedUp,
            true,
            false,
        ),
    ]
}

pub(crate) fn command_palette_actions(me: &EditorUiBuild) -> Vec<UiActionDescriptor> {
    let mut actions = Vec::new();

    for preset in workspace_preset_choices(me) {
        actions.push(action(
            format!("Workspace: {}", preset.label),
            format!("workspace {} layout", preset.label.to_ascii_lowercase()),
            UiAction::SetWorkspacePreset(preset.value),
            preset.enabled,
            preset.selected,
        ));
    }

    actions.extend(viewport_mode_actions(me).into_iter().map(|mut desc| {
        let label = desc.label.clone();
        desc.label = Cow::Owned(format!("Viewport: {label}"));
        desc
    }));

    actions.extend(tool_actions(me).into_iter().map(|mut desc| {
        let label = desc.label.clone();
        desc.label = Cow::Owned(format!("Tool: {label}"));
        desc
    }));

    actions.extend(runtime_actions(me).into_iter().map(|mut desc| {
        let label = desc.label.clone();
        desc.label = Cow::Owned(format!("Mode: {label}"));
        desc
    }));

    actions.extend(viewport_overlay_quick_actions(me));

    let surface = me.surface_context();
    for it in schema::selection_context_actions(me, surface.primary_selection.as_ref()) {
        let mapped = match it.id {
            schema::ContextActionId::FrameSelection => UiAction::FrameSelection,
            schema::ContextActionId::Deselect => UiAction::Deselect,
            schema::ContextActionId::ToggleCollisionOverlay => UiAction::ToggleCollisionOverlay,
            schema::ContextActionId::EnterPlay => UiAction::SetPlayMode(crate::gameplay::EditorPlayMode::Play),
            schema::ContextActionId::EnterSimulate => UiAction::SetPlayMode(crate::gameplay::EditorPlayMode::Simulate),
            schema::ContextActionId::StopRuntime => UiAction::StopRuntime,
            schema::ContextActionId::SelectTool => UiAction::SetTool(ToolId::Select),
            schema::ContextActionId::MoveTool => UiAction::SetTool(ToolId::Translate),
            schema::ContextActionId::RotateTool => UiAction::SetTool(ToolId::Rotate),
            schema::ContextActionId::ScaleTool => UiAction::SetTool(ToolId::Scale),
            schema::ContextActionId::AddCollision => UiAction::AddCollisionToSelection,
            schema::ContextActionId::RemoveCollision => UiAction::RemoveCollisionFromSelection,
            schema::ContextActionId::SpawnAssetHere => UiAction::SpawnPendingAsset,
        };
        actions.push(action(it.label, it.keywords, mapped, it.enabled, it.selected));
    }

    actions.push(action(
        "Camera Speed Up",
        "camera speed faster",
        UiAction::CameraSpeedUp,
        true,
        false,
    ));
    actions.push(action(
        "Camera Speed Down",
        "camera speed slower",
        UiAction::CameraSpeedDown,
        true,
        false,
    ));

    actions
}

pub(crate) fn menubar_descriptors(me: &EditorUiBuild) -> Vec<UiMenuDescriptor> {
    let mut file_entries = vec![UiMenuEntry::Action(action(
        "New Scene\tCtrl+N",
        "new scene file",
        UiAction::NewScene,
        true,
        false,
    ))];
    let has_scene_io = me.scene_io.is_some();
    file_entries.push(UiMenuEntry::Action(action(
        "Load Scene...\tCtrl+O",
        "load scene open",
        UiAction::OpenScene(SceneIoMode::Load),
        has_scene_io,
        false,
    )));
    file_entries.push(UiMenuEntry::Action(action(
        "Save Scene\tCtrl+S",
        "save scene",
        UiAction::OpenScene(SceneIoMode::Save),
        has_scene_io,
        false,
    )));
    if !has_scene_io {
        file_entries.push(UiMenuEntry::Info("Scene IO service not found".into()));
    }
    file_entries.push(UiMenuEntry::Separator);
    file_entries.push(UiMenuEntry::Action(action(
        "Quit",
        "quit shutdown",
        UiAction::QuitStub,
        true,
        false,
    )));

    let edit_entries = vec![
        UiMenuEntry::Action(action(
            "Undo\tCtrl+Z",
            "undo",
            UiAction::Undo,
            me.editor.commands.can_undo(),
            false,
        )),
        UiMenuEntry::Action(action(
            "Redo\tCtrl+Y",
            "redo",
            UiAction::Redo,
            me.editor.commands.can_redo(),
            false,
        )),
        UiMenuEntry::Separator,
        UiMenuEntry::Action(action(
            "Deselect\tEsc",
            "selection clear deselect",
            UiAction::Deselect,
            !me.editor.selection.is_empty(),
            false,
        )),
    ];

    let mut asset_entries = vec![UiMenuEntry::Action(action(
        "Asset Manager",
        "assets asset manager",
        UiAction::OpenAssetManager,
        me.assets.is_some(),
        false,
    ))];
    if me.assets.is_none() {
        asset_entries.push(UiMenuEntry::Info("AssetManager service not found".into()));
    }

    let view_entries = vec![
        UiMenuEntry::Action(action(
            "Console\tF1",
            "toggle console output log",
            UiAction::ToggleConsole,
            true,
            me.console_open,
        )),
        UiMenuEntry::Action(action(
            "Plugins\tF2",
            "toggle plugin manager",
            UiAction::TogglePlugins,
            true,
            false,
        )),
        UiMenuEntry::Action(action(
            "Actions\tCtrl+P",
            "command palette quick actions",
            UiAction::OpenCommandPalette,
            true,
            false,
        )),
    ];

    let mut window_entries: Vec<UiMenuEntry> = dock_panel_actions(me);
    window_entries.push(UiMenuEntry::Separator);
    window_entries.push(UiMenuEntry::Action(action(
        "Asset Manager",
        "assets asset manager",
        UiAction::OpenAssetManager,
        me.assets.is_some(),
        false,
    )));

    let tools_entries = vec![
        UiMenuEntry::Action(action(
            "Frame Selection\tF",
            "frame selection focus",
            UiAction::FrameSelection,
            true,
            false,
        )),
        UiMenuEntry::Action(action(
            "Frame All\tShift+F",
            "frame all focus",
            UiAction::FrameAll,
            true,
            false,
        )),
    ];

    let help_entries = vec![
        UiMenuEntry::Info("NewEngine Editor".into()),
        UiMenuEntry::Info("UI shell: centralized provider-driven shell".into()),
    ];

    vec![
        UiMenuDescriptor {
            label: "File",
            entries: file_entries,
        },
        UiMenuDescriptor {
            label: "Edit",
            entries: edit_entries,
        },
        UiMenuDescriptor {
            label: "Asset",
            entries: asset_entries,
        },
        UiMenuDescriptor {
            label: "View",
            entries: view_entries,
        },
        UiMenuDescriptor {
            label: "Window",
            entries: window_entries,
        },
        UiMenuDescriptor {
            label: "Tools",
            entries: tools_entries,
        },
        UiMenuDescriptor {
            label: "Help",
            entries: help_entries,
        },
    ]
}
