#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::{EntityId, World};

use crate::components::{ActiveCamera, SceneRoot};
use crate::settings::SceneSettings;
use crate::SceneState;

/// Runtime scene: owned ECS `World` + settings.
///
/// Entity roles are expressed via components (`SceneRoot`, `ActiveCamera`)
/// and cached in `SceneState` for strict invariants.
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
    pub fn root(&self) -> Option<EntityId> {
        self.world.resource::<SceneState>().and_then(|s| s.root)
    }

    #[inline]
    pub fn active_camera(&self) -> Option<EntityId> {
        self.world.resource::<SceneState>().and_then(|s| s.active_camera)
    }

    #[inline]
    pub fn set_active_camera(&mut self, id: EntityId) -> bool {
        if !self.world.exists(id) {
            return false;
        }

        if let Some(st) = self.world.resource::<SceneState>() {
            if let Some(prev) = st.active_camera {
                self.world.remove::<ActiveCamera>(prev);
            }
        }

        self.world.insert(id, ActiveCamera);

        if let Some(st) = self.world.resource_mut::<SceneState>() {
            st.active_camera = Some(id);
        }

        true
    }

    /// Ensures:
    /// - at most one `SceneRoot` marker
    /// - at most one `ActiveCamera` marker
    /// - `SceneState` matches markers
    ///
    /// Foundation-first rule: this method never spawns entities.
    /// Scene bootstrap (root/camera defaults) must live in higher layers (editor/game).
    pub fn validate_invariants(&mut self) -> bool {
        let mut changed = false;

        // ---- ROOT ----
        let mut roots: Vec<EntityId> = self
            .world
            .query::<SceneRoot>()
            .map(|(id, _)| id)
            .collect();

        let root: Option<EntityId> = match roots.len() {
            0 => None,
            1 => Some(roots[0]),
            _ => {
                roots.sort_unstable_by_key(|e| e.stable_u64());
                let keep = roots[0];
                for e in roots.iter().skip(1) {
                    self.world.remove::<SceneRoot>(*e);
                }
                changed = true;
                Some(keep)
            }
        };

        // ---- CAMERA ----
        let mut cams: Vec<EntityId> = self
            .world
            .query::<ActiveCamera>()
            .map(|(id, _)| id)
            .collect();

        let cam: Option<EntityId> = match cams.len() {
            0 => None,
            1 => Some(cams[0]),
            _ => {
                cams.sort_unstable_by_key(|e| e.stable_u64());
                let keep = cams[0];
                for e in cams.iter().skip(1) {
                    self.world.remove::<ActiveCamera>(*e);
                }
                changed = true;
                Some(keep)
            }
        };

        match self.world.resource_mut::<SceneState>() {
            Some(st) => {
                if st.root != root || st.active_camera != cam {
                    st.root = root;
                    st.active_camera = cam;
                    changed = true;
                }
            }
            None => {
                self.world.insert_resource(SceneState::new(root, cam));
                changed = true;
            }
        }

        changed
    }
}