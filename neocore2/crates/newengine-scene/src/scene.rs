#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::{EntityId, World};

use crate::settings::SceneSettings;
use crate::SceneState;

/// Runtime scene: owned ECS `World` + settings.
///
/// This crate must stay "foundation-first":
/// - no editor/demo bootstraps in `Scene::new()`
/// - scene roles are optional and provided by higher-level layers (editor/game)
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
    /// Creates an empty scene (no implicit entities).
    #[inline]
    pub fn new() -> Self {
        Self {
            world: World::new(),
            settings: SceneSettings::default(),
        }
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
    pub fn settings(&self) -> &SceneSettings {
        &self.settings
    }

    #[inline]
    pub fn settings_mut(&mut self) -> &mut SceneSettings {
        &mut self.settings
    }

    /// Returns strong scene invariants if configured by a higher layer.
    #[inline]
    pub fn state(&self) -> Option<&SceneState> {
        self.world.resource::<SceneState>()
    }

    /// Installs/overwrites the scene invariants resource.
    #[inline]
    pub fn set_state(&mut self, state: SceneState) {
        self.world.insert_resource(state);
    }

    #[inline]
    pub fn root(&self) -> Option<EntityId> {
        self.state().map(|s| s.root)
    }

    #[inline]
    pub fn active_camera(&self) -> Option<EntityId> {
        self.state().map(|s| s.active_camera)
    }

    #[inline]
    pub fn set_active_camera(&mut self, id: EntityId) -> bool {
        if !self.world.exists(id) {
            return false;
        }

        let Some(s) = self.world.resource_mut::<SceneState>() else {
            return false;
        };

        s.active_camera = id;
        true
    }
}