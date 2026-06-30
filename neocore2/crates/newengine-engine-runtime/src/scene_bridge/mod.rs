#![forbid(unsafe_op_in_unsafe_fn)]

mod accessors;
mod apply_commands;
mod commands;
mod commands_api;
mod definitions_runtime;
mod game_ready;
mod helpers;
mod imported_assets;
mod material_application;
mod queue;
mod view_gateway;

pub use commands::SceneCommand;
pub use definitions_runtime::{
    DefinitionInstance, DefinitionInstantiateTransform, DefinitionRuntimeTrace,
    DefinitionRuntimeTraceComponent, RuntimeCommand,
};
pub use imported_assets::{
    PrimitiveMaterialBase, SceneImportedAssetAssembler, SceneImportedAssetAssemblyDescriptor,
    SceneImportedAssetAssemblyKind, SceneImportedAssetDescriptor, SceneImportedAssetKind,
    SceneImportedAssetRepresentation,
};
pub(crate) use view_gateway::{
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

use crate::audio_gateway::register_audio_gateway_best_effort;
use crate::authority::RuntimeWorldAuthorityBridge;
use crate::camera_gateway::CameraGatewayBridge;
use crate::gameplay::{
    ensure_physics_body, remove_physics_body, spawn_default_player, DisplayMode, DisplayVisibility,
    GameRunMode,
};
use crate::scene_bootstrap::bootstrap_runtime_scene;

use game_ready::bootstrap_fps_game_ready_scene;
pub(crate) use game_ready::{
    tick_game_ready_sky_cycle, tick_game_ready_streaming_terrain, PreparedTerrainPrimitiveMesh,
    SkyClearColorRuntime, SkyDomeRuntime, TerrainSurfaceLayers,
};
use helpers::{
    apply_primitive_instance, effective_material_base, ensure_primitive_base, ensure_root,
    place_spawn_position, primitive_bounds, reset_game_runtime_state, restore_non_collision_bounds,
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
    selection_authority: Arc<Mutex<Option<newengine_entity_api::EntityHandle>>>,
    primitives: Arc<RwLock<PrimitiveRegistry>>,
    materials: Arc<RwLock<MaterialRegistry>>,
    asset_assemblers: Arc<RwLock<Vec<SceneImportedAssetAssembler>>>,
    camera_gateway: Arc<CameraGatewayBridge>,
    authority: Arc<RuntimeWorldAuthorityBridge>,
    play_mode: Arc<Mutex<GameRunMode>>,
}
impl SceneBridge {
    #[inline]
    pub fn new(mut initial: Scene) -> Self {
        bootstrap_runtime_scene(&mut initial);
        register_audio_gateway_best_effort();
        let primitives = Arc::new(RwLock::new(PrimitiveRegistry::with_builtins()));
        let materials = Arc::new(RwLock::new(MaterialRegistry::with_builtins()));

        // Product/profile scene assembly happens through profile-owned modules.
        // The reusable scene bridge starts in staging and exposes explicit hooks
        // for profiles that need late bootstrap after providers are routed.
        let (initial_selection, initial_mode) = (None, GameRunMode::Staging);

        Self {
            scene: Arc::new(RwLock::new(initial)),
            queue: Arc::new(Mutex::new(SceneQueue::default())),
            selection: Arc::new(Mutex::new(initial_selection)),
            selection_authority: Arc::new(Mutex::new(None)),
            primitives,
            materials,
            asset_assemblers: Arc::new(RwLock::new(builtin_asset_assemblers())),
            camera_gateway: Arc::new(CameraGatewayBridge::new()),
            authority: Arc::new(RuntimeWorldAuthorityBridge::new()),
            play_mode: Arc::new(Mutex::new(initial_mode)),
        }
    }

    #[inline]
    pub fn scene(&self) -> Arc<RwLock<Scene>> {
        Arc::clone(&self.scene)
    }

    #[inline]
    pub fn primitives(&self) -> Arc<RwLock<PrimitiveRegistry>> {
        Arc::clone(&self.primitives)
    }

    #[inline]
    pub fn materials(&self) -> Arc<RwLock<MaterialRegistry>> {
        Arc::clone(&self.materials)
    }

    pub fn bootstrap_profile_scene_now(&self) -> Option<EntityId> {
        let (selected, selected_authority) = {
            let mut scene = self.scene.write();
            let mut prims = self.primitives.write();
            let mats = self.materials.read();
            let selected = bootstrap_fps_game_ready_scene(&mut scene, &mut prims, &mats);
            let selected_authority = self.authority.declare_native_scene_cache(
                scene.world_mut(),
                "game-ready-scene-bootstrap",
                selected,
            );
            (selected, selected_authority)
        };

        *self.selection.lock() = selected;
        *self.selection_authority.lock() = selected_authority;
        self.authority
            .log_bootstrap_boundary("game-ready-scene-bootstrap");
        // Do not expose Play here. CPU scene bootstrap is not equivalent to
        // playable-world readiness; renderer-side launch gate owns promotion.
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
