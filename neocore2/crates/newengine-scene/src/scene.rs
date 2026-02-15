#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::{EntityId, World};
use newengine_transform::set_parent;

use crate::components::{ActiveCamera, SceneRoot};
use crate::settings::SceneSettings;
use crate::spawn::spawn_named;
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
        let mut world = World::new();

        let root = spawn_named(&mut world, "Root");
        world.insert(root, SceneRoot);

        let cam = spawn_named(&mut world, "Camera");
        world.insert(cam, ActiveCamera);

        set_parent(&mut world, cam, Some(root));

        world.insert_resource(SceneState::new(root, cam));

        Self {
            world,
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
        self.world.resource::<SceneState>().map(|s| s.root)
    }

    #[inline]
    pub fn active_camera(&self) -> Option<EntityId> {
        self.world.resource::<SceneState>().map(|s| s.active_camera)
    }

    #[inline]
    pub fn set_active_camera(&mut self, id: EntityId) -> bool {
        if !self.world.exists(id) {
            return false;
        }

        if let Some(st) = self.world.resource::<SceneState>().copied() {
            self.world.remove::<ActiveCamera>(st.active_camera);
        }

        self.world.insert(id, ActiveCamera);

        if let Some(st) = self.world.resource_mut::<SceneState>() {
            st.active_camera = id;
        }

        true
    }

    /// Ensures:
    /// - exactly one SceneRoot
    /// - exactly one ActiveCamera
    /// - SceneState matches markers
    pub fn validate_invariants(&mut self) -> bool {
        let mut changed = false;

        // ---- ROOT ----
        let mut roots: Vec<EntityId> =
            self.world.query::<SceneRoot>().map(|(id, _)| id).collect();

        let root = match roots.len() {
            1 => roots[0],
            0 => {
                let e = spawn_named(&mut self.world, "Root");
                self.world.insert(e, SceneRoot);
                changed = true;
                e
            }
            _ => {
                roots.sort_unstable_by_key(|e| e.stable_u64());
                let keep = roots[0];
                for e in roots.iter().skip(1) {
                    self.world.remove::<SceneRoot>(*e);
                }
                changed = true;
                keep
            }
        };

        // ---- CAMERA ----
        let mut cams: Vec<EntityId> =
            self.world.query::<ActiveCamera>().map(|(id, _)| id).collect();

        let cam = match cams.len() {
            1 => cams[0],
            0 => {
                let e = spawn_named(&mut self.world, "Camera");
                self.world.insert(e, ActiveCamera);
                set_parent(&mut self.world, e, Some(root));
                changed = true;
                e
            }
            _ => {
                cams.sort_unstable_by_key(|e| e.stable_u64());
                let keep = cams[0];
                for e in cams.iter().skip(1) {
                    self.world.remove::<ActiveCamera>(*e);
                }
                changed = true;
                keep
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