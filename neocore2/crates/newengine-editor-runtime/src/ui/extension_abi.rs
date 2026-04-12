#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;

use parking_lot::RwLock;

use newengine_plugin_api as plugin_api;
use plugin_api::editor::{
    EditorAssetAssemblerDescriptorV1, EditorAssetImportClassV1, EditorAssetImportDescriptorV1,
    EditorAssetImportProviderV1Dyn, EditorCommandDescriptorV1, EditorCommandHandlerResultV1,
    EditorCommandHandlerV1Dyn, EditorCommandInvocationV1, EditorComponentSchemaIdV1,
    EditorContextActionIdV1, EditorContextActionProviderV1Dyn, EditorContextActionSchemaV1,
    EditorDisplayModeV1, EditorExtensionsV1, EditorFieldEditorKindV1,
    EditorFieldFactoryProviderV1Dyn, EditorImportedAssetAssemblyDescriptorV1,
    EditorImportedAssetAssemblyKindV1, EditorImportedAssetKindV1, EditorImportedAssetRepresentationV1,
    EditorPlayModeV1, EditorPropertyFieldIdV1, EditorPropertyFieldSchemaV1,
    EditorPropertySectionIdV1, EditorSelectionContextV1, EditorSelectionKindV1, EditorSurfaceContextV1,
    EditorToolIdV1, EditorViewportModeV1, EditorWorkspacePresetV1,
};

use super::commands::TypedEditorCommand;
use super::providers;
use super::schema::{
    self, AssetImportClass, AssetImportDescriptor, AssetSpawnContract, ComponentSchemaId,
    ContextActionId, ContextActionSchema, FieldEditorKind, PropertyFieldId, PropertyFieldSchema,
    PropertySectionId, SelectionContext, SelectionKind,
};
use super::{EditorUiBuild, ViewportMode, WorkspacePreset};
use crate::gameplay::{DisplayMode, EditorPlayMode};
use crate::scene_bridge::{
    SceneImportedAssetAssembler, SceneImportedAssetAssemblyDescriptor,
    SceneImportedAssetAssemblyKind, SceneImportedAssetKind,
    SceneImportedAssetRepresentation,
};

pub struct EditorExtensionAbiRegistry {
    pub field_factories: Vec<(String, EditorFieldFactoryProviderV1Dyn<'static>)>,
    pub context_action_providers: Vec<(String, EditorContextActionProviderV1Dyn<'static>)>,
    pub asset_import_providers: Vec<(String, EditorAssetImportProviderV1Dyn<'static>)>,
    pub command_handlers: Vec<(String, EditorCommandHandlerV1Dyn<'static>)>,
}

impl Default for EditorExtensionAbiRegistry {
    #[inline]
    fn default() -> Self {
        Self {
            field_factories: Vec::new(),
            context_action_providers: Vec::new(),
            asset_import_providers: Vec::new(),
            command_handlers: Vec::new(),
        }
    }
}

#[inline]
pub fn register_editor_extensions(
    registry: &Arc<RwLock<EditorExtensionAbiRegistry>>,
    scene_bridge: &crate::scene_bridge::SceneBridge,
    plugin_id: &str,
    extensions: EditorExtensionsV1,
) -> usize {
    let mut installed = 0usize;
    let mut reg = registry.write();

    for factory in extensions.field_factories.into_iter() {
        reg.field_factories.push((plugin_id.to_string(), factory));
        installed += 1;
    }
    for provider in extensions.context_action_providers.into_iter() {
        reg.context_action_providers.push((plugin_id.to_string(), provider));
        installed += 1;
    }
    for provider in extensions.asset_import_providers.into_iter() {
        reg.asset_import_providers.push((plugin_id.to_string(), provider));
        installed += 1;
    }
    for handler in extensions.command_handlers.into_iter() {
        reg.command_handlers.push((plugin_id.to_string(), handler));
        installed += 1;
    }
    drop(reg);

    for assembler in extensions.asset_assemblers.into_iter() {
        scene_bridge.register_imported_asset_assembler(from_abi_asset_assembler(assembler));
        installed += 1;
    }

    installed
}

#[inline]
pub fn to_abi_surface_context(me: &EditorUiBuild) -> EditorSurfaceContextV1 {
    let ctx = schema::build_surface_context(me);
    EditorSurfaceContextV1 {
        play_mode: to_abi_play_mode(ctx.play_mode),
        runtime_active: ctx.runtime_active,
        viewport_mode: to_abi_viewport_mode(ctx.viewport_mode),
        active_tool: to_abi_tool_id(ctx.active_tool),
        camera_speed_label: ctx.camera_speed_label.into(),
        collision_overlay: ctx.collision_overlay,
        selection_count: ctx.selection_count.min(u32::MAX as usize) as u32,
        entity_count: ctx.entity_count.min(u32::MAX as usize) as u32,
        primary_selection: match ctx.primary_selection.as_ref() {
            Some(selection) => abi_stable::std_types::ROption::RSome(to_abi_selection_context(selection)),
            None => abi_stable::std_types::ROption::RNone,
        },
    }
}

#[inline]
pub fn to_abi_selection_context(ctx: &SelectionContext) -> EditorSelectionContextV1 {
    EditorSelectionContextV1 {
        entity_stable_id: ctx.entity.stable_u64(),
        name: ctx.name.clone().into(),
        kind: to_abi_selection_kind(ctx.kind),
        has_collision: ctx.has_collision,
        display_mode: to_abi_display_mode(ctx.display_mode),
        components: ctx.components.iter().copied().map(to_abi_component_id).collect::<abi_stable::std_types::RVec<_>>(),
    }
}

#[inline]
pub fn from_abi_property_field_schema(field: EditorPropertyFieldSchemaV1) -> Option<PropertyFieldSchema> {
    Some(PropertyFieldSchema {
        id: from_abi_property_field_id(field.id)?,
        label: Box::leak(field.label.to_string().into_boxed_str()),
        keywords: Box::leak(field.keywords.to_string().into_boxed_str()),
        editor: from_abi_field_editor_kind(field.editor)?,
        visible: field.visible,
        enabled: field.enabled,
    })
}

#[inline]
pub fn from_abi_context_action_schema(action: EditorContextActionSchemaV1) -> Option<ContextActionSchema> {
    Some(ContextActionSchema {
        id: from_abi_context_action_id(action.id)?,
        label: Box::leak(action.label.to_string().into_boxed_str()),
        keywords: Box::leak(action.keywords.to_string().into_boxed_str()),
        enabled: action.enabled,
        selected: action.selected,
    })
}

#[inline]
pub fn from_abi_asset_import_descriptor(import: EditorAssetImportDescriptorV1) -> Option<AssetImportDescriptor> {
    Some(AssetImportDescriptor {
        class: from_abi_asset_import_class(import.class),
        representation: from_abi_imported_asset_representation(import.representation),
        import_kind: from_abi_imported_asset_kind(import.import_kind),
        assembler_key: Box::leak(import.assembler_key.to_string().into_boxed_str()),
        assembly: from_abi_imported_asset_assembly_descriptor(import.assembly)?,
        default_scale: import.default_scale,
        tint: import.tint,
    })
}

#[inline]
pub fn from_abi_asset_assembler(desc: EditorAssetAssemblerDescriptorV1) -> SceneImportedAssetAssembler {
    SceneImportedAssetAssembler {
        key: desc.key.to_string(),
        label: Box::leak(desc.label.to_string().into_boxed_str()),
        import_kind: from_abi_imported_asset_kind(desc.import_kind),
        assembly: from_abi_imported_asset_assembly_kind(desc.assembly),
    }
}

#[inline]
pub fn to_abi_command_invocation(me: &EditorUiBuild, command: &TypedEditorCommand, source: &str) -> EditorCommandInvocationV1 {
    EditorCommandInvocationV1 {
        source: source.to_string().into(),
        command: to_abi_command_descriptor(command),
        surface: to_abi_surface_context(me),
    }
}

#[inline]
pub fn apply_command_handler_result(me: &mut EditorUiBuild, result: EditorCommandHandlerResultV1) -> bool {
    for emitted in result.emitted.into_iter() {
        if let Some(cmd) = from_abi_command_descriptor(emitted) {
            me.command_bus.push(cmd);
        }
    }
    result.handled
}

#[inline]
pub fn to_abi_command_descriptor(command: &TypedEditorCommand) -> EditorCommandDescriptorV1 {
    match command {
        TypedEditorCommand::UiAction(action) => EditorCommandDescriptorV1::NamedAction(ui_action_name(action).into()),
        TypedEditorCommand::ContextAction(action) => EditorCommandDescriptorV1::ContextAction(to_abi_context_action_id(*action)),
        TypedEditorCommand::SpawnAsset { contract, .. } => EditorCommandDescriptorV1::SpawnAsset(to_abi_asset_spawn_contract(contract)),
        TypedEditorCommand::SetTool(tool) => EditorCommandDescriptorV1::SetTool(to_abi_tool_id(*tool)),
        TypedEditorCommand::SetPlayMode(mode) => EditorCommandDescriptorV1::SetPlayMode(to_abi_play_mode(*mode)),
        TypedEditorCommand::SetWorkspacePreset(preset) => EditorCommandDescriptorV1::SetWorkspacePreset(to_abi_workspace_preset(*preset)),
        TypedEditorCommand::SetViewportMode(mode) => EditorCommandDescriptorV1::SetViewportMode(to_abi_viewport_mode(*mode)),
        TypedEditorCommand::PublishFrameSelection => EditorCommandDescriptorV1::PublishFrameSelection,
        TypedEditorCommand::PublishFrameAll => EditorCommandDescriptorV1::PublishFrameAll,
        TypedEditorCommand::ToggleCollisionOverlay => EditorCommandDescriptorV1::ToggleCollisionOverlay,
    }
}

#[inline]
pub fn from_abi_command_descriptor(command: EditorCommandDescriptorV1) -> Option<TypedEditorCommand> {
    Some(match command {
        EditorCommandDescriptorV1::NamedAction(name) => TypedEditorCommand::UiAction(named_ui_action(&name.to_string())?),
        EditorCommandDescriptorV1::ContextAction(action) => TypedEditorCommand::ContextAction(from_abi_context_action_id(action)?),
        EditorCommandDescriptorV1::SpawnAsset(contract) => TypedEditorCommand::SpawnAsset {
            contract: from_abi_asset_spawn_contract(contract)?,
            source: "abi.command_handler",
        },
        EditorCommandDescriptorV1::SetTool(tool) => TypedEditorCommand::SetTool(from_abi_tool_id(tool)),
        EditorCommandDescriptorV1::SetPlayMode(mode) => TypedEditorCommand::SetPlayMode(from_abi_play_mode(mode)),
        EditorCommandDescriptorV1::SetWorkspacePreset(preset) => TypedEditorCommand::SetWorkspacePreset(from_abi_workspace_preset(preset)),
        EditorCommandDescriptorV1::SetViewportMode(mode) => TypedEditorCommand::SetViewportMode(from_abi_viewport_mode(mode)),
        EditorCommandDescriptorV1::PublishFrameSelection => TypedEditorCommand::PublishFrameSelection,
        EditorCommandDescriptorV1::PublishFrameAll => TypedEditorCommand::PublishFrameAll,
        EditorCommandDescriptorV1::ToggleCollisionOverlay => TypedEditorCommand::ToggleCollisionOverlay,
    })
}

#[inline]
pub fn to_abi_asset_spawn_contract(contract: &AssetSpawnContract) -> plugin_api::editor::EditorAssetSpawnContractV1 {
    plugin_api::editor::EditorAssetSpawnContractV1 {
        logical_path: contract.logical_path.clone().into(),
        actor_name: contract.actor_name.clone().into(),
        import: to_abi_asset_import_descriptor(&contract.import),
    }
}

#[inline]
pub fn from_abi_asset_spawn_contract(contract: plugin_api::editor::EditorAssetSpawnContractV1) -> Option<AssetSpawnContract> {
    Some(AssetSpawnContract {
        logical_path: contract.logical_path.to_string(),
        actor_name: contract.actor_name.to_string(),
        import: from_abi_asset_import_descriptor(contract.import)?,
    })
}

#[inline]
pub fn to_abi_asset_import_descriptor(import: &AssetImportDescriptor) -> EditorAssetImportDescriptorV1 {
    EditorAssetImportDescriptorV1 {
        class: to_abi_asset_import_class(import.class),
        representation: to_abi_imported_asset_representation(import.representation),
        import_kind: to_abi_imported_asset_kind(import.import_kind),
        assembler_key: import.assembler_key.to_string().into(),
        assembly: to_abi_imported_asset_assembly_descriptor(&import.assembly),
        default_scale: import.default_scale,
        tint: import.tint,
    }
}

#[inline]
fn to_abi_imported_asset_assembly_descriptor(desc: &SceneImportedAssetAssemblyDescriptor) -> EditorImportedAssetAssemblyDescriptorV1 {
    let primitive_key = format!("{}", desc.primitive_id.0);
    EditorImportedAssetAssemblyDescriptorV1 {
        assembly: to_abi_imported_asset_assembly_kind(desc.assembly),
        primitive_key: primitive_key.into(),
        display_mode: to_abi_display_mode(desc.display_mode),
        with_collision: desc.with_collision,
        dynamic_collision: desc.dynamic_collision,
    }
}

#[inline]
fn from_abi_imported_asset_assembly_descriptor(desc: EditorImportedAssetAssemblyDescriptorV1) -> Option<SceneImportedAssetAssemblyDescriptor> {
    Some(SceneImportedAssetAssemblyDescriptor {
        assembly: from_abi_imported_asset_assembly_kind(desc.assembly),
        primitive_id: parse_primitive_id(&desc.primitive_key.to_string())?,
        display_mode: from_abi_display_mode(desc.display_mode),
        with_collision: desc.with_collision,
        dynamic_collision: desc.dynamic_collision,
    })
}

#[inline]
fn parse_primitive_id(raw: &str) -> Option<newengine_primitives::PrimitiveId> {
    raw.parse::<u64>()
        .ok()
        .map(newengine_primitives::PrimitiveId)
}

#[inline]
fn ui_action_name(action: &providers::UiAction) -> &'static str {
    match action {
        providers::UiAction::NewScene => "new_scene",
        providers::UiAction::OpenScene(_) => "open_scene",
        providers::UiAction::OpenAssetManager => "open_asset_manager",
        providers::UiAction::OpenCommandPalette => "open_command_palette",
        providers::UiAction::QuitStub => "quit_stub",
        providers::UiAction::Undo => "undo",
        providers::UiAction::Redo => "redo",
        providers::UiAction::Deselect => "deselect",
        providers::UiAction::ToggleConsole => "toggle_console",
        providers::UiAction::TogglePlugins => "toggle_plugins",
        providers::UiAction::FrameSelection => "frame_selection",
        providers::UiAction::FrameAll => "frame_all",
        providers::UiAction::SetWorkspacePreset(_) => "set_workspace_preset",
        providers::UiAction::SetViewportMode(_) => "set_viewport_mode",
        providers::UiAction::SetTool(_) => "set_tool",
        providers::UiAction::SetPlayMode(_) => "set_play_mode",
        providers::UiAction::StopRuntime => "stop_runtime",
        providers::UiAction::OpenDockTab(tab) => match tab {
            crate::ui::dock::EditorDockTab::Hierarchy => "open_dock_tab_hierarchy",
            crate::ui::dock::EditorDockTab::Viewport => "open_dock_tab_viewport",
            crate::ui::dock::EditorDockTab::Inspector => "open_dock_tab_inspector",
            crate::ui::dock::EditorDockTab::AssetBrowser => "open_dock_tab_asset_browser",
            crate::ui::dock::EditorDockTab::Console => "open_dock_tab_console",
            crate::ui::dock::EditorDockTab::Profiler => "open_dock_tab_profiler",
        },
        providers::UiAction::CameraSpeedUp => "camera_speed_up",
        providers::UiAction::CameraSpeedDown => "camera_speed_down",
        providers::UiAction::SetCameraSpeedPreset(_) => "set_camera_speed_preset",
        providers::UiAction::SpawnPlayer => "spawn_player",
        providers::UiAction::SpawnPrimitive { .. } => "spawn_primitive",
        providers::UiAction::SpawnDirectionalLight => "spawn_directional_light",
        providers::UiAction::SpawnPointLight => "spawn_point_light",
        providers::UiAction::TogglePanel(_) => "toggle_panel",
        providers::UiAction::AddCollisionToSelection => "add_collision_to_selection",
        providers::UiAction::RemoveCollisionFromSelection => "remove_collision_from_selection",
        providers::UiAction::SpawnPendingAsset => "spawn_pending_asset",
        providers::UiAction::ToggleCollisionOverlay => "toggle_collision_overlay",
    }
}

#[inline]
fn named_ui_action(name: &str) -> Option<providers::UiAction> {
    Some(match name {
        "new_scene" => providers::UiAction::NewScene,
        "open_asset_manager" => providers::UiAction::OpenAssetManager,
        "open_command_palette" => providers::UiAction::OpenCommandPalette,
        "quit_stub" => providers::UiAction::QuitStub,
        "undo" => providers::UiAction::Undo,
        "redo" => providers::UiAction::Redo,
        "deselect" => providers::UiAction::Deselect,
        "toggle_console" => providers::UiAction::ToggleConsole,
        "toggle_plugins" => providers::UiAction::TogglePlugins,
        "frame_selection" => providers::UiAction::FrameSelection,
        "frame_all" => providers::UiAction::FrameAll,
        "stop_runtime" => providers::UiAction::StopRuntime,
        "open_dock_tab_hierarchy" => providers::UiAction::OpenDockTab(crate::ui::dock::EditorDockTab::Hierarchy),
        "open_dock_tab_viewport" => providers::UiAction::OpenDockTab(crate::ui::dock::EditorDockTab::Viewport),
        "open_dock_tab_inspector" => providers::UiAction::OpenDockTab(crate::ui::dock::EditorDockTab::Inspector),
        "open_dock_tab_asset_browser" => providers::UiAction::OpenDockTab(crate::ui::dock::EditorDockTab::AssetBrowser),
        "open_dock_tab_console" => providers::UiAction::OpenDockTab(crate::ui::dock::EditorDockTab::Console),
        "open_dock_tab_profiler" => providers::UiAction::OpenDockTab(crate::ui::dock::EditorDockTab::Profiler),
        "camera_speed_up" => providers::UiAction::CameraSpeedUp,
        "camera_speed_down" => providers::UiAction::CameraSpeedDown,
        "spawn_player" => providers::UiAction::SpawnPlayer,
        "spawn_directional_light" => providers::UiAction::SpawnDirectionalLight,
        "spawn_point_light" => providers::UiAction::SpawnPointLight,
        "add_collision_to_selection" => providers::UiAction::AddCollisionToSelection,
        "remove_collision_from_selection" => providers::UiAction::RemoveCollisionFromSelection,
        "spawn_pending_asset" => providers::UiAction::SpawnPendingAsset,
        "toggle_collision_overlay" => providers::UiAction::ToggleCollisionOverlay,
        _ => return None,
    })
}

#[inline]
pub fn to_abi_component_id(id: ComponentSchemaId) -> EditorComponentSchemaIdV1 {
    match id {
        ComponentSchemaId::Identity => EditorComponentSchemaIdV1::Identity,
        ComponentSchemaId::Transform => EditorComponentSchemaIdV1::Transform,
        ComponentSchemaId::Display => EditorComponentSchemaIdV1::Display,
        ComponentSchemaId::Gameplay => EditorComponentSchemaIdV1::Gameplay,
        ComponentSchemaId::Collision => EditorComponentSchemaIdV1::Collision,
        ComponentSchemaId::Primitive => EditorComponentSchemaIdV1::Primitive,
        ComponentSchemaId::DirectionalLight => EditorComponentSchemaIdV1::DirectionalLight,
        ComponentSchemaId::PointLight => EditorComponentSchemaIdV1::PointLight,
        ComponentSchemaId::Material => EditorComponentSchemaIdV1::Material,
        ComponentSchemaId::ImportedAsset => EditorComponentSchemaIdV1::ImportedAsset,
    }
}

#[inline]
pub fn from_abi_component_id(id: EditorComponentSchemaIdV1) -> ComponentSchemaId {
    match id {
        EditorComponentSchemaIdV1::Identity => ComponentSchemaId::Identity,
        EditorComponentSchemaIdV1::Transform => ComponentSchemaId::Transform,
        EditorComponentSchemaIdV1::Display => ComponentSchemaId::Display,
        EditorComponentSchemaIdV1::Gameplay => ComponentSchemaId::Gameplay,
        EditorComponentSchemaIdV1::Collision => ComponentSchemaId::Collision,
        EditorComponentSchemaIdV1::Primitive => ComponentSchemaId::Primitive,
        EditorComponentSchemaIdV1::DirectionalLight => ComponentSchemaId::DirectionalLight,
        EditorComponentSchemaIdV1::PointLight => ComponentSchemaId::PointLight,
        EditorComponentSchemaIdV1::Material => ComponentSchemaId::Material,
        EditorComponentSchemaIdV1::ImportedAsset => ComponentSchemaId::ImportedAsset,
    }
}

#[inline]
pub fn to_abi_section_id(id: PropertySectionId) -> EditorPropertySectionIdV1 {
    match id {
        PropertySectionId::Summary => EditorPropertySectionIdV1::Summary,
        PropertySectionId::Transform => EditorPropertySectionIdV1::Transform,
        PropertySectionId::Display => EditorPropertySectionIdV1::Display,
        PropertySectionId::Gameplay => EditorPropertySectionIdV1::Gameplay,
        PropertySectionId::Collision => EditorPropertySectionIdV1::Collision,
        PropertySectionId::Primitive => EditorPropertySectionIdV1::Primitive,
        PropertySectionId::Lighting => EditorPropertySectionIdV1::Lighting,
        PropertySectionId::Material => EditorPropertySectionIdV1::Material,
    }
}

#[inline]
pub fn from_abi_section_id(id: EditorPropertySectionIdV1) -> PropertySectionId {
    match id {
        EditorPropertySectionIdV1::Summary => PropertySectionId::Summary,
        EditorPropertySectionIdV1::Transform => PropertySectionId::Transform,
        EditorPropertySectionIdV1::Display => PropertySectionId::Display,
        EditorPropertySectionIdV1::Gameplay => PropertySectionId::Gameplay,
        EditorPropertySectionIdV1::Collision => PropertySectionId::Collision,
        EditorPropertySectionIdV1::Primitive => PropertySectionId::Primitive,
        EditorPropertySectionIdV1::Lighting => PropertySectionId::Lighting,
        EditorPropertySectionIdV1::Material => PropertySectionId::Material,
    }
}

#[inline]
pub fn to_abi_property_field_id(id: PropertyFieldId) -> EditorPropertyFieldIdV1 {
    match id {
        PropertyFieldId::Name => EditorPropertyFieldIdV1::Name,
        PropertyFieldId::Kind => EditorPropertyFieldIdV1::Kind,
        PropertyFieldId::Entity => EditorPropertyFieldIdV1::Entity,
        PropertyFieldId::DisplayMode => EditorPropertyFieldIdV1::DisplayMode,
        PropertyFieldId::Position => EditorPropertyFieldIdV1::Position,
        PropertyFieldId::RotationDeg => EditorPropertyFieldIdV1::RotationDeg,
        PropertyFieldId::Scale => EditorPropertyFieldIdV1::Scale,
        PropertyFieldId::SnapTranslate => EditorPropertyFieldIdV1::SnapTranslate,
        PropertyFieldId::SnapRotate => EditorPropertyFieldIdV1::SnapRotate,
        PropertyFieldId::SnapScale => EditorPropertyFieldIdV1::SnapScale,
        PropertyFieldId::GameplayRole => EditorPropertyFieldIdV1::GameplayRole,
        PropertyFieldId::CollisionEnabled => EditorPropertyFieldIdV1::CollisionEnabled,
        PropertyFieldId::CollisionDynamic => EditorPropertyFieldIdV1::CollisionDynamic,
        PropertyFieldId::CollisionTrigger => EditorPropertyFieldIdV1::CollisionTrigger,
        PropertyFieldId::CollisionShape => EditorPropertyFieldIdV1::CollisionShape,
        PropertyFieldId::CollisionBoxExtents => EditorPropertyFieldIdV1::CollisionBoxExtents,
        PropertyFieldId::CollisionSphereRadius => EditorPropertyFieldIdV1::CollisionSphereRadius,
        PropertyFieldId::CollisionCapsuleRadius => EditorPropertyFieldIdV1::CollisionCapsuleRadius,
        PropertyFieldId::CollisionCapsuleHalfHeight => EditorPropertyFieldIdV1::CollisionCapsuleHalfHeight,
        PropertyFieldId::PrimitiveKind => EditorPropertyFieldIdV1::PrimitiveKind,
        PropertyFieldId::PrimitiveColor => EditorPropertyFieldIdV1::PrimitiveColor,
        PropertyFieldId::LightAmbientColor => EditorPropertyFieldIdV1::LightAmbientColor,
        PropertyFieldId::LightAmbientIntensity => EditorPropertyFieldIdV1::LightAmbientIntensity,
        PropertyFieldId::LightColor => EditorPropertyFieldIdV1::LightColor,
        PropertyFieldId::LightIntensity => EditorPropertyFieldIdV1::LightIntensity,
        PropertyFieldId::LightRange => EditorPropertyFieldIdV1::LightRange,
        PropertyFieldId::LightYawDeg => EditorPropertyFieldIdV1::LightYawDeg,
        PropertyFieldId::LightPitchDeg => EditorPropertyFieldIdV1::LightPitchDeg,
        PropertyFieldId::MaterialAsset => EditorPropertyFieldIdV1::MaterialAsset,
        PropertyFieldId::MaterialBaseColor => EditorPropertyFieldIdV1::MaterialBaseColor,
        PropertyFieldId::MaterialMetallic => EditorPropertyFieldIdV1::MaterialMetallic,
        PropertyFieldId::MaterialRoughness => EditorPropertyFieldIdV1::MaterialRoughness,
        PropertyFieldId::MaterialEmissiveColor => EditorPropertyFieldIdV1::MaterialEmissiveColor,
        PropertyFieldId::MaterialEmissiveStrength => EditorPropertyFieldIdV1::MaterialEmissiveStrength,
        PropertyFieldId::MaterialNormalScale => EditorPropertyFieldIdV1::MaterialNormalScale,
        PropertyFieldId::MaterialAoStrength => EditorPropertyFieldIdV1::MaterialAoStrength,
        PropertyFieldId::MaterialAlphaCutoff => EditorPropertyFieldIdV1::MaterialAlphaCutoff,
        PropertyFieldId::ImportedAssetPath => EditorPropertyFieldIdV1::ImportedAssetPath,
        PropertyFieldId::ImportedAssetKind => EditorPropertyFieldIdV1::ImportedAssetKind,
        PropertyFieldId::ImportedAssetRepresentation => EditorPropertyFieldIdV1::ImportedAssetRepresentation,
    }
}

#[inline]
pub fn from_abi_property_field_id(id: EditorPropertyFieldIdV1) -> Option<PropertyFieldId> {
    Some(match id {
        EditorPropertyFieldIdV1::Name => PropertyFieldId::Name,
        EditorPropertyFieldIdV1::Kind => PropertyFieldId::Kind,
        EditorPropertyFieldIdV1::Entity => PropertyFieldId::Entity,
        EditorPropertyFieldIdV1::DisplayMode => PropertyFieldId::DisplayMode,
        EditorPropertyFieldIdV1::Position => PropertyFieldId::Position,
        EditorPropertyFieldIdV1::RotationDeg => PropertyFieldId::RotationDeg,
        EditorPropertyFieldIdV1::Scale => PropertyFieldId::Scale,
        EditorPropertyFieldIdV1::SnapTranslate => PropertyFieldId::SnapTranslate,
        EditorPropertyFieldIdV1::SnapRotate => PropertyFieldId::SnapRotate,
        EditorPropertyFieldIdV1::SnapScale => PropertyFieldId::SnapScale,
        EditorPropertyFieldIdV1::GameplayRole => PropertyFieldId::GameplayRole,
        EditorPropertyFieldIdV1::CollisionEnabled => PropertyFieldId::CollisionEnabled,
        EditorPropertyFieldIdV1::CollisionDynamic => PropertyFieldId::CollisionDynamic,
        EditorPropertyFieldIdV1::CollisionTrigger => PropertyFieldId::CollisionTrigger,
        EditorPropertyFieldIdV1::CollisionShape => PropertyFieldId::CollisionShape,
        EditorPropertyFieldIdV1::CollisionBoxExtents => PropertyFieldId::CollisionBoxExtents,
        EditorPropertyFieldIdV1::CollisionSphereRadius => PropertyFieldId::CollisionSphereRadius,
        EditorPropertyFieldIdV1::CollisionCapsuleRadius => PropertyFieldId::CollisionCapsuleRadius,
        EditorPropertyFieldIdV1::CollisionCapsuleHalfHeight => PropertyFieldId::CollisionCapsuleHalfHeight,
        EditorPropertyFieldIdV1::PrimitiveKind => PropertyFieldId::PrimitiveKind,
        EditorPropertyFieldIdV1::PrimitiveColor => PropertyFieldId::PrimitiveColor,
        EditorPropertyFieldIdV1::LightAmbientColor => PropertyFieldId::LightAmbientColor,
        EditorPropertyFieldIdV1::LightAmbientIntensity => PropertyFieldId::LightAmbientIntensity,
        EditorPropertyFieldIdV1::LightColor => PropertyFieldId::LightColor,
        EditorPropertyFieldIdV1::LightIntensity => PropertyFieldId::LightIntensity,
        EditorPropertyFieldIdV1::LightRange => PropertyFieldId::LightRange,
        EditorPropertyFieldIdV1::LightYawDeg => PropertyFieldId::LightYawDeg,
        EditorPropertyFieldIdV1::LightPitchDeg => PropertyFieldId::LightPitchDeg,
        EditorPropertyFieldIdV1::MaterialAsset => PropertyFieldId::MaterialAsset,
        EditorPropertyFieldIdV1::MaterialBaseColor => PropertyFieldId::MaterialBaseColor,
        EditorPropertyFieldIdV1::MaterialMetallic => PropertyFieldId::MaterialMetallic,
        EditorPropertyFieldIdV1::MaterialRoughness => PropertyFieldId::MaterialRoughness,
        EditorPropertyFieldIdV1::MaterialEmissiveColor => PropertyFieldId::MaterialEmissiveColor,
        EditorPropertyFieldIdV1::MaterialEmissiveStrength => PropertyFieldId::MaterialEmissiveStrength,
        EditorPropertyFieldIdV1::MaterialNormalScale => PropertyFieldId::MaterialNormalScale,
        EditorPropertyFieldIdV1::MaterialAoStrength => PropertyFieldId::MaterialAoStrength,
        EditorPropertyFieldIdV1::MaterialAlphaCutoff => PropertyFieldId::MaterialAlphaCutoff,
        EditorPropertyFieldIdV1::ImportedAssetPath => PropertyFieldId::ImportedAssetPath,
        EditorPropertyFieldIdV1::ImportedAssetKind => PropertyFieldId::ImportedAssetKind,
        EditorPropertyFieldIdV1::ImportedAssetRepresentation => PropertyFieldId::ImportedAssetRepresentation,
    })
}

#[inline]
pub fn to_abi_field_editor_kind(kind: FieldEditorKind) -> EditorFieldEditorKindV1 {
    match kind {
        FieldEditorKind::ReadOnlyText => EditorFieldEditorKindV1::ReadOnlyText,
        FieldEditorKind::Vec3 => EditorFieldEditorKindV1::Vec3,
        FieldEditorKind::Toggle => EditorFieldEditorKindV1::Toggle,
        FieldEditorKind::EnumChoice => EditorFieldEditorKindV1::EnumChoice,
        FieldEditorKind::Color3 => EditorFieldEditorKindV1::Color3,
        FieldEditorKind::Color4 => EditorFieldEditorKindV1::Color4,
        FieldEditorKind::Scalar => EditorFieldEditorKindV1::Scalar,
        FieldEditorKind::MaterialChoice => EditorFieldEditorKindV1::MaterialChoice,
    }
}

#[inline]
pub fn from_abi_field_editor_kind(kind: EditorFieldEditorKindV1) -> Option<FieldEditorKind> {
    Some(match kind {
        EditorFieldEditorKindV1::ReadOnlyText => FieldEditorKind::ReadOnlyText,
        EditorFieldEditorKindV1::Vec3 => FieldEditorKind::Vec3,
        EditorFieldEditorKindV1::Toggle => FieldEditorKind::Toggle,
        EditorFieldEditorKindV1::EnumChoice => FieldEditorKind::EnumChoice,
        EditorFieldEditorKindV1::Color3 => FieldEditorKind::Color3,
        EditorFieldEditorKindV1::Color4 => FieldEditorKind::Color4,
        EditorFieldEditorKindV1::Scalar => FieldEditorKind::Scalar,
        EditorFieldEditorKindV1::MaterialChoice => FieldEditorKind::MaterialChoice,
    })
}

#[inline]
pub fn to_abi_context_action_id(id: ContextActionId) -> EditorContextActionIdV1 {
    match id {
        ContextActionId::FrameSelection => EditorContextActionIdV1::FrameSelection,
        ContextActionId::Deselect => EditorContextActionIdV1::Deselect,
        ContextActionId::ToggleCollisionOverlay => EditorContextActionIdV1::ToggleCollisionOverlay,
        ContextActionId::EnterPlay => EditorContextActionIdV1::EnterPlay,
        ContextActionId::EnterSimulate => EditorContextActionIdV1::EnterSimulate,
        ContextActionId::StopRuntime => EditorContextActionIdV1::StopRuntime,
        ContextActionId::SelectTool => EditorContextActionIdV1::SelectTool,
        ContextActionId::MoveTool => EditorContextActionIdV1::MoveTool,
        ContextActionId::RotateTool => EditorContextActionIdV1::RotateTool,
        ContextActionId::ScaleTool => EditorContextActionIdV1::ScaleTool,
        ContextActionId::AddCollision => EditorContextActionIdV1::AddCollision,
        ContextActionId::RemoveCollision => EditorContextActionIdV1::RemoveCollision,
        ContextActionId::SpawnAssetHere => EditorContextActionIdV1::SpawnAssetHere,
    }
}

#[inline]
pub fn from_abi_context_action_id(id: EditorContextActionIdV1) -> Option<ContextActionId> {
    Some(match id {
        EditorContextActionIdV1::FrameSelection => ContextActionId::FrameSelection,
        EditorContextActionIdV1::Deselect => ContextActionId::Deselect,
        EditorContextActionIdV1::ToggleCollisionOverlay => ContextActionId::ToggleCollisionOverlay,
        EditorContextActionIdV1::EnterPlay => ContextActionId::EnterPlay,
        EditorContextActionIdV1::EnterSimulate => ContextActionId::EnterSimulate,
        EditorContextActionIdV1::StopRuntime => ContextActionId::StopRuntime,
        EditorContextActionIdV1::SelectTool => ContextActionId::SelectTool,
        EditorContextActionIdV1::MoveTool => ContextActionId::MoveTool,
        EditorContextActionIdV1::RotateTool => ContextActionId::RotateTool,
        EditorContextActionIdV1::ScaleTool => ContextActionId::ScaleTool,
        EditorContextActionIdV1::AddCollision => ContextActionId::AddCollision,
        EditorContextActionIdV1::RemoveCollision => ContextActionId::RemoveCollision,
        EditorContextActionIdV1::SpawnAssetHere => ContextActionId::SpawnAssetHere,
    })
}

#[inline]
pub fn to_abi_selection_kind(kind: SelectionKind) -> EditorSelectionKindV1 {
    match kind {
        SelectionKind::Actor => EditorSelectionKindV1::Actor,
        SelectionKind::Primitive => EditorSelectionKindV1::Primitive,
        SelectionKind::DirectionalLight => EditorSelectionKindV1::DirectionalLight,
        SelectionKind::PointLight => EditorSelectionKindV1::PointLight,
        SelectionKind::Player => EditorSelectionKindV1::Player,
    }
}

#[inline]
pub fn to_abi_display_mode(mode: DisplayMode) -> EditorDisplayModeV1 {
    match mode {
        DisplayMode::Both => EditorDisplayModeV1::Both,
        DisplayMode::EditorOnly => EditorDisplayModeV1::EditorOnly,
        DisplayMode::GameOnly => EditorDisplayModeV1::GameOnly,
    }
}

#[inline]
pub fn from_abi_display_mode(mode: EditorDisplayModeV1) -> DisplayMode {
    match mode {
        EditorDisplayModeV1::Both => DisplayMode::Both,
        EditorDisplayModeV1::EditorOnly => DisplayMode::EditorOnly,
        EditorDisplayModeV1::GameOnly => DisplayMode::GameOnly,
    }
}

#[inline]
pub fn to_abi_tool_id(tool: newengine_editor_core::ToolId) -> EditorToolIdV1 {
    match tool {
        newengine_editor_core::ToolId::Select => EditorToolIdV1::Select,
        newengine_editor_core::ToolId::Translate => EditorToolIdV1::Translate,
        newengine_editor_core::ToolId::Rotate => EditorToolIdV1::Rotate,
        newengine_editor_core::ToolId::Scale => EditorToolIdV1::Scale,
    }
}

#[inline]
pub fn from_abi_tool_id(tool: EditorToolIdV1) -> newengine_editor_core::ToolId {
    match tool {
        EditorToolIdV1::Select => newengine_editor_core::ToolId::Select,
        EditorToolIdV1::Translate => newengine_editor_core::ToolId::Translate,
        EditorToolIdV1::Rotate => newengine_editor_core::ToolId::Rotate,
        EditorToolIdV1::Scale => newengine_editor_core::ToolId::Scale,
    }
}

#[inline]
pub fn to_abi_play_mode(mode: EditorPlayMode) -> EditorPlayModeV1 {
    match mode {
        EditorPlayMode::Edit => EditorPlayModeV1::Edit,
        EditorPlayMode::Simulate => EditorPlayModeV1::Simulate,
        EditorPlayMode::Play => EditorPlayModeV1::Play,
    }
}

#[inline]
pub fn from_abi_play_mode(mode: EditorPlayModeV1) -> EditorPlayMode {
    match mode {
        EditorPlayModeV1::Edit => EditorPlayMode::Edit,
        EditorPlayModeV1::Simulate => EditorPlayMode::Simulate,
        EditorPlayModeV1::Play => EditorPlayMode::Play,
    }
}

#[inline]
pub fn to_abi_viewport_mode(mode: ViewportMode) -> EditorViewportModeV1 {
    match mode {
        ViewportMode::Lit => EditorViewportModeV1::Lit,
        ViewportMode::Unlit => EditorViewportModeV1::Unlit,
        ViewportMode::Wireframe => EditorViewportModeV1::Wireframe,
        ViewportMode::Collision => EditorViewportModeV1::Collision,
    }
}

#[inline]
pub fn from_abi_viewport_mode(mode: EditorViewportModeV1) -> ViewportMode {
    match mode {
        EditorViewportModeV1::Lit => ViewportMode::Lit,
        EditorViewportModeV1::Unlit => ViewportMode::Unlit,
        EditorViewportModeV1::Wireframe => ViewportMode::Wireframe,
        EditorViewportModeV1::Collision => ViewportMode::Collision,
    }
}

#[inline]
pub fn to_abi_workspace_preset(preset: WorkspacePreset) -> EditorWorkspacePresetV1 {
    match preset {
        WorkspacePreset::Minimal => EditorWorkspacePresetV1::Minimal,
        WorkspacePreset::Editing => EditorWorkspacePresetV1::Editing,
        WorkspacePreset::Debug => EditorWorkspacePresetV1::Debug,
    }
}

#[inline]
pub fn from_abi_workspace_preset(preset: EditorWorkspacePresetV1) -> WorkspacePreset {
    match preset {
        EditorWorkspacePresetV1::Minimal => WorkspacePreset::Minimal,
        EditorWorkspacePresetV1::Editing => WorkspacePreset::Editing,
        EditorWorkspacePresetV1::Debug => WorkspacePreset::Debug,
    }
}

#[inline]
pub fn to_abi_asset_import_class(class: AssetImportClass) -> EditorAssetImportClassV1 {
    match class {
        AssetImportClass::Model => EditorAssetImportClassV1::Model,
        AssetImportClass::Scene => EditorAssetImportClassV1::Scene,
        AssetImportClass::Texture => EditorAssetImportClassV1::Texture,
        AssetImportClass::Material => EditorAssetImportClassV1::Material,
        AssetImportClass::Unknown => EditorAssetImportClassV1::Unknown,
    }
}

#[inline]
pub fn from_abi_asset_import_class(class: EditorAssetImportClassV1) -> AssetImportClass {
    match class {
        EditorAssetImportClassV1::Model => AssetImportClass::Model,
        EditorAssetImportClassV1::Scene => AssetImportClass::Scene,
        EditorAssetImportClassV1::Texture => AssetImportClass::Texture,
        EditorAssetImportClassV1::Material => AssetImportClass::Material,
        EditorAssetImportClassV1::Unknown => AssetImportClass::Unknown,
    }
}

#[inline]
pub fn to_abi_imported_asset_kind(kind: SceneImportedAssetKind) -> EditorImportedAssetKindV1 {
    match kind {
        SceneImportedAssetKind::StaticMesh => EditorImportedAssetKindV1::StaticMesh,
        SceneImportedAssetKind::SceneReference => EditorImportedAssetKindV1::SceneReference,
        SceneImportedAssetKind::TextureReference => EditorImportedAssetKindV1::TextureReference,
        SceneImportedAssetKind::MaterialReference => EditorImportedAssetKindV1::MaterialReference,
        SceneImportedAssetKind::OpaqueReference => EditorImportedAssetKindV1::OpaqueReference,
    }
}

#[inline]
pub fn from_abi_imported_asset_kind(kind: EditorImportedAssetKindV1) -> SceneImportedAssetKind {
    match kind {
        EditorImportedAssetKindV1::StaticMesh => SceneImportedAssetKind::StaticMesh,
        EditorImportedAssetKindV1::SceneReference => SceneImportedAssetKind::SceneReference,
        EditorImportedAssetKindV1::TextureReference => SceneImportedAssetKind::TextureReference,
        EditorImportedAssetKindV1::MaterialReference => SceneImportedAssetKind::MaterialReference,
        EditorImportedAssetKindV1::OpaqueReference => SceneImportedAssetKind::OpaqueReference,
    }
}

#[inline]
pub fn to_abi_imported_asset_representation(repr: SceneImportedAssetRepresentation) -> EditorImportedAssetRepresentationV1 {
    match repr {
        SceneImportedAssetRepresentation::PrimitiveCube => EditorImportedAssetRepresentationV1::PrimitiveCube,
        SceneImportedAssetRepresentation::PrimitivePlane => EditorImportedAssetRepresentationV1::PrimitivePlane,
        SceneImportedAssetRepresentation::PrimitiveSphere => EditorImportedAssetRepresentationV1::PrimitiveSphere,
    }
}

#[inline]
pub fn from_abi_imported_asset_representation(repr: EditorImportedAssetRepresentationV1) -> SceneImportedAssetRepresentation {
    match repr {
        EditorImportedAssetRepresentationV1::PrimitiveCube => SceneImportedAssetRepresentation::PrimitiveCube,
        EditorImportedAssetRepresentationV1::PrimitivePlane => SceneImportedAssetRepresentation::PrimitivePlane,
        EditorImportedAssetRepresentationV1::PrimitiveSphere => SceneImportedAssetRepresentation::PrimitiveSphere,
    }
}

#[inline]
pub fn to_abi_imported_asset_assembly_kind(kind: SceneImportedAssetAssemblyKind) -> EditorImportedAssetAssemblyKindV1 {
    match kind {
        SceneImportedAssetAssemblyKind::StaticMeshActor => EditorImportedAssetAssemblyKindV1::StaticMeshActor,
        SceneImportedAssetAssemblyKind::SceneAnchor => EditorImportedAssetAssemblyKindV1::SceneAnchor,
        SceneImportedAssetAssemblyKind::TextureCard => EditorImportedAssetAssemblyKindV1::TextureCard,
        SceneImportedAssetAssemblyKind::MaterialPreviewSphere => EditorImportedAssetAssemblyKindV1::MaterialPreviewSphere,
        SceneImportedAssetAssemblyKind::OpaqueProxy => EditorImportedAssetAssemblyKindV1::OpaqueProxy,
    }
}

#[inline]
pub fn from_abi_imported_asset_assembly_kind(kind: EditorImportedAssetAssemblyKindV1) -> SceneImportedAssetAssemblyKind {
    match kind {
        EditorImportedAssetAssemblyKindV1::StaticMeshActor => SceneImportedAssetAssemblyKind::StaticMeshActor,
        EditorImportedAssetAssemblyKindV1::SceneAnchor => SceneImportedAssetAssemblyKind::SceneAnchor,
        EditorImportedAssetAssemblyKindV1::TextureCard => SceneImportedAssetAssemblyKind::TextureCard,
        EditorImportedAssetAssemblyKindV1::MaterialPreviewSphere => SceneImportedAssetAssemblyKind::MaterialPreviewSphere,
        EditorImportedAssetAssemblyKindV1::OpaqueProxy => SceneImportedAssetAssemblyKind::OpaqueProxy,
    }
}
