#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::EntityId;

use crate::gameplay::DisplayMode;
use crate::scene_bridge::{
    SceneImportedAssetAssemblyDescriptor, SceneImportedAssetKind,
    SceneImportedAssetRepresentation,
};
use crate::ui::EditorUiBuild;

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
pub(crate) struct EditorSurfaceContext {
    pub(crate) play_mode: crate::gameplay::EditorPlayMode,
    pub(crate) runtime_active: bool,
    pub(crate) viewport_mode: crate::ui::ViewportMode,
    pub(crate) active_tool: newengine_editor_core::ToolId,
    pub(crate) camera_speed_label: &'static str,
    pub(crate) collision_overlay: bool,
    pub(crate) selection_count: usize,
    pub(crate) entity_count: usize,
    pub(crate) primary_selection: Option<SelectionContext>,
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
pub(crate) struct EditorSchemaContext {
    pub(crate) archetype: SelectionArchetype,
    pub(crate) runtime_active: bool,
    pub(crate) viewport_mode: crate::ui::ViewportMode,
    pub(crate) play_mode: crate::gameplay::EditorPlayMode,
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
pub(crate) struct RuntimeStateSchemaProvider {
    pub(crate) runtime_active: bool,
    pub(crate) collision_overlay: bool,
    pub(crate) viewport_mode: crate::ui::ViewportMode,
}

#[derive(Debug, Clone)]
pub struct EditorStateSchemaProvider {
    pub(crate) active_tool: newengine_editor_core::ToolId,
    pub has_pending_asset_spawn: bool,
    pub(crate) camera_speed_label: &'static str,
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

