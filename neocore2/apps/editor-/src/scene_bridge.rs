#![forbid(unsafe_op_in_unsafe_fn)]

use parking_lot::{Mutex, RwLock};
use std::sync::Arc;

use glam::Vec3;

use newengine_ecs::EntityId;
use newengine_primitives::{builtins, Primitive, PrimitiveId, PrimitiveRegistry};
use newengine_scene::{spawn_named, Scene};
use newengine_transform::Transform;

use crate::scene_bootstrap::bootstrap_editor_scene;

/// Scene commands produced by UI/editor tools and consumed by the render/controller side.
///
/// We keep this queue deterministic and explicit: no hidden async, no world access from UI.
#[derive(Clone, Debug)]
pub enum SceneCommand {
    /// Replace the current scene with a fresh one (root + camera).
    NewScene,
    SpawnPrimitive {
        id: PrimitiveId,
        name: String,
        position: [f32; 3],
        scale: [f32; 3],
        color: [f32; 4],
    },
}

#[derive(Default)]
struct SceneQueue {
    cmds: Vec<SceneCommand>,
}

/// Thread-safe bridge between UI and the scene world.
///
/// - UI pushes commands (no world mutation).
/// - Render/controller applies commands once per frame.
#[derive(Clone)]
pub struct SceneBridge {
    scene: Arc<RwLock<Scene>>,
    queue: Arc<Mutex<SceneQueue>>,
    selection: Arc<Mutex<Option<EntityId>>>,
    primitives: Arc<RwLock<PrimitiveRegistry>>,
}

impl SceneBridge {
    #[inline]
    pub fn new(mut initial: Scene) -> Self {
        bootstrap_editor_scene(&mut initial);

        Self {
            scene: Arc::new(RwLock::new(initial)),
            queue: Arc::new(Mutex::new(SceneQueue::default())),
            selection: Arc::new(Mutex::new(None)),
            primitives: Arc::new(RwLock::new(PrimitiveRegistry::with_builtins())),
        }
    }

    #[inline]
    pub fn primitives(&self) -> Arc<RwLock<PrimitiveRegistry>> {
        Arc::clone(&self.primitives)
    }

    /// Snapshot primitives for UI.
    ///
    /// Returns sorted (name, id) pairs.
    #[inline]
    pub fn primitives_snapshot(&self) -> Vec<(String, PrimitiveId)> {
        let reg = self.primitives.read();
        let mut out: Vec<(String, PrimitiveId)> = reg
            .ids()
            .filter_map(|id| reg.name(id).map(|n| (n.to_string(), id)))
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    #[inline]
    pub fn scene(&self) -> Arc<RwLock<Scene>> {
        Arc::clone(&self.scene)
    }

    #[inline]
    pub fn selection(&self) -> Option<EntityId> {
        *self.selection.lock()
    }

    #[inline]
    pub fn set_selection(&self, id: Option<EntityId>) {
        *self.selection.lock() = id;
    }

    #[inline]
    pub fn cmd_new_scene(&self) {
        self.queue.lock().cmds.push(SceneCommand::NewScene);
    }

    #[inline]
    pub fn cmd_spawn_primitive(&self, id: PrimitiveId, name: String, position: Vec3) {
        // Default scale presets for common built-ins.
        let scale = if id == builtins::ID_PLANE {
            [10.0, 1.0, 10.0]
        } else {
            [1.0, 1.0, 1.0]
        };

        let color = if id == builtins::ID_PLANE {
            [0.35, 0.35, 0.38, 1.0]
        } else {
            [0.85, 0.85, 0.9, 1.0]
        };

        self.queue.lock().cmds.push(SceneCommand::SpawnPrimitive {
            id,
            name,
            position: [position.x, position.y, position.z],
            scale,
            color,
        });
    }

    /// Applies queued commands to the scene world.
    ///
    /// Call from the render/controller thread once per frame.
    pub fn apply_commands(&self) {
        let cmds = {
            let mut q = self.queue.lock();
            if q.cmds.is_empty() {
                return;
            }
            std::mem::take(&mut q.cmds)
        };

        // Defer selection until after we release the scene write lock.
        let mut pending_selection: Option<Option<EntityId>> = None;

        {
            let mut scene = self.scene.write();

            for cmd in cmds {
                match cmd {
                    SceneCommand::NewScene => {
                        *scene = Scene::new();
                        bootstrap_editor_scene(&mut *scene);
                        pending_selection = Some(scene.active_camera());
                    }

                    SceneCommand::SpawnPrimitive {
                        id,
                        name,
                        position,
                        scale,
                        color,
                    } => {
                        // 1) Take immutable data from scene BEFORE world_mut().
                        let root_opt = scene.root();

                        // 2) Now take world_mut (locks mutable borrow of scene).
                        let world = scene.world_mut();

                        // 3) Resolve root using world only.
                        let root = root_opt.unwrap_or_else(|| world.spawn());

                        let e = spawn_named(world, name);
                        let _ = newengine_transform::set_parent(world, e, Some(root));

                        let _ = world.insert(
                            e,
                            Primitive {
                                id,
                                color,
                            },
                        );

                        if let Some(t) = world.get_mut_tracked::<Transform>(e) {
                            t.position = Vec3::new(position[0], position[1], position[2]);
                            t.scale = Vec3::new(scale[0], scale[1], scale[2]);
                        }

                        pending_selection = Some(Some(e));
                    }
                }
            }
        } // scene write lock dropped here

        if let Some(sel) = pending_selection {
            self.set_selection(sel);
        }
    }
}
