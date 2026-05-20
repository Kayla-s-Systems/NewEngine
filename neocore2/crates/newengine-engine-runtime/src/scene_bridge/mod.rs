#![forbid(unsafe_op_in_unsafe_fn)]

mod commands;
mod game_ready;
mod helpers;
mod imported_assets;
mod material_application;
mod queue;
mod accessors;
mod commands_api;
mod apply_commands;
mod view_gateway;

pub use commands::SceneCommand;
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
use crate::camera_gateway::CameraGatewayBridge;
use crate::gameplay::{
    ensure_physics_body, remove_physics_body, spawn_default_player, DisplayMode,
    DisplayVisibility, GameRunMode,
};
use crate::scene_bootstrap::bootstrap_runtime_scene;

use game_ready::{bootstrap_fps_game_ready_scene, game_ready_demo_enabled};
pub(crate) use game_ready::{
    tick_game_ready_sky_cycle, tick_game_ready_streaming_terrain, PreparedTerrainPrimitiveMesh,
    SkyDomeRuntime, TerrainSurfaceLayers,
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
    primitives: Arc<RwLock<PrimitiveRegistry>>,
    materials: Arc<RwLock<MaterialRegistry>>,
    asset_assemblers: Arc<RwLock<Vec<SceneImportedAssetAssembler>>>,
    camera_gateway: Arc<CameraGatewayBridge>,
    play_mode: Arc<Mutex<GameRunMode>>,
}
impl SceneBridge {
    #[inline]
    pub fn new(mut initial: Scene) -> Self {
        bootstrap_runtime_scene(&mut initial);
        register_audio_gateway_best_effort();
        let primitives = Arc::new(RwLock::new(PrimitiveRegistry::with_builtins()));
        let materials = Arc::new(RwLock::new(MaterialRegistry::with_builtins()));

        // Game-ready scenes must be assembled only after engine plugins are loaded:
        // AssetManager discovers geometryImporter during the engine plugin phase.
        // The standalone game profile owns that late bootstrap module.
        let (initial_selection, initial_mode) = if game_ready_demo_enabled() {
            // Standalone game-ready scenes start in a non-playable staging mode.
            // The render controller promotes the bridge to Play only after the
            // scene launch gate verifies CPU scene assembly and GPU residency.
            (None, GameRunMode::Staging)
        } else {
            (None, GameRunMode::Staging)
        };

        Self {
            scene: Arc::new(RwLock::new(initial)),
            queue: Arc::new(Mutex::new(SceneQueue::default())),
            selection: Arc::new(Mutex::new(initial_selection)),
            primitives,
            materials,
            asset_assemblers: Arc::new(RwLock::new(builtin_asset_assemblers())),
            camera_gateway: Arc::new(CameraGatewayBridge::new()),
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

    pub fn bootstrap_game_ready_scene_now(&self) -> Option<EntityId> {
        if !game_ready_demo_enabled() {
            return None;
        }

        let selected = {
            let mut scene = self.scene.write();
            let mut prims = self.primitives.write();
            let mats = self.materials.read();
            bootstrap_fps_game_ready_scene(&mut scene, &mut *prims, &*mats)
        };

        *self.selection.lock() = selected;
        // Do not expose Play here. CPU scene bootstrap is not equivalent to
        // playable-world readiness; renderer-side launch gate owns promotion.
        *self.play_mode.lock() = GameRunMode::Staging;
        selected
    }

    #[inline]
    pub fn activate_game_ready_play_now(&self) {
        if !game_ready_demo_enabled() {
            return;
        }
        let mut play_mode = self.play_mode.lock();
        if !play_mode.is_runtime() {
            *play_mode = GameRunMode::Play;
            log::info!(
                "game-ready runtime: public play mode activated after scene launch gate release"
            );
        }
    }
}
