use super::*;

#[derive(Clone, Copy, Debug, Default)]
pub struct PlayerActor;

#[derive(Clone, Copy, Debug, Default)]
pub struct GameplayActor;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneEntityRole {
    Environment,
    Sun,
    SkyCycle,
    SkyDome,
    Terrain,
    TerrainStreamingAnchor,
    Foliage,
    Definitions,
    Actors,
    Cameras,
    Player,
    ActiveCamera,
}

pub use newengine_world_authoring_api::{
    AuthoredMapPlacement, AuthoredMapPlacementCloneSource, AuthoredMapPlacementDirty,
    AuthoredMapPlacementReplicaScaleState, AuthoredMapPlacementSource,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneEntityAnchor {
    pub role: SceneEntityRole,
    pub label: &'static str,
}

impl SceneEntityAnchor {
    #[inline]
    pub const fn new(role: SceneEntityRole, label: &'static str) -> Self {
        Self { role, label }
    }
}

#[inline]
pub fn scene_entity_by_role(
    world: &newengine_ecs::World,
    role: SceneEntityRole,
) -> Option<newengine_ecs::EntityId> {
    world
        .query::<SceneEntityAnchor>()
        .find_map(|(entity, anchor)| (anchor.role == role).then_some(entity))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneAnchorFollow {
    pub enabled: bool,
    pub target: Option<EntityId>,
}

impl SceneAnchorFollow {
    #[inline]
    pub const fn player() -> Self {
        Self {
            enabled: true,
            target: None,
        }
    }
}

#[inline]
pub fn attach_scene_object_core(
    world: &mut newengine_ecs::World,
    entity: EntityId,
    position: Vec3,
    half_extents: Vec3,
) {
    if world.get::<Transform>(entity).is_none() {
        let _ = world.insert(
            entity,
            Transform {
                position,
                ..Transform::default()
            },
        );
    }
    if world.get::<Bounds>(entity).is_none() {
        let he = sanitized_half_extents(half_extents);
        let _ = world.insert(
            entity,
            Bounds::from_local_aabb(Aabb::from_center_half_extents(Vec3::ZERO, he)),
        );
    }
}

#[inline]
pub fn attach_scene_element_core(
    world: &mut newengine_ecs::World,
    entity: EntityId,
    role: SceneEntityRole,
    label: &'static str,
    position: Vec3,
    half_extents: Vec3,
) {
    attach_scene_object_core(world, entity, position, half_extents);
    let _ = world.insert(entity, SceneEntityAnchor::new(role, label));
}

#[inline]
fn sanitized_half_extents(value: Vec3) -> Vec3 {
    Vec3::new(
        value.x.abs().max(0.001),
        value.y.abs().max(0.001),
        value.z.abs().max(0.001),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_object_core_does_not_imply_physics() {
        let mut world = newengine_ecs::World::new();
        let entity = world.spawn();
        attach_scene_object_core(
            &mut world,
            entity,
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::splat(0.5),
        );

        assert!(world.get::<Transform>(entity).is_some());
        assert!(world.get::<Bounds>(entity).is_some());
        assert!(
            world.get::<PhysicsBodyDesc>(entity).is_none(),
            "scene membership must not manufacture collision; physics is explicit opt-in"
        );
    }
}
