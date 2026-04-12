#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::Path;

use newengine_ecs::EntityId;

use crate::gameplay::{CollisionBody, DisplayMode, PlayerActor};
use crate::scene_bridge::{SceneImportedAssetAssemblyDescriptor, SceneImportedAssetAssemblyKind, SceneImportedAssetDescriptor, SceneImportedAssetKind, SceneImportedAssetRepresentation};
use crate::ui::{extension_abi, EditorUiBuild};

pub type PropertyFieldFactoryFn = fn(&EditorUiBuild, &SelectionContext, ComponentSchemaId, PropertySectionId) -> Vec<ComponentFieldFactory>;
pub type ContextActionProviderFn = fn(&EditorUiBuild, Option<&SelectionContext>) -> Vec<ContextActionSchema>;
pub type AssetImportProviderFn = fn(&str) -> Option<AssetImportDescriptor>;

#[derive(Debug, Clone)]
pub struct RegisteredFieldFactory {
    pub id: &'static str,
    pub component: Option<ComponentSchemaId>,
    pub section: Option<PropertySectionId>,
    pub factory: PropertyFieldFactoryFn,
}

#[derive(Debug, Clone)]
pub struct RegisteredContextActionProvider {
    pub id: &'static str,
    pub provider: ContextActionProviderFn,
}

#[derive(Debug, Clone)]
pub struct RegisteredAssetImportProvider {
    pub id: &'static str,
    pub provider: AssetImportProviderFn,
}

#[derive(Debug, Default)]
pub struct EditorSchemaRegistry {
    pub field_factories: Vec<RegisteredFieldFactory>,
    pub context_action_providers: Vec<RegisteredContextActionProvider>,
    pub asset_import_providers: Vec<RegisteredAssetImportProvider>,
}

#[derive(Debug, Clone)]
pub struct EditorSurfaceContext {
    pub play_mode: crate::gameplay::EditorPlayMode,
    pub runtime_active: bool,
    pub viewport_mode: crate::ui::ViewportMode,
    pub active_tool: newengine_editor_core::ToolId,
    pub camera_speed_label: &'static str,
    pub collision_overlay: bool,
    pub selection_count: usize,
    pub entity_count: usize,
    pub primary_selection: Option<SelectionContext>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PropertySectionId {
    Summary,
    Transform,
    Display,
    Gameplay,
    Collision,
    Primitive,
    Lighting,
    Material,
}

#[derive(Debug, Clone)]
pub struct PropertySectionSchema {
    pub id: PropertySectionId,
    pub label: &'static str,
    pub keywords: &'static str,
    pub visible: bool,
    pub components: Vec<ComponentSchemaId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComponentSchemaId {
    Identity,
    Transform,
    Display,
    Gameplay,
    Collision,
    Primitive,
    DirectionalLight,
    PointLight,
    Material,
    ImportedAsset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionKind {
    Actor,
    Primitive,
    DirectionalLight,
    PointLight,
    Player,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SelectionArchetype {
    Actor,
    Primitive,
    DirectionalLight,
    PointLight,
    Player,
    ImportedAsset,
}

#[derive(Debug, Clone)]
pub struct EditorSchemaContext {
    pub archetype: SelectionArchetype,
    pub runtime_active: bool,
    pub viewport_mode: crate::ui::ViewportMode,
    pub play_mode: crate::gameplay::EditorPlayMode,
}

#[derive(Debug, Clone)]
pub struct ComponentFieldFactory {
    pub component: ComponentSchemaId,
    pub fields: Vec<PropertyFieldSchema>,
}

#[derive(Debug, Clone)]
pub struct ArchetypeSchemaProvider {
    pub archetype: SelectionArchetype,
    pub components: Vec<ComponentSchemaId>,
}

#[derive(Debug, Clone)]
pub struct RuntimeStateSchemaProvider {
    pub runtime_active: bool,
    pub collision_overlay: bool,
    pub viewport_mode: crate::ui::ViewportMode,
}

#[derive(Debug, Clone)]
pub struct EditorStateSchemaProvider {
    pub active_tool: newengine_editor_core::ToolId,
    pub has_pending_asset_spawn: bool,
    pub camera_speed_label: &'static str,
}

#[derive(Debug, Clone)]
pub struct SelectionContext {
    pub entity: EntityId,
    pub name: String,
    pub kind: SelectionKind,
    pub has_collision: bool,
    pub display_mode: DisplayMode,
    pub components: Vec<ComponentSchemaId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContextActionId {
    FrameSelection,
    Deselect,
    ToggleCollisionOverlay,
    EnterPlay,
    EnterSimulate,
    StopRuntime,
    SelectTool,
    MoveTool,
    RotateTool,
    ScaleTool,
    AddCollision,
    RemoveCollision,
    SpawnAssetHere,
}

#[derive(Debug, Clone)]
pub struct ContextActionSchema {
    pub id: ContextActionId,
    pub label: &'static str,
    pub keywords: &'static str,
    pub enabled: bool,
    pub selected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PropertyFieldId {
    Name,
    Kind,
    Entity,
    DisplayMode,
    Position,
    RotationDeg,
    Scale,
    SnapTranslate,
    SnapRotate,
    SnapScale,
    GameplayRole,
    CollisionEnabled,
    CollisionDynamic,
    CollisionTrigger,
    CollisionShape,
    CollisionBoxExtents,
    CollisionSphereRadius,
    CollisionCapsuleRadius,
    CollisionCapsuleHalfHeight,
    PrimitiveKind,
    PrimitiveColor,
    LightAmbientColor,
    LightAmbientIntensity,
    LightColor,
    LightIntensity,
    LightRange,
    LightYawDeg,
    LightPitchDeg,
    MaterialAsset,
    MaterialBaseColor,
    MaterialMetallic,
    MaterialRoughness,
    MaterialEmissiveColor,
    MaterialEmissiveStrength,
    MaterialNormalScale,
    MaterialAoStrength,
    MaterialAlphaCutoff,
    ImportedAssetPath,
    ImportedAssetKind,
    ImportedAssetRepresentation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FieldEditorKind {
    ReadOnlyText,
    Vec3,
    Toggle,
    EnumChoice,
    Color3,
    Color4,
    Scalar,
    MaterialChoice,
}

#[derive(Debug, Clone)]
pub struct PropertyFieldSchema {
    pub id: PropertyFieldId,
    pub label: &'static str,
    pub keywords: &'static str,
    pub editor: FieldEditorKind,
    pub visible: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct AssetSpawnContract {
    pub logical_path: String,
    pub actor_name: String,
    pub import: AssetImportDescriptor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssetImportClass {
    Model,
    Scene,
    Texture,
    Material,
    Unknown,
}

impl AssetImportClass {
    #[inline]
    pub fn label(self) -> &'static str {
        match self {
            Self::Model => "Model",
            Self::Scene => "Scene",
            Self::Texture => "Texture",
            Self::Material => "Material",
            Self::Unknown => "Unknown",
        }
    }
}

#[derive(Debug, Clone)]
pub struct AssetImportDescriptor {
    pub class: AssetImportClass,
    pub representation: SceneImportedAssetRepresentation,
    pub import_kind: SceneImportedAssetKind,
    pub assembler_key: &'static str,
    pub assembly: SceneImportedAssetAssemblyDescriptor,
    pub default_scale: [f32; 3],
    pub tint: [f32; 4],
}

pub fn build_selection_context(me: &EditorUiBuild, entity: EntityId) -> SelectionContext {
    use newengine_lighting::{DirectionalLight, PointLight};
    use newengine_primitives::Primitive;
    use newengine_scene::components::Name;

    let scene = me.scene_bridge.scene();
    let s = scene.read();
    let w = s.world();

    let name = w
        .get::<Name>(entity)
        .map(|n| n.as_str().to_string())
        .unwrap_or_else(|| format!("Entity #{}", entity.stable_u64()));
    let kind = if w.get::<PlayerActor>(entity).is_some() {
        SelectionKind::Player
    } else if w.get::<DirectionalLight>(entity).is_some() {
        SelectionKind::DirectionalLight
    } else if w.get::<PointLight>(entity).is_some() {
        SelectionKind::PointLight
    } else if w.get::<Primitive>(entity).is_some() {
        SelectionKind::Primitive
    } else {
        SelectionKind::Actor
    };
    let has_collision = w.get::<CollisionBody>(entity).is_some();
    let display_mode = w
        .get::<crate::gameplay::DisplayVisibility>(entity)
        .copied()
        .unwrap_or_default()
        .mode;

    let mut components = vec![ComponentSchemaId::Identity, ComponentSchemaId::Transform, ComponentSchemaId::Display];
    match kind {
        SelectionKind::Primitive => {
            components.push(ComponentSchemaId::Primitive);
            components.push(ComponentSchemaId::Material);
        }
        SelectionKind::DirectionalLight => {
            components.push(ComponentSchemaId::DirectionalLight);
            components.push(ComponentSchemaId::Material);
        }
        SelectionKind::PointLight => {
            components.push(ComponentSchemaId::PointLight);
            components.push(ComponentSchemaId::Material);
        }
        SelectionKind::Player | SelectionKind::Actor => {
            components.push(ComponentSchemaId::Gameplay);
        }
    }
    if has_collision {
        components.push(ComponentSchemaId::Collision);
    }
    if w.get::<crate::scene_bridge::SceneImportedAssetDescriptor>(entity).is_some() {
        components.push(ComponentSchemaId::ImportedAsset);
    }
    if !matches!(kind, SelectionKind::Actor | SelectionKind::Player) && !components.contains(&ComponentSchemaId::Material) {
        components.push(ComponentSchemaId::Material);
    }

    SelectionContext {
        entity,
        name,
        kind,
        has_collision,
        display_mode,
        components,
    }
}

pub fn selection_archetype(ctx: &SelectionContext) -> SelectionArchetype {
    if ctx.components.contains(&ComponentSchemaId::ImportedAsset) {
        return SelectionArchetype::ImportedAsset;
    }
    match ctx.kind {
        SelectionKind::Actor => SelectionArchetype::Actor,
        SelectionKind::Primitive => SelectionArchetype::Primitive,
        SelectionKind::DirectionalLight => SelectionArchetype::DirectionalLight,
        SelectionKind::PointLight => SelectionArchetype::PointLight,
        SelectionKind::Player => SelectionArchetype::Player,
    }
}

pub fn build_editor_schema_context(me: &EditorUiBuild, selection: Option<&SelectionContext>) -> EditorSchemaContext {
    let play_mode = me.scene_bridge.play_mode();
    EditorSchemaContext {
        archetype: selection.map(selection_archetype).unwrap_or(SelectionArchetype::Actor),
        runtime_active: play_mode.is_runtime(),
        viewport_mode: me.viewport_mode,
        play_mode,
    }
}

pub fn archetype_provider(ctx: &SelectionContext) -> ArchetypeSchemaProvider {
    ArchetypeSchemaProvider {
        archetype: selection_archetype(ctx),
        components: ctx.components.clone(),
    }
}

pub fn runtime_state_provider(me: &EditorUiBuild) -> RuntimeStateSchemaProvider {
    RuntimeStateSchemaProvider {
        runtime_active: me.scene_bridge.play_mode().is_runtime(),
        collision_overlay: me.scene_bridge.collision_wireframe_enabled(),
        viewport_mode: me.viewport_mode,
    }
}

pub fn editor_state_provider(me: &EditorUiBuild) -> EditorStateSchemaProvider {
    EditorStateSchemaProvider {
        active_tool: me.editor.active_tool,
        has_pending_asset_spawn: me.asset_spawn_request.is_some(),
        camera_speed_label: me.camera_speed.active_label(),
    }
}

pub fn build_surface_context(me: &EditorUiBuild) -> EditorSurfaceContext {
    let primary_selection = me
        .editor
        .selection
        .primary()
        .map(|entity| build_selection_context(me, entity));
    EditorSurfaceContext {
        play_mode: me.scene_bridge.play_mode(),
        runtime_active: me.scene_bridge.play_mode().is_runtime(),
        viewport_mode: me.viewport_mode,
        active_tool: me.editor.active_tool,
        camera_speed_label: me.camera_speed.active_label(),
        collision_overlay: me.scene_bridge.collision_wireframe_enabled(),
        selection_count: me.editor.selection.len(),
        entity_count: me.scene_bridge.scene().read().world().entity_count(),
        primary_selection,
    }
}

pub fn property_sections(me: &EditorUiBuild, ctx: &SelectionContext) -> Vec<PropertySectionSchema> {
    let filter = me.details_filter.trim().to_ascii_lowercase();
    let wants = |label: &'static str, keywords: &'static str| {
        filter.is_empty()
            || label.to_ascii_lowercase().contains(&filter)
            || keywords.to_ascii_lowercase().contains(&filter)
    };
    let has = |id: ComponentSchemaId| ctx.components.contains(&id);

    [
        PropertySectionSchema { id: PropertySectionId::Summary, label: "Summary", keywords: "summary identity type selection", visible: wants("Summary", "summary identity type selection"), components: vec![ComponentSchemaId::Identity] },
        PropertySectionSchema { id: PropertySectionId::Transform, label: "Transform", keywords: "transform position rotation scale", visible: wants("Transform", "transform position rotation scale"), components: vec![ComponentSchemaId::Transform] },
        PropertySectionSchema { id: PropertySectionId::Display, label: "Display", keywords: "display visibility editor game", visible: wants("Display", "display visibility editor game"), components: vec![ComponentSchemaId::Display] },
        PropertySectionSchema { id: PropertySectionId::Gameplay, label: "Gameplay", keywords: "gameplay role actor player", visible: has(ComponentSchemaId::Gameplay) && wants("Gameplay", "gameplay role actor player"), components: vec![ComponentSchemaId::Gameplay] },
        PropertySectionSchema { id: PropertySectionId::Collision, label: "Collision", keywords: "collision physics shape body trigger", visible: wants("Collision", "collision physics shape body trigger"), components: vec![ComponentSchemaId::Collision] },
        PropertySectionSchema { id: PropertySectionId::Primitive, label: "Primitive", keywords: "primitive mesh color geometry", visible: has(ComponentSchemaId::Primitive) && wants("Primitive", "primitive mesh color geometry"), components: vec![ComponentSchemaId::Primitive] },
        PropertySectionSchema { id: PropertySectionId::Lighting, label: "Lighting", keywords: "lighting light ambient directional point", visible: (has(ComponentSchemaId::DirectionalLight) || has(ComponentSchemaId::PointLight)) && wants("Lighting", "lighting light ambient directional point"), components: vec![ComponentSchemaId::DirectionalLight, ComponentSchemaId::PointLight] },
        PropertySectionSchema { id: PropertySectionId::Material, label: "Material", keywords: "material shader surface", visible: has(ComponentSchemaId::Material) && wants("Material", "material shader surface"), components: vec![ComponentSchemaId::Material, ComponentSchemaId::ImportedAsset] },
    ]
        .into_iter()
        .filter(|s| s.visible)
        .collect()
}

pub fn property_fields(me: &EditorUiBuild, ctx: &SelectionContext, section: PropertySectionId) -> Vec<PropertyFieldSchema> {
    let mut out = Vec::new();
    for component in &ctx.components {
        for factory in component_field_factories(me, ctx, *component, section) {
            out.extend(factory.fields.into_iter().filter(|f| f.visible));
        }
    }

    if section == PropertySectionId::Summary {
        out.insert(0, field(PropertyFieldId::Name, "Name", "name identity label", FieldEditorKind::ReadOnlyText));
        out.insert(1, field(PropertyFieldId::Kind, "Kind", "kind type category", FieldEditorKind::ReadOnlyText));
        out.insert(2, field(PropertyFieldId::Entity, "Entity", "entity id", FieldEditorKind::ReadOnlyText));
    }

    let mut dedup = std::collections::HashSet::new();
    out.into_iter().filter(|f| dedup.insert(f.id)).collect()
}

pub fn component_field_factories(
    me: &EditorUiBuild,
    ctx: &SelectionContext,
    component: ComponentSchemaId,
    section: PropertySectionId,
) -> Vec<ComponentFieldFactory> {
    let fields = match (component, section) {
        (ComponentSchemaId::Identity, PropertySectionId::Summary) => vec![
            field(PropertyFieldId::DisplayMode, "Display", "display editor game", FieldEditorKind::ReadOnlyText),
        ],
        (ComponentSchemaId::ImportedAsset, PropertySectionId::Summary) => vec![
            field(PropertyFieldId::ImportedAssetPath, "Asset Path", "asset imported path", FieldEditorKind::ReadOnlyText),
            field(PropertyFieldId::ImportedAssetKind, "Import Kind", "asset imported kind", FieldEditorKind::ReadOnlyText),
            field(PropertyFieldId::ImportedAssetRepresentation, "Representation", "asset representation", FieldEditorKind::ReadOnlyText),
        ],
        (ComponentSchemaId::Transform, PropertySectionId::Transform) => vec![
            field(PropertyFieldId::Position, "Position", "position location", FieldEditorKind::Vec3),
            field(PropertyFieldId::RotationDeg, "Rotation (deg)", "rotation yaw pitch roll", FieldEditorKind::Vec3),
            field(PropertyFieldId::Scale, "Scale", "scale size", FieldEditorKind::Vec3),
            field(PropertyFieldId::SnapTranslate, "Snap Move", "snap translate move", FieldEditorKind::Toggle),
            field(PropertyFieldId::SnapRotate, "Snap Rotate", "snap rotate", FieldEditorKind::Toggle),
            field(PropertyFieldId::SnapScale, "Snap Scale", "snap scale", FieldEditorKind::Toggle),
        ],
        (ComponentSchemaId::Display, PropertySectionId::Display) => vec![
            field(PropertyFieldId::DisplayMode, "Display Mode", "display visibility editor game", FieldEditorKind::EnumChoice),
        ],
        (ComponentSchemaId::Gameplay, PropertySectionId::Gameplay) => vec![
            field(PropertyFieldId::GameplayRole, "Role", "role gameplay actor player", FieldEditorKind::ReadOnlyText),
        ],
        (ComponentSchemaId::Collision, PropertySectionId::Collision) => vec![
            field(PropertyFieldId::CollisionEnabled, "Enabled", "collision enabled", FieldEditorKind::Toggle),
            field(PropertyFieldId::CollisionDynamic, "Dynamic", "collision dynamic rigidbody", FieldEditorKind::Toggle),
            field(PropertyFieldId::CollisionTrigger, "Trigger", "collision trigger", FieldEditorKind::Toggle),
            field(PropertyFieldId::CollisionShape, "Shape", "collision shape", FieldEditorKind::EnumChoice),
            field(PropertyFieldId::CollisionBoxExtents, "Half Extents", "collision box extents", FieldEditorKind::Vec3),
            field(PropertyFieldId::CollisionSphereRadius, "Radius", "collision sphere radius", FieldEditorKind::Scalar),
            field(PropertyFieldId::CollisionCapsuleRadius, "Capsule Radius", "collision capsule radius", FieldEditorKind::Scalar),
            field(PropertyFieldId::CollisionCapsuleHalfHeight, "Half Height", "collision capsule height", FieldEditorKind::Scalar),
        ],
        (ComponentSchemaId::Primitive, PropertySectionId::Primitive) => vec![
            field(PropertyFieldId::PrimitiveKind, "Primitive", "primitive kind mesh", FieldEditorKind::ReadOnlyText),
            field(PropertyFieldId::PrimitiveColor, "Color", "primitive color tint", FieldEditorKind::Color4),
        ],
        (ComponentSchemaId::DirectionalLight, PropertySectionId::Lighting) => vec![
            field(PropertyFieldId::LightAmbientColor, "Ambient", "ambient color", FieldEditorKind::Color3),
            field(PropertyFieldId::LightAmbientIntensity, "Ambient Intensity", "ambient intensity", FieldEditorKind::Scalar),
            field(PropertyFieldId::LightColor, "Light Color", "light color", FieldEditorKind::Color3),
            field(PropertyFieldId::LightIntensity, "Intensity", "light intensity", FieldEditorKind::Scalar),
            field(PropertyFieldId::LightYawDeg, "Yaw (deg)", "directional yaw", FieldEditorKind::Scalar),
            field(PropertyFieldId::LightPitchDeg, "Pitch (deg)", "directional pitch", FieldEditorKind::Scalar),
        ],
        (ComponentSchemaId::PointLight, PropertySectionId::Lighting) => vec![
            field(PropertyFieldId::LightAmbientColor, "Ambient", "ambient color", FieldEditorKind::Color3),
            field(PropertyFieldId::LightAmbientIntensity, "Ambient Intensity", "ambient intensity", FieldEditorKind::Scalar),
            field(PropertyFieldId::LightColor, "Light Color", "light color", FieldEditorKind::Color3),
            field(PropertyFieldId::LightIntensity, "Intensity", "light intensity", FieldEditorKind::Scalar),
            field(PropertyFieldId::LightRange, "Range", "point light range", FieldEditorKind::Scalar),
        ],
        (ComponentSchemaId::Material, PropertySectionId::Material) => vec![
            field(PropertyFieldId::MaterialAsset, "Material", "material asset", FieldEditorKind::MaterialChoice),
            field(PropertyFieldId::MaterialBaseColor, "Base Color", "material base color", FieldEditorKind::Color4),
            field(PropertyFieldId::MaterialMetallic, "Metallic", "material metallic", FieldEditorKind::Scalar),
            field(PropertyFieldId::MaterialRoughness, "Roughness", "material roughness", FieldEditorKind::Scalar),
            field(PropertyFieldId::MaterialEmissiveColor, "Emissive Color", "material emissive color", FieldEditorKind::Color3),
            field(PropertyFieldId::MaterialEmissiveStrength, "Emissive Strength", "material emissive strength", FieldEditorKind::Scalar),
            field(PropertyFieldId::MaterialNormalScale, "Normal Scale", "material normal scale", FieldEditorKind::Scalar),
            field(PropertyFieldId::MaterialAoStrength, "AO Strength", "material ao strength", FieldEditorKind::Scalar),
            field(PropertyFieldId::MaterialAlphaCutoff, "Alpha Cutoff", "material alpha cutoff", FieldEditorKind::Scalar),
        ],
        _ => Vec::new(),
    };

    let fields: Vec<PropertyFieldSchema> = fields.into_iter().map(|mut f| {
        f.visible = match f.id {
            PropertyFieldId::ImportedAssetPath | PropertyFieldId::ImportedAssetKind | PropertyFieldId::ImportedAssetRepresentation => ctx.components.contains(&ComponentSchemaId::ImportedAsset),
            PropertyFieldId::CollisionDynamic
            | PropertyFieldId::CollisionTrigger
            | PropertyFieldId::CollisionShape
            | PropertyFieldId::CollisionBoxExtents
            | PropertyFieldId::CollisionSphereRadius
            | PropertyFieldId::CollisionCapsuleRadius
            | PropertyFieldId::CollisionCapsuleHalfHeight => ctx.has_collision,
            PropertyFieldId::PrimitiveKind | PropertyFieldId::PrimitiveColor => matches!(ctx.kind, SelectionKind::Primitive),
            PropertyFieldId::LightColor | PropertyFieldId::LightIntensity | PropertyFieldId::LightYawDeg | PropertyFieldId::LightPitchDeg => matches!(ctx.kind, SelectionKind::DirectionalLight),
            PropertyFieldId::LightRange => matches!(ctx.kind, SelectionKind::PointLight),
            _ => true,
        };
        f
    }).collect();

    let mut out = Vec::new();
    if !fields.is_empty() {
        out.push(ComponentFieldFactory { component, fields });
    }
    let registry = me.schema_registry.read();
    for ext in registry.field_factories.iter() {
        if ext.component.is_some() && ext.component != Some(component) {
            continue;
        }
        if ext.section.is_some() && ext.section != Some(section) {
            continue;
        }
        out.extend((ext.factory)(me, ctx, component, section));
    }
    drop(registry);

    let abi_registry = me.extension_registry.read();
    if !abi_registry.field_factories.is_empty() {
        let surface = extension_abi::to_abi_surface_context(me);
        let selection = extension_abi::to_abi_selection_context(ctx);
        let abi_component = extension_abi::to_abi_component_id(component);
        let abi_section = extension_abi::to_abi_section_id(section);
        for (_plugin_id, ext) in abi_registry.field_factories.iter() {
            let filter_component = ext.component().into_option().map(|id| extension_abi::from_abi_component_id(id));
            let filter_section = ext.section().into_option().map(|id| extension_abi::from_abi_section_id(id));
            if filter_component.is_some() && filter_component != Some(component) {
                continue;
            }
            if filter_section.is_some() && filter_section != Some(section) {
                continue;
            }
            let fields = ext.build_fields(surface.clone(), selection.clone(), abi_component, abi_section);
            let converted: Vec<_> = fields
                .into_iter()
                .filter_map(|field| extension_abi::from_abi_property_field_schema(field))
                .collect();
            if !converted.is_empty() {
                out.push(ComponentFieldFactory { component, fields: converted });
            }
        }
    }
    out
}

#[inline]
fn field(id: PropertyFieldId, label: &'static str, keywords: &'static str, editor: FieldEditorKind) -> PropertyFieldSchema {
    PropertyFieldSchema { id, label, keywords, editor, visible: true, enabled: true }
}

pub fn builtin_selection_context_actions(me: &EditorUiBuild, selection: Option<&SelectionContext>) -> Vec<ContextActionSchema> {
    let play_mode = me.scene_bridge.play_mode();
    let runtime_active = play_mode.is_runtime();
    let has_selection = selection.is_some();
    let has_collision = selection.map(|s| s.has_collision).unwrap_or(false);

    vec![
        ContextActionSchema { id: ContextActionId::FrameSelection, label: "Frame Selection", keywords: "frame focus selection", enabled: has_selection, selected: false },
        ContextActionSchema { id: ContextActionId::Deselect, label: "Deselect", keywords: "clear deselect selection", enabled: has_selection, selected: false },
        ContextActionSchema { id: ContextActionId::ToggleCollisionOverlay, label: "Collision Overlay", keywords: "collision overlay wireframe", enabled: true, selected: me.scene_bridge.collision_wireframe_enabled() },
        ContextActionSchema { id: ContextActionId::EnterPlay, label: "Play", keywords: "play runtime", enabled: true, selected: play_mode == crate::gameplay::EditorPlayMode::Play },
        ContextActionSchema { id: ContextActionId::EnterSimulate, label: "Simulate", keywords: "simulate runtime", enabled: true, selected: play_mode == crate::gameplay::EditorPlayMode::Simulate },
        ContextActionSchema { id: ContextActionId::StopRuntime, label: "Stop", keywords: "stop runtime", enabled: runtime_active, selected: false },
        ContextActionSchema { id: ContextActionId::SelectTool, label: "Select Tool", keywords: "tool select", enabled: true, selected: me.editor.active_tool == newengine_editor_core::ToolId::Select },
        ContextActionSchema { id: ContextActionId::MoveTool, label: "Move Tool", keywords: "tool move translate", enabled: true, selected: me.editor.active_tool == newengine_editor_core::ToolId::Translate },
        ContextActionSchema { id: ContextActionId::RotateTool, label: "Rotate Tool", keywords: "tool rotate", enabled: true, selected: me.editor.active_tool == newengine_editor_core::ToolId::Rotate },
        ContextActionSchema { id: ContextActionId::ScaleTool, label: "Scale Tool", keywords: "tool scale", enabled: true, selected: me.editor.active_tool == newengine_editor_core::ToolId::Scale },
        ContextActionSchema { id: ContextActionId::AddCollision, label: "Add Collision", keywords: "collision add body", enabled: has_selection && !has_collision, selected: false },
        ContextActionSchema { id: ContextActionId::RemoveCollision, label: "Remove Collision", keywords: "collision remove body", enabled: has_collision, selected: false },
        ContextActionSchema { id: ContextActionId::SpawnAssetHere, label: "Spawn Dropped Asset", keywords: "spawn dropped asset here", enabled: me.asset_spawn_request.is_some(), selected: false },
    ]
}

pub fn selection_context_actions(me: &EditorUiBuild, selection: Option<&SelectionContext>) -> Vec<ContextActionSchema> {
    let mut out = builtin_selection_context_actions(me, selection);
    let registry = me.schema_registry.read();
    for provider in registry.context_action_providers.iter() {
        out.extend((provider.provider)(me, selection));
    }
    drop(registry);

    let abi_registry = me.extension_registry.read();
    if !abi_registry.context_action_providers.is_empty() {
        let surface = extension_abi::to_abi_surface_context(me);
        let abi_selection = match selection {
            Some(selection) => abi_stable::std_types::ROption::RSome(extension_abi::to_abi_selection_context(selection)),
            None => abi_stable::std_types::ROption::RNone,
        };
        for (_plugin_id, provider) in abi_registry.context_action_providers.iter() {
            let actions = provider.build_actions(surface.clone(), abi_selection.clone());
            out.extend(actions.into_iter().filter_map(|action| extension_abi::from_abi_context_action_schema(action)));
        }
    }
    let mut dedup = std::collections::HashSet::new();
    out.into_iter().filter(|it| dedup.insert(it.id)).collect()
}

pub fn infer_asset_spawn_contract(registry: &EditorSchemaRegistry, path: &str) -> AssetSpawnContract {
    infer_asset_spawn_contract_with_abi(registry, None, path)
}

pub fn infer_asset_spawn_contract_with_abi(
    registry: &EditorSchemaRegistry,
    abi_registry: Option<&extension_abi::EditorExtensionAbiRegistry>,
    path: &str,
) -> AssetSpawnContract {
    let logical_path = path.trim().to_string();
    let asset_path = Path::new(&logical_path);
    let stem = asset_path
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("Asset")
        .to_string();
    let import = infer_asset_import_descriptor_with_abi(registry, abi_registry, &logical_path);

    AssetSpawnContract {
        logical_path,
        actor_name: stem,
        import,
    }
}

pub fn infer_asset_import_descriptor(registry: &EditorSchemaRegistry, path: &str) -> AssetImportDescriptor {
    infer_asset_import_descriptor_with_abi(registry, None, path)
}

pub fn infer_asset_import_descriptor_with_abi(
    registry: &EditorSchemaRegistry,
    abi_registry: Option<&extension_abi::EditorExtensionAbiRegistry>,
    path: &str,
) -> AssetImportDescriptor {
    let ext = Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    for provider in registry.asset_import_providers.iter() {
        if let Some(import) = (provider.provider)(path) {
            return import;
        }
    }

    if let Some(abi_registry) = abi_registry {
        for (_plugin_id, provider) in abi_registry.asset_import_providers.iter() {
            if let Some(import) = provider.infer_import(path.to_string().into()).into_option().and_then(extension_abi::from_abi_asset_import_descriptor) {
                return import;
            }
        }
    }

    match ext.as_str() {
        "gltf" | "glb" | "fbx" | "obj" | "dae" | "stl" | "ply" | "blend" => AssetImportDescriptor {
            class: AssetImportClass::Model,
            representation: SceneImportedAssetRepresentation::PrimitiveCube,
            import_kind: SceneImportedAssetKind::StaticMesh,
            assembler_key: "builtin.static_mesh_actor",
            assembly: SceneImportedAssetAssemblyDescriptor {
                assembly: SceneImportedAssetAssemblyKind::StaticMeshActor,
                primitive_id: newengine_primitives::builtins::ID_CUBE,
                display_mode: DisplayMode::Both,
                with_collision: true,
                dynamic_collision: false,
            },
            default_scale: [1.25, 1.25, 1.25],
            tint: [0.72, 0.76, 0.84, 1.0],
        },
        "scene" | "scn" => AssetImportDescriptor {
            class: AssetImportClass::Scene,
            representation: SceneImportedAssetRepresentation::PrimitivePlane,
            import_kind: SceneImportedAssetKind::SceneReference,
            assembler_key: "builtin.scene_anchor",
            assembly: SceneImportedAssetAssemblyDescriptor {
                assembly: SceneImportedAssetAssemblyKind::SceneAnchor,
                primitive_id: newengine_primitives::builtins::ID_PLANE,
                display_mode: DisplayMode::EditorOnly,
                with_collision: false,
                dynamic_collision: false,
            },
            default_scale: [1.5, 1.0, 1.5],
            tint: [0.58, 0.72, 0.86, 1.0],
        },
        "png" | "jpg" | "jpeg" | "tga" | "dds" | "ktx2" => AssetImportDescriptor {
            class: AssetImportClass::Texture,
            representation: SceneImportedAssetRepresentation::PrimitivePlane,
            import_kind: SceneImportedAssetKind::TextureReference,
            assembler_key: "builtin.texture_card",
            assembly: SceneImportedAssetAssemblyDescriptor {
                assembly: SceneImportedAssetAssemblyKind::TextureCard,
                primitive_id: newengine_primitives::builtins::ID_PLANE,
                display_mode: DisplayMode::Both,
                with_collision: true,
                dynamic_collision: false,
            },
            default_scale: [1.5, 1.0, 1.5],
            tint: [0.86, 0.82, 0.68, 1.0],
        },
        "nemat" | "mat" | "material" => AssetImportDescriptor {
            class: AssetImportClass::Material,
            representation: SceneImportedAssetRepresentation::PrimitiveSphere,
            import_kind: SceneImportedAssetKind::MaterialReference,
            assembler_key: "builtin.material_preview_sphere",
            assembly: SceneImportedAssetAssemblyDescriptor {
                assembly: SceneImportedAssetAssemblyKind::MaterialPreviewSphere,
                primitive_id: newengine_primitives::builtins::ID_SPHERE_UV,
                display_mode: DisplayMode::Both,
                with_collision: true,
                dynamic_collision: false,
            },
            default_scale: [1.0, 1.0, 1.0],
            tint: [0.7, 0.86, 0.76, 1.0],
        },
        _ => AssetImportDescriptor {
            class: AssetImportClass::Unknown,
            representation: SceneImportedAssetRepresentation::PrimitiveCube,
            import_kind: SceneImportedAssetKind::OpaqueReference,
            assembler_key: "builtin.opaque_proxy",
            assembly: SceneImportedAssetAssemblyDescriptor {
                assembly: SceneImportedAssetAssemblyKind::OpaqueProxy,
                primitive_id: newengine_primitives::builtins::ID_CUBE,
                display_mode: DisplayMode::Both,
                with_collision: true,
                dynamic_collision: false,
            },
            default_scale: [1.0, 1.0, 1.0],
            tint: [0.64, 0.64, 0.7, 1.0],
        },
    }
}

#[inline]
pub fn to_scene_import_descriptor(contract: &AssetSpawnContract) -> SceneImportedAssetDescriptor {
    SceneImportedAssetDescriptor {
        logical_path: contract.logical_path.clone(),
        import_kind: contract.import.import_kind,
        representation: contract.import.representation,
        assembler_key: contract.import.assembler_key.to_string(),
        assembly: contract.import.assembly.clone(),
        default_scale: contract.import.default_scale,
        tint: contract.import.tint,
    }
}
