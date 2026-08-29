use super::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct GameReadySceneEntityLayout {
    pub(super) environment: EntityId,
    pub(super) terrain: EntityId,
    pub(super) foliage: EntityId,
    pub(super) definitions: EntityId,
    pub(super) actors: EntityId,
    pub(super) cameras: EntityId,
}

fn spawn_scene_layout_node(
    world: &mut newengine_ecs::World,
    parent: EntityId,
    name: &'static str,
    role: newengine_engine_runtime::gameplay::SceneEntityRole,
) -> EntityId {
    let entity = spawn_named(world, name);
    let _ = set_parent(world, entity, Some(parent));
    newengine_engine_runtime::gameplay::attach_scene_element_core(
        world,
        entity,
        role,
        name,
        Vec3::ZERO,
        Vec3::splat(0.25),
    );
    entity
}

pub(super) fn spawn_game_ready_scene_entity_layout(
    world: &mut newengine_ecs::World,
    root: EntityId,
) -> GameReadySceneEntityLayout {
    let layout = GameReadySceneEntityLayout {
        environment: spawn_scene_layout_node(
            world,
            root,
            "Scene/Environment",
            newengine_engine_runtime::gameplay::SceneEntityRole::Environment,
        ),
        terrain: spawn_scene_layout_node(
            world,
            root,
            "Scene/Terrain",
            newengine_engine_runtime::gameplay::SceneEntityRole::Terrain,
        ),
        foliage: spawn_scene_layout_node(
            world,
            root,
            "Scene/Foliage",
            newengine_engine_runtime::gameplay::SceneEntityRole::Foliage,
        ),
        definitions: spawn_scene_layout_node(
            world,
            root,
            "Scene/Definitions",
            newengine_engine_runtime::gameplay::SceneEntityRole::Definitions,
        ),
        actors: spawn_scene_layout_node(
            world,
            root,
            "Scene/Actors",
            newengine_engine_runtime::gameplay::SceneEntityRole::Actors,
        ),
        cameras: spawn_scene_layout_node(
            world,
            root,
            "Scene/Cameras",
            newengine_engine_runtime::gameplay::SceneEntityRole::Cameras,
        ),
    };
    newengine_ulog_api::ulog::info!(
        "game-ready scene layout: all authored scene elements are ordinary ECS entities environment={:?} terrain={:?} foliage={:?} definitions={:?} actors={:?} cameras={:?} policy='no special scene side-channel elements'",
        layout.environment,
        layout.terrain,
        layout.foliage,
        layout.definitions,
        layout.actors,
        layout.cameras
    );
    layout
}

pub(super) fn spawn_authored_terrain_reference(
    world: &mut newengine_ecs::World,
    parent: EntityId,
    spec: &GameReadyTerrainSpec,
) -> EntityId {
    let entity = spawn_named(world, "Scene/Terrain/AuthoredWorldReference");
    let _ = set_parent(world, entity, Some(parent));
    let _ = world.insert(
        entity,
        Transform {
            position: Vec3::new(0.0, spec.base_height, 0.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        },
    );
    newengine_ulog_api::ulog::info!(
        "game-ready terrain: procedural terrain disabled; authored world reference entity={:?} base_height={} policy='no default terrain mesh, no default terrain collider'",
        entity,
        spec.base_height
    );
    entity
}
