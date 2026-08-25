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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthoredMapPlacementSource {
    ProfilePrefab,
    DiscretePlacement,
}

/// Runtime-only authoring marker. It is attached only by editor mutations and
/// is cleared after a successful project save. Simulation must never set it.
#[derive(Clone, Copy, Debug, Default)]
pub struct AuthoredMapPlacementDirty;

/// Marks a live actor as a newly-created authored placement cloned from an
/// existing source element. It survives until the first successful project save,
/// after which the new placement becomes canonical and this marker is removed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthoredMapPlacementCloneSource {
    pub placement_id: String,
}

impl AuthoredMapPlacementCloneSource {
    #[inline]
    pub fn new(placement_id: impl Into<String>) -> Self {
        Self {
            placement_id: placement_id.into(),
        }
    }
}

/// Runtime-only scale state for a derived collider/replica that shares an authored
/// placement with a primary visual actor. Static mesh collider scale is baked into
/// vertices, so the editor uses this state to apply incremental authored scale deltas.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthoredMapPlacementReplicaScaleState {
    pub last_authored_scale: Vec3,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthoredMapPlacement {
    pub map_ref: String,
    pub placement_id: String,
    pub source: AuthoredMapPlacementSource,
    /// True for the actor that owns authoring. False for runtime replicas such as
    /// a collision companion generated from the same authored placement.
    pub primary: bool,
}

impl AuthoredMapPlacement {
    pub fn new(
        map_ref: impl Into<String>,
        placement_id: impl Into<String>,
        source: AuthoredMapPlacementSource,
        primary: bool,
    ) -> Self {
        Self {
            map_ref: map_ref.into(),
            placement_id: placement_id.into(),
            source,
            primary,
        }
    }
}

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
