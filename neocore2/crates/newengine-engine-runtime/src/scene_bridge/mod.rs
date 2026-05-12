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

pub use commands::SceneCommand;
pub use imported_assets::{
    PrimitiveMaterialBase, SceneImportedAssetAssembler, SceneImportedAssetAssemblyDescriptor,
    SceneImportedAssetAssemblyKind, SceneImportedAssetDescriptor, SceneImportedAssetKind,
    SceneImportedAssetRepresentation,
};

use parking_lot::{Mutex, RwLock};
use std::sync::Arc;

use newengine_ecs::EntityId;
use newengine_lighting::{AmbientLight, DirectionalLight, PointLight};
use newengine_materials::api::MaterialRegistryApi;
use newengine_materials::{MaterialDescriptor, MaterialId, MaterialRef, MaterialRegistry};
use newengine_math::{EulerRot, Quat, Vec3};
use newengine_primitives::{builtins, Primitive, PrimitiveId, PrimitiveRegistry};
use newengine_scene::{spawn_named, Scene, SceneAsset};
use newengine_transform::Transform;

use crate::gameplay::{
    ensure_collision_body, remove_collision_body, spawn_default_player, CollisionBody, DisplayMode,
    DisplayVisibility, EditorPlayMode,
};
use crate::scene_bootstrap::bootstrap_runtime_scene;

use game_ready::{bootstrap_fps_game_ready_scene, game_ready_demo_enabled};
use helpers::{
    apply_primitive_instance, effective_material_base, ensure_primitive_base, ensure_root,
    place_spawn_position, primitive_bounds, reset_editor_runtime_state, restore_non_collision_bounds,
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
    play_mode: Arc<Mutex<EditorPlayMode>>,
    collision_wireframe: Arc<Mutex<bool>>,
}
impl SceneBridge {
    #[inline]
    pub fn new(mut initial: Scene) -> Self {
        bootstrap_runtime_scene(&mut initial);

        let primitives = Arc::new(RwLock::new(PrimitiveRegistry::with_builtins()));
        let materials = Arc::new(RwLock::new(MaterialRegistry::with_builtins()));

        // Game-ready scenes must be assembled only after engine plugins are loaded:
        // AssetManager discovers geometryImporter during the engine plugin phase.
        // The standalone game profile owns that late bootstrap module.
        let (initial_selection, initial_mode, initial_wire) = if game_ready_demo_enabled() {
            // Standalone game-ready scenes start in a non-playable staging mode.
            // The render controller promotes the bridge to Play only after the
            // scene launch gate verifies CPU scene assembly and GPU residency.
            (None, EditorPlayMode::Edit, false)
        } else {
            (None, EditorPlayMode::Edit, true)
        };

        Self {
            scene: Arc::new(RwLock::new(initial)),
            queue: Arc::new(Mutex::new(SceneQueue::default())),
            selection: Arc::new(Mutex::new(initial_selection)),
            primitives,
            materials,
            asset_assemblers: Arc::new(RwLock::new(builtin_asset_assemblers())),
            play_mode: Arc::new(Mutex::new(initial_mode)),
            collision_wireframe: Arc::new(Mutex::new(initial_wire)),
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
        *self.play_mode.lock() = EditorPlayMode::Edit;
        *self.collision_wireframe.lock() = false;
        selected
    }

    #[inline]
    pub fn activate_game_ready_play_now(&self) {
        if !game_ready_demo_enabled() {
            return;
        }
        let mut play_mode = self.play_mode.lock();
        if !play_mode.is_runtime() {
            *play_mode = EditorPlayMode::Play;
            log::info!(
                "game-ready runtime: public play mode activated after scene launch gate release"
            );
        }
        *self.collision_wireframe.lock() = false;
    }
}
