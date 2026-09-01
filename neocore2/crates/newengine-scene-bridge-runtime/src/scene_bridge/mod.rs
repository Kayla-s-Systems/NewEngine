#![forbid(unsafe_op_in_unsafe_fn)]

mod accessors;
mod apply_commands;
mod authoring_adapter;
mod bootstrap_provider;
mod commands;
mod commands_api;
mod definitions_runtime;
mod helpers;
mod imported_assets;
mod material_application;
mod queue;
mod scene_object_validation;
mod view_gateway;
mod weapon_grip_authoring;

pub use bootstrap_provider::{SceneBootstrapContext, SceneBootstrapProvider, SceneBootstrapResult};
pub use commands::SceneCommand;
pub(crate) use scene_object_validation::{
    scene_object_invariant_snapshot_json, validate_scene_object_invariants,
};

pub(crate) use definitions_runtime::apply_definition_instantiation;
pub use definitions_runtime::{
    DefinitionInstance, DefinitionInstantiateTransform, DefinitionRuntimeTrace,
    DefinitionRuntimeTraceComponent, RuntimeCommand,
};
pub use imported_assets::{
    PrimitiveMaterialBase, SceneImportedAssetAssembler, SceneImportedAssetAssemblyDescriptor,
    SceneImportedAssetAssemblyKind, SceneImportedAssetDescriptor, SceneImportedAssetKind,
    SceneImportedAssetRepresentation,
};
pub use view_gateway::{
    apply_engine_view_postfx, EngineViewDiagnostics, EngineViewGatewayFrame, EngineViewInput,
    EngineViewTransitionPhase,
};

use parking_lot::{Mutex, RwLock};
use std::sync::Arc;

use newengine_ecs::EntityId;
use newengine_lighting::{AmbientLight, DirectionalLight, PointLight};
use newengine_materials::api::MaterialRegistryApi;
use newengine_materials::{MaterialDescriptor, MaterialId, MaterialRef, MaterialRegistry};
use newengine_math::{EulerRot, Quat, Vec3};
use newengine_physics_contracts::PhysicsBodyDesc;
use newengine_primitives::{builtins, Primitive, PrimitiveId, PrimitiveRegistry};
use newengine_scene::{spawn_named, Scene, SceneAsset};
use newengine_transform::Transform;

use crate::scene_bootstrap::bootstrap_runtime_scene;
use newengine_camera_gateway_runtime::CameraGatewayBridge;
use newengine_gameplay_world_runtime::gameplay::{
    ensure_physics_body, remove_physics_body, spawn_default_player, DisplayMode, DisplayVisibility,
    GameRunMode,
};
use newengine_world_authority_runtime::RuntimeWorldAuthorityBridge;

pub(crate) use helpers::{
    apply_exact_material, apply_primitive_instance, ensure_primitive_base, ensure_root,
    primitive_bounds,
};
use helpers::{
    effective_material_base, place_spawn_position, reset_game_runtime_state,
    restore_non_collision_bounds,
};
use imported_assets::{
    builtin_asset_assemblers, imported_asset_collision, imported_asset_primitive_id,
    resolve_asset_assembler,
};
use queue::SceneQueue;

#[derive(Clone)]
pub struct SceneBridge {
    scene: Arc<RwLock<Scene>>,
    queue: Arc<Mutex<SceneQueue>>,
    selection: Arc<Mutex<Option<EntityId>>>,
    selection_set: Arc<Mutex<Vec<EntityId>>>,
    selection_authority: Arc<Mutex<Option<newengine_entity_api::EntityHandle>>>,
    primitives: Arc<RwLock<PrimitiveRegistry>>,
    materials: Arc<RwLock<MaterialRegistry>>,
    asset_assemblers: Arc<RwLock<Vec<SceneImportedAssetAssembler>>>,
    camera_gateway: Arc<CameraGatewayBridge>,
    authority: Arc<RuntimeWorldAuthorityBridge>,
    play_mode: Arc<Mutex<GameRunMode>>,
    authoring: Arc<RwLock<Option<Arc<dyn newengine_scene_authoring_api::SceneAuthoringService>>>>,
    scene_bootstrap_provider: Arc<RwLock<Option<Arc<dyn SceneBootstrapProvider>>>>,
}
impl SceneBridge {
    #[inline]
    pub fn new(mut initial: Scene) -> Self {
        bootstrap_runtime_scene(&mut initial);
        let primitives = Arc::new(RwLock::new(PrimitiveRegistry::with_builtins()));
        let materials = Arc::new(RwLock::new(MaterialRegistry::with_builtins()));

        // Product/profile scene assembly happens through profile-owned modules.
        // The reusable scene bridge starts in staging and exposes explicit hooks
        // for profiles that need late bootstrap after providers are routed.
        let initial_mode = GameRunMode::Staging;

        Self {
            scene: Arc::new(RwLock::new(initial)),
            queue: Arc::new(Mutex::new(SceneQueue::default())),
            selection: Arc::new(Mutex::new(None)),
            selection_set: Arc::new(Mutex::new(Vec::new())),
            selection_authority: Arc::new(Mutex::new(None)),
            primitives,
            materials,
            asset_assemblers: Arc::new(RwLock::new(builtin_asset_assemblers())),
            camera_gateway: Arc::new(CameraGatewayBridge::new()),
            authority: Arc::new(RuntimeWorldAuthorityBridge::new()),
            play_mode: Arc::new(Mutex::new(initial_mode)),
            authoring: Arc::new(RwLock::new(None)),
            scene_bootstrap_provider: Arc::new(RwLock::new(None)),
        }
    }

    #[inline]
    pub fn set_scene_authoring_provider(
        &self,
        provider: Arc<dyn newengine_scene_authoring_api::SceneAuthoringService>,
    ) {
        *self.authoring.write() = Some(provider);
    }

    #[inline]
    pub fn clear_scene_authoring_provider(&self) {
        *self.authoring.write() = None;
    }

    #[inline]
    pub fn scene_authoring_available(&self) -> bool {
        self.authoring.read().is_some()
    }

    #[inline]
    fn scene_authoring_provider(
        &self,
    ) -> Option<Arc<dyn newengine_scene_authoring_api::SceneAuthoringService>> {
        self.authoring.read().clone()
    }

    #[inline]
    pub fn scene(&self) -> Arc<RwLock<Scene>> {
        Arc::clone(&self.scene)
    }

    /// Publishes the scene-owned authoritative camera bridge into the active HostContext.
    #[inline]
    pub fn publish_camera_gateway_best_effort(&self) -> bool {
        self.camera_gateway.publish_gateway_best_effort()
    }

    #[inline]
    pub fn primitives(&self) -> Arc<RwLock<PrimitiveRegistry>> {
        Arc::clone(&self.primitives)
    }

    #[inline]
    pub fn materials(&self) -> Arc<RwLock<MaterialRegistry>> {
        Arc::clone(&self.materials)
    }

    pub fn scene_object_invariants_snapshot_json(&self) -> serde_json::Value {
        let scene = self.scene.read();
        scene_object_invariant_snapshot_json(scene.world())
    }

    pub fn set_scene_bootstrap_provider(&self, provider: Arc<dyn SceneBootstrapProvider>) {
        let descriptor = provider.descriptor();
        if let Err(error) = newengine_runtime_provider_api::validate_provider_contract(
            descriptor,
            newengine_runtime_provider_api::I_SCENE_BOOTSTRAP_PROVIDER_V1,
            newengine_runtime_provider_api::PROVIDER_CONTRACT_V1,
        ) {
            newengine_ulog_api::ulog::warn!("scene bootstrap provider rejected: {}", error);
            return;
        }
        *self.scene_bootstrap_provider.write() = Some(provider);
    }

    pub fn clear_scene_bootstrap_provider(&self) {
        *self.scene_bootstrap_provider.write() = None;
    }

    pub fn bootstrap_profile_scene_now(&self) -> Option<EntityId> {
        let provider = self.scene_bootstrap_provider.read().clone();
        let Some(provider) = provider else {
            newengine_ulog_api::ulog::error!(
                "scene bootstrap: no SceneBootstrapProvider registered; profile scene assembly skipped"
            );
            return None;
        };
        let provider_id = provider.id();
        let (selected, selected_authority) = {
            let mut scene = self.scene.write();
            let mut prims = self.primitives.write();
            let mats = self.materials.read();
            let mut ctx = SceneBootstrapContext {
                scene: &mut scene,
                primitives: &mut prims,
                materials: &mats,
            };
            let result = match provider.bootstrap(&mut ctx) {
                Ok(result) => result,
                Err(error) => {
                    newengine_ulog_api::ulog::error!(
                        "scene bootstrap provider failed provider='{}' err='{}'",
                        provider_id,
                        error
                    );
                    return None;
                }
            };
            let selected = result.primary_entity;
            let selected_authority =
                self.authority
                    .declare_native_scene_cache(scene.world_mut(), provider_id, selected);
            (selected, selected_authority)
        };

        *self.selection.lock() = selected;
        *self.selection_set.lock() = selected.into_iter().collect();
        *self.selection_authority.lock() = selected_authority;
        self.authority.log_bootstrap_boundary(provider_id);
        // CPU scene assembly is not equivalent to playable-world readiness.
        // Generic activation state/policy owns promotion to public Play.
        *self.play_mode.lock() = GameRunMode::Staging;
        selected
    }

    #[inline]
    pub fn activate_profile_play_now(&self) {
        let mut play_mode = self.play_mode.lock();
        if !play_mode.is_runtime() {
            *play_mode = GameRunMode::Play;
            newengine_ulog_api::ulog::info!(
                "game-ready runtime: public play mode activated after scene launch gate release"
            );
        }
    }
}
