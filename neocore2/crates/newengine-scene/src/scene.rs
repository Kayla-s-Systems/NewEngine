#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::{Component, EntityId, World};

use crate::components::{ActiveCamera, SceneRoot};
use crate::settings::SceneSettings;
use crate::SceneState;

/// Runtime scene: owned ECS `World` + settings.
///
/// Entity roles are expressed via components (`SceneRoot`, `ActiveCamera`)
/// and cached in `SceneState` for strict invariants.
pub struct Scene {
    pub(crate) world: World,
    pub(crate) settings: SceneSettings,
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

    /// Runs a deterministic scene frame with strict single-source-of-truth semantics.
    ///
    /// Pipeline:
    /// 1) `world.tick = frame_tick` (authoring/sim inputs)
    /// 2) `update_scene_world()` (derive bounds / world poses for controllers)
    /// 3) `world.tick++` (controller phase; camera/nav writes)
    /// 4) `controller(world)`
    /// 5) `update_scene_world()` again (derive outputs for rendering)
    ///
    /// Why this exists:
    /// - Avoids ad-hoc tick juggling in higher layers.
    /// - Preserves change-tracking invariants without hidden allocations.
    #[inline]
    pub fn run_frame<R, F: FnOnce(&mut World) -> R>(&mut self, frame_tick: u64, controller: F) -> R {
        self.world.set_tick(frame_tick);

        // Pre: derived state for controller logic (bounds/world pose queries).
        crate::update_scene_world(&mut self.world);

        // Separate phase so writes done by controllers are visible to the post update.
        self.world.advance_tick();

        let out = controller(&mut self.world);

        // Post: derived state for rendering/picking.
        crate::update_scene_world(&mut self.world);

        out
    }


    #[inline]
    fn reconcile_unique_marker<C: Component>(&mut self) -> (Option<EntityId>, bool) {
        // Pass 1: find the deterministic keeper (min stable_u64) and count.
        let mut keep: Option<EntityId> = None;
        let mut keep_key: u64 = 0;
        let mut count: usize = 0;

        for (id, _) in self.world.query::<C>() {
            count += 1;
            let k = id.stable_u64();
            match keep {
                None => {
                    keep = Some(id);
                    keep_key = k;
                }
                Some(_) => {
                    if k < keep_key {
                        keep = Some(id);
                        keep_key = k;
                    }
                }
            }
        }

        if count <= 1 {
            return (keep, false);
        }

        // Pass 2: remove all non-keepers.
        let Some(keep_id) = keep else {
            return (None, false);
        };
        let mut to_remove: Vec<EntityId> = Vec::with_capacity(count.saturating_sub(1));
        for (id, _) in self.world.query::<C>() {
            if id != keep_id {
                to_remove.push(id);
            }
        }
        for id in to_remove {
            let _ = self.world.remove::<C>(id);
        }

        (Some(keep_id), true)
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
        let (root, root_changed) = self.reconcile_unique_marker::<SceneRoot>();
        changed |= root_changed;

        // ---- CAMERA ----
        let (cam, cam_changed) = self.reconcile_unique_marker::<ActiveCamera>();
        changed |= cam_changed;

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