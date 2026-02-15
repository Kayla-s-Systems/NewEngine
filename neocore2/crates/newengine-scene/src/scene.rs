#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::{EntityId, World};
use newengine_transform::set_parent;

use crate::components::{ActiveCamera, SceneRoot};
use crate::settings::SceneSettings;
use crate::spawn::spawn_named;

/// Runtime scene: owned ECS `World` + settings.
///
/// Entity roles are expressed via components (`SceneRoot`, `ActiveCamera`).
pub struct Scene {
    world: World,
    settings: SceneSettings,
}

impl Default for Scene {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Scene {
    /// Creates an empty scene with a root entity and an active camera entity.
    #[inline]
    pub fn new() -> Self {
        let mut world = World::new();

        let root = spawn_named(&mut world, "Root");
        let _ = world.insert(root, SceneRoot);

        let cam = spawn_named(&mut world, "Camera");
        let _ = world.insert(cam, ActiveCamera);

        let _ = set_parent(&mut world, cam, Some(root));

        Self {
            world,
            settings: SceneSettings::default(),
        }
    }

    #[inline]
    pub fn settings(&self) -> SceneSettings {
        self.settings
    }

    #[inline]
    pub fn settings_mut(&mut self) -> &mut SceneSettings {
        &mut self.settings
    }

    #[inline]
    pub fn world(&self) -> &World {
        &self.world
    }

    #[inline]
    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    /// Returns the current root entity (first found).
    #[inline]
    pub fn root(&self) -> Option<EntityId> {
        self.world.query::<SceneRoot>().map(|(id, _)| id).next()
    }

    /// Returns the active camera entity (first found).
    #[inline]
    pub fn active_camera(&self) -> Option<EntityId> {
        self.world.query::<ActiveCamera>().map(|(id, _)| id).next()
    }

    #[inline]
    pub fn set_active_camera(&mut self, id: EntityId) -> bool {
        if !self.world.exists(id) {
            return false;
        }

        let old: Vec<EntityId> = self
            .world
            .query::<ActiveCamera>()
            .map(|(eid, _)| eid)
            .collect();

        for e in old {
            let _ = self.world.remove::<ActiveCamera>(e);
        }

        let _ = self.world.insert(id, ActiveCamera);
        true
    }
}