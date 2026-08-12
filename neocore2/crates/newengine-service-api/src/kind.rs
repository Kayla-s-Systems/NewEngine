use serde::{Serialize, Serializer};

use crate::{normalize_engine_gateway_id, ENGINE_SERVICE_GATEWAY_PREFIX};

/// Engine-side vocabulary for service provider kinds accepted by the host.
///
/// Plugins do not need to import this enum or know the full set. They describe
/// themselves with string metadata such as `service_kind = "render"`; the
/// host validates that string against this vocabulary and ignores unknown kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EngineServiceKind {
    Assets,
    AssetVfs,
    AssetTypes,
    AssetInspect,
    AssetEdit,
    AssetPackages,
    AssetListFiles,
    AssetUid,
    AssetDependencies,
    AssetImportQueue,
    AssetPackageWriter,
    AssetMaps,
    AssetValidation,
    AssetUi,
    Materials,
    Textures,
    Definitions,
    AssetGraph,
    Time,
    Schema,
    Animation,
    Navigation,
    Ai,
    Tags,
    Tasks,
    Scripting,
    Audio,
    Render,
    RenderEffects,
    RenderMaterials,
    Model,
    ModelSkeletons,
    ModelMaterials,
    ModelCollisions,
    Camera,
    CameraModes,
    CameraAnimations,
    Scene,
    Physics,
    PhysicsContacts,
    PhysicsConstraints,
    Input,
    InputBindings,
    InputActions,
    InputContexts,
    Ui,
    UiText,
    UiDebug,
    Logging,
    Loading,
    Threading,
    Platform,
    Ecs,
    Entity,
    PluginHost,
    Abi,
    GatewayRegistry,
    Security,
    SchedulerCore,
    CapabilityValidator,
}

impl Serialize for EngineServiceKind {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl AsRef<str> for EngineServiceKind {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl EngineServiceKind {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Assets => "assets",
            Self::AssetVfs => "assets.vfs",
            Self::AssetTypes => "assets.types",
            Self::AssetInspect => "assets.inspect",
            Self::AssetEdit => "assets.edit",
            Self::AssetPackages => "assets.packages",
            Self::AssetListFiles => "assets.listfiles",
            Self::AssetUid => "assets.uid",
            Self::AssetDependencies => "assets.dependencies",
            Self::AssetImportQueue => "assets.import_queue",
            Self::AssetPackageWriter => "assets.package_writer",
            Self::AssetMaps => "assets.maps",
            Self::AssetValidation => "assets.validation",
            Self::AssetUi => "assets.ui",
            Self::Materials => "assets.materials",
            Self::Textures => "assets.textures",
            Self::Definitions => "assets.definitions",
            Self::AssetGraph => "assets.graph",
            Self::Time => "time",
            Self::Schema => "schema",
            Self::Animation => "animation",
            Self::Navigation => "navigation",
            Self::Ai => "ai",
            Self::Tags => "tags",
            Self::Tasks => "tasks",
            Self::Scripting => "scripting",
            Self::Audio => "audio",
            Self::Render => "render",
            Self::RenderEffects => "render.effects",
            Self::RenderMaterials => "render.materials",
            Self::Model => "assets.models",
            Self::ModelSkeletons => "assets.models.skeletons",
            Self::ModelMaterials => "assets.models.materials",
            Self::ModelCollisions => "assets.models.collisions",
            Self::Camera => "camera",
            Self::CameraModes => "camera.modes",
            Self::CameraAnimations => "camera.animations",
            Self::Scene => "scene",
            Self::Physics => "physics",
            Self::PhysicsContacts => "physics.contacts",
            Self::PhysicsConstraints => "physics.constraints",
            Self::Input => "input",
            Self::InputBindings => "input.bindings",
            Self::InputActions => "input.actions",
            Self::InputContexts => "input.contexts",
            Self::Ui => "ui",
            Self::UiText => "ui.text",
            Self::UiDebug => "ui.debug",
            Self::Logging => "logging",
            Self::Loading => "loading",
            Self::Threading => "threading",
            Self::Platform => "platform",
            Self::Ecs => "ecs",
            Self::Entity => "entity",
            Self::PluginHost => "plugin_host",
            Self::Abi => "abi",
            Self::GatewayRegistry => "gateway_registry",
            Self::Security => "security",
            Self::SchedulerCore => "scheduler.core",
            Self::CapabilityValidator => "capability_validator",
        }
    }

    #[inline]
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "assets" => Some(Self::Assets),
            "assets.vfs" | "assets_vfs" => Some(Self::AssetVfs),
            "assets.types" | "assets_types" => Some(Self::AssetTypes),
            "assets.inspect" | "assets_inspect" => Some(Self::AssetInspect),
            "assets.edit" | "assets_edit" => Some(Self::AssetEdit),
            "assets.packages" | "assets_packages" => Some(Self::AssetPackages),
            "assets.listfiles" | "assets_listfiles" | "assets.list_files" | "assets_list_files" => {
                Some(Self::AssetListFiles)
            }
            "assets.uid" | "assets_uid" => Some(Self::AssetUid),
            "assets.dependencies" | "assets_dependencies" => Some(Self::AssetDependencies),
            "assets.import_queue" | "assets_import_queue" | "assets.import-queue" => {
                Some(Self::AssetImportQueue)
            }
            "assets.package_writer" | "assets_package_writer" | "assets.package-writer" => {
                Some(Self::AssetPackageWriter)
            }
            "assets.maps" | "assets_maps" => Some(Self::AssetMaps),
            "assets.validation" | "assets_validation" => Some(Self::AssetValidation),
            "assets.ui" | "assets_ui" => Some(Self::AssetUi),
            "assets.materials" | "assets_materials" => Some(Self::Materials),
            "assets.textures" | "assets_textures" => Some(Self::Textures),
            "assets.definitions" | "assets_definitions" => Some(Self::Definitions),
            "assets.graph" | "assets_graph" | "assets-graph" => Some(Self::AssetGraph),
            "time" => Some(Self::Time),
            "schema" => Some(Self::Schema),
            "animation" => Some(Self::Animation),
            "navigation" => Some(Self::Navigation),
            "ai" => Some(Self::Ai),
            "tags" => Some(Self::Tags),
            "tasks" => Some(Self::Tasks),
            "scripting" => Some(Self::Scripting),
            "audio" => Some(Self::Audio),
            "render" => Some(Self::Render),
            "render.effects" | "render_effects" => Some(Self::RenderEffects),
            "render.materials" | "render_materials" => Some(Self::RenderMaterials),
            "assets.models" | "assets_models" => Some(Self::Model),
            "assets.models.skeletons" | "assets_models_skeletons" => Some(Self::ModelSkeletons),
            "assets.models.materials" | "assets_models_materials" => Some(Self::ModelMaterials),
            "assets.models.collisions" | "assets_models_collisions" => Some(Self::ModelCollisions),
            "camera" => Some(Self::Camera),
            "camera.modes" | "camera_modes" => Some(Self::CameraModes),
            "camera.animations" | "camera_animations" => Some(Self::CameraAnimations),
            "scene" => Some(Self::Scene),
            "physics" => Some(Self::Physics),
            "physics.contacts" | "physics_contacts" => Some(Self::PhysicsContacts),
            "physics.constraints" | "physics_constraints" => Some(Self::PhysicsConstraints),
            "input" => Some(Self::Input),
            "input.bindings" | "input_bindings" => Some(Self::InputBindings),
            "input.actions" | "input_actions" => Some(Self::InputActions),
            "input.contexts" | "input_contexts" => Some(Self::InputContexts),
            "ui" => Some(Self::Ui),
            "ui.text" | "ui_text" => Some(Self::UiText),
            "ui.debug" | "ui_debug" => Some(Self::UiDebug),
            "logging" => Some(Self::Logging),
            "loading" => Some(Self::Loading),
            "threading" => Some(Self::Threading),
            "platform" => Some(Self::Platform),
            "ecs" => Some(Self::Ecs),
            "entity" => Some(Self::Entity),
            "plugin_host" | "plugin-host" | "plugin.host" => Some(Self::PluginHost),
            "abi" => Some(Self::Abi),
            "gateway_registry" | "gateway-registry" | "gateway.registry" => {
                Some(Self::GatewayRegistry)
            }
            "security" => Some(Self::Security),
            "scheduler.core" | "scheduler_core" | "scheduler-core" => Some(Self::SchedulerCore),
            "capability_validator" | "capability-validator" | "capability.validator" => {
                Some(Self::CapabilityValidator)
            }
            _ => None,
        }
    }

    /// Returns the direct parent domain for third-level extension domains.
    ///
    /// Example: `input.bindings -> input`, `render.effects -> render`.
    #[inline]
    pub const fn parent(self) -> Option<Self> {
        match self {
            Self::AssetVfs
            | Self::AssetTypes
            | Self::AssetInspect
            | Self::AssetEdit
            | Self::AssetPackages
            | Self::AssetListFiles
            | Self::AssetUid
            | Self::AssetDependencies
            | Self::AssetImportQueue
            | Self::AssetPackageWriter
            | Self::AssetMaps
            | Self::AssetValidation
            | Self::AssetUi
            | Self::Materials
            | Self::Textures
            | Self::Definitions
            | Self::AssetGraph
            | Self::Model => Some(Self::Assets),
            Self::RenderEffects | Self::RenderMaterials => Some(Self::Render),
            Self::ModelSkeletons | Self::ModelMaterials | Self::ModelCollisions => {
                Some(Self::Model)
            }
            Self::CameraModes | Self::CameraAnimations => Some(Self::Camera),
            Self::PhysicsContacts | Self::PhysicsConstraints => Some(Self::Physics),
            Self::InputBindings | Self::InputActions | Self::InputContexts => Some(Self::Input),
            Self::UiText | Self::UiDebug => Some(Self::Ui),
            _ => None,
        }
    }

    #[inline]
    pub const fn root(self) -> Self {
        match self.parent() {
            Some(parent) => parent,
            None => self,
        }
    }

    #[inline]
    pub const fn domain_depth(self) -> u8 {
        match self.parent() {
            Some(_) => 3,
            None => 2,
        }
    }

    #[inline]
    pub const fn engine_gateway_id(self) -> &'static str {
        match self {
            Self::Assets => "engine.assets",
            Self::AssetVfs => "engine.assets.vfs",
            Self::AssetTypes => "engine.assets.types",
            Self::AssetInspect => "engine.assets.inspect",
            Self::AssetEdit => "engine.assets.edit",
            Self::AssetPackages => "engine.assets.packages",
            Self::AssetListFiles => "engine.assets.listfiles",
            Self::AssetUid => "engine.assets.uid",
            Self::AssetDependencies => "engine.assets.dependencies",
            Self::AssetImportQueue => "engine.assets.import_queue",
            Self::AssetPackageWriter => "engine.assets.package_writer",
            Self::AssetMaps => "engine.assets.maps",
            Self::AssetValidation => "engine.assets.validation",
            Self::AssetUi => "engine.assets.ui",
            Self::Materials => "engine.assets.materials",
            Self::Textures => "engine.assets.textures",
            Self::Definitions => "engine.assets.definitions",
            Self::AssetGraph => "engine.assets.graph",
            Self::Time => "engine.time",
            Self::Schema => "engine.schema",
            Self::Animation => "engine.animation",
            Self::Navigation => "engine.navigation",
            Self::Ai => "engine.ai",
            Self::Tags => "engine.tags",
            Self::Tasks => "engine.tasks",
            Self::Scripting => "engine.scripting",
            Self::Audio => "engine.audio",
            Self::Render => "engine.render",
            Self::RenderEffects => "engine.render.effects",
            Self::RenderMaterials => "engine.render.materials",
            Self::Model => "engine.assets.models",
            Self::ModelSkeletons => "engine.assets.models.skeletons",
            Self::ModelMaterials => "engine.assets.models.materials",
            Self::ModelCollisions => "engine.assets.models.collisions",
            Self::Camera => "engine.camera",
            Self::CameraModes => "engine.camera.modes",
            Self::CameraAnimations => "engine.camera.animations",
            Self::Scene => "engine.scene",
            Self::Physics => "engine.physics",
            Self::PhysicsContacts => "engine.physics.contacts",
            Self::PhysicsConstraints => "engine.physics.constraints",
            Self::Input => "engine.input",
            Self::InputBindings => "engine.input.bindings",
            Self::InputActions => "engine.input.actions",
            Self::InputContexts => "engine.input.contexts",
            Self::Ui => "engine.ui",
            Self::UiText => "engine.ui.text",
            Self::UiDebug => "engine.ui.debug",
            Self::Logging => "engine.logging",
            Self::Loading => "engine.ui.loading",
            Self::Threading => "engine.threading",
            Self::Platform => "engine.platform",
            Self::Ecs => "engine.ecs",
            Self::Entity => "engine.entity",
            Self::PluginHost => "engine.plugin_host",
            Self::Abi => "engine.abi",
            Self::GatewayRegistry => "engine.gateway_registry",
            Self::Security => "engine.security",
            Self::SchedulerCore => "engine.scheduler.core",
            Self::CapabilityValidator => "engine.capability_validator",
        }
    }

    #[inline]
    pub fn matches_engine_gateway_id(self, gateway_id: &str) -> bool {
        self.engine_gateway_id()
            == normalize_engine_gateway_id(gateway_id)
                .as_deref()
                .unwrap_or("")
    }

    #[inline]
    pub fn parse_engine_gateway_id(gateway_id: &str) -> Option<Self> {
        let normalized = normalize_engine_gateway_id(gateway_id)?;
        let domain = normalized.strip_prefix(ENGINE_SERVICE_GATEWAY_PREFIX)?;
        Self::parse(domain)
    }
}
