#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::{EntityId, World};
use newengine_transform::Transform;

/// Human-readable name of an entity.
#[derive(Clone, Debug)]
pub struct Name(pub String);

impl Name {
    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Runtime scene: an owned ECS `World` plus a few canonical entity handles.
///
/// The scene is renderer-agnostic and editor-agnostic.
pub struct Scene {
    world: World,
    active_camera: EntityId,
    root: EntityId,
}

impl Scene {
    /// Creates an empty scene with a root entity and an active camera.
    #[inline]
    pub fn new() -> Self {
        let mut world = World::new();
        let root = spawn_named(&mut world, "Root");
        let active_camera = spawn_named(&mut world, "Camera");
        Self {
            world,
            active_camera,
            root,
        }
    }

    /// A deterministic demo scene used by the editor while the pipeline is being built.
    #[inline]
    pub fn demo() -> Self {
        let mut s = Self::new();
        let _ = spawn_named(&mut s.world, "DirectionalLight");
        let cube = spawn_named(&mut s.world, "Cube");
        if let Some(t) = s.world.get_mut::<Transform>(cube) {
            t.position = glam::Vec3::new(0.0, 0.0, 0.0);
        }
        s
    }

    #[inline]
    pub fn world(&self) -> &World {
        &self.world
    }

    #[inline]
    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    #[inline]
    pub fn root(&self) -> EntityId {
        self.root
    }

    #[inline]
    pub fn active_camera(&self) -> EntityId {
        self.active_camera
    }

    #[inline]
    pub fn set_active_camera(&mut self, id: EntityId) -> bool {
        if !self.world.exists(id) {
            return false;
        }
        self.active_camera = id;
        true
    }
}

impl Default for Scene {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

/// Spawns an entity with `Name` and `Transform`.
#[inline]
pub fn spawn_named(world: &mut World, name: impl Into<String>) -> EntityId {
    let e = world.spawn();
    let _ = world.insert(e, Name(name.into()));
    let _ = world.insert(e, Transform::default());
    e
}

/// Attempts to read an entity name.
#[inline]
pub fn name_or<'a>(world: &'a World, id: EntityId, fallback: &'a str) -> &'a str {
    world.get::<Name>(id).map(|n| n.as_str()).unwrap_or(fallback)
}
