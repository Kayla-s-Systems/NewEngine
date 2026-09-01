#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::EntityId;
use newengine_materials::{MaterialDescriptor, MaterialId};
use newengine_primitives::PrimitiveId;
use newengine_scene::SceneAsset;

use newengine_gameplay_world_runtime::gameplay::{DisplayMode, GameRunMode};
use newengine_physics_contracts::PhysicsBodyDesc;

use super::imported_assets::SceneImportedAssetDescriptor;

#[derive(Clone, Debug)]
pub enum SceneCommand {
    NewScene,
    LoadSceneAsset {
        asset: SceneAsset,
    },

    SpawnPrimitive {
        id: PrimitiveId,
        name: String,
        position: [f32; 3],
        scale: [f32; 3],
        color: [f32; 4],
    },
    SpawnDirectionalLight {
        name: String,
        position: [f32; 3],
        direction_ws: [f32; 3],
    },
    SpawnPointLight {
        name: String,
        position: [f32; 3],
    },
    SpawnPlayer {
        name: String,
        position: [f32; 3],
    },
    SpawnImportedAsset {
        descriptor: SceneImportedAssetDescriptor,
        name: String,
        position: [f32; 3],
    },

    InstantiateDefinition {
        definition_ref: String,
        position: [f32; 3],
        rotation_ypr: [f32; 3],
        scale: [f32; 3],
    },

    SetTransform {
        entity: EntityId,
        position: [f32; 3],
        rotation_ypr: [f32; 3],
        scale: [f32; 3],
    },
    SetPrimitiveColor {
        entity: EntityId,
        color: [f32; 4],
    },
    SetMaterial {
        entity: EntityId,
        material: MaterialId,
    },
    UpdateMaterial {
        material: MaterialId,
        desc: MaterialDescriptor,
    },

    SetAmbientLight {
        color: [f32; 3],
        intensity: f32,
    },
    SetDirectionalLight {
        entity: EntityId,
        direction_ws: [f32; 3],
        color: [f32; 3],
        intensity: f32,
    },
    SetPointLight {
        entity: EntityId,
        color: [f32; 3],
        intensity: f32,
        range: f32,
    },

    SetPhysicsBody {
        entity: EntityId,
        body: PhysicsBodyDesc,
    },
    ClearPhysicsBody {
        entity: EntityId,
    },
    SetDisplayVisibility {
        entity: EntityId,
        mode: DisplayMode,
    },
    SetParent {
        child: EntityId,
        parent: Option<EntityId>,
    },

    SetPlayMode {
        mode: GameRunMode,
    },
}
