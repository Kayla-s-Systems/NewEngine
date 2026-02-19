#![forbid(unsafe_op_in_unsafe_fn)]

use parking_lot::{Mutex, RwLock};
use std::sync::Arc;

use newengine_math::{EulerRot, Quat, Vec3};

use newengine_ecs::EntityId;
use newengine_materials::api::MaterialRegistryApi;
use newengine_materials::{MaterialDescriptor, MaterialId, MaterialRef, MaterialRegistry};
use newengine_primitives::{builtins, Primitive, PrimitiveId, PrimitiveRegistry};
use newengine_scene::{components::SceneRoot, SceneState};
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

    /// Set local transform of an entity.
    SetTransform {
        entity: EntityId,
        position: [f32; 3],
        rotation_ypr: [f32; 3],
        scale: [f32; 3],
    },

    /// Set primitive color (if entity has `Primitive`).
    SetPrimitiveColor {
        entity: EntityId,
        color: [f32; 4],
    },

    /// Assign a material id to an entity (adds/overwrites `MaterialRef`).
    SetMaterial {
        entity: EntityId,
        material: MaterialId,
    },

    /// Update a material descriptor in the registry.
    UpdateMaterial {
        material: MaterialId,
        desc: MaterialDescriptor,
    },
}

#[inline]
fn place_spawn_position(base: Vec3, primitive_index: usize) -> Vec3 {
    // Deterministic, editor-friendly placement.
    //
    // If we always spawn at the exact same coordinates, objects overlap perfectly and it *looks*
    // like the previous one disappears.
    // Place new objects on a small grid in XZ around the requested base position.
    //
    // NOTE: Intentionally simple and deterministic (no RNG, no camera dependency).
    let spacing = 1.75_f32;
    let cols = 6_usize;

    let x = (primitive_index % cols) as f32;
    let z = (primitive_index / cols) as f32;

    // Center around base so early spawns distribute on both sides.
    let cx = (cols as f32 - 1.0) * 0.5;
    base + Vec3::new((x - cx) * spacing, 0.0, z * spacing)
}

#[derive(Default)]
struct SceneQueue {
    cmds: Vec<SceneCommand>,
}

// Grid is an editor overlay with fixed defaults. We intentionally avoid exposing runtime tuning
// knobs from the scene layer to keep render world clean and deterministic.

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
    materials: Arc<RwLock<MaterialRegistry>>,
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
            materials: Arc::new(RwLock::new(MaterialRegistry::with_builtins())),
        }
    }

    #[inline]
    pub fn primitives(&self) -> Arc<RwLock<PrimitiveRegistry>> {
        Arc::clone(&self.primitives)
    }

    #[inline]
    pub fn materials(&self) -> Arc<RwLock<MaterialRegistry>> {
        Arc::clone(&self.materials)
    }

    /// Snapshot materials for UI.
    ///
    /// Returns sorted (name, id) pairs.
    #[inline]
    pub fn materials_snapshot(&self) -> Vec<(String, MaterialId)> {
        let reg = self.materials.read();
        let mut out: Vec<(String, MaterialId)> = reg
            .ids()
            .into_iter()
            .filter_map(|id| reg.name(id).map(|n| (n, id)))
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
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

    #[inline]
    pub fn cmd_set_transform(
        &self,
        entity: EntityId,
        position: Vec3,
        rotation_ypr: (f32, f32, f32),
        scale: Vec3,
    ) {
        self.queue.lock().cmds.push(SceneCommand::SetTransform {
            entity,
            position: [position.x, position.y, position.z],
            rotation_ypr: [rotation_ypr.0, rotation_ypr.1, rotation_ypr.2],
            scale: [scale.x, scale.y, scale.z],
        });
    }

    #[inline]
    pub fn cmd_set_primitive_color(&self, entity: EntityId, color: [f32; 4]) {
        self.queue
            .lock()
            .cmds
            .push(SceneCommand::SetPrimitiveColor { entity, color });
    }

    #[inline]
    pub fn cmd_set_material(&self, entity: EntityId, material: MaterialId) {
        self.queue
            .lock()
            .cmds
            .push(SceneCommand::SetMaterial { entity, material });
    }

    #[inline]
    pub fn cmd_update_material(&self, material: MaterialId, desc: MaterialDescriptor) {
        self.queue
            .lock()
            .cmds
            .push(SceneCommand::UpdateMaterial { material, desc });
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

            // IMPORTANT:
            // The ECS relies on a monotonically increasing tick for change tracking.
            // `get_mut_tracked()` tags writes with the current tick. If tick does not advance,
            // downstream systems that use `query_changed(since_tick)` (e.g. transform propagation)
            // can miss updates, causing the gizmo/overlay to move while geometry stays put.
            scene.world_mut().advance_tick();

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
                        let cam_opt = scene.active_camera();

                        // 2) Now take world_mut (locks mutable borrow of scene).
                        let world = scene.world_mut();

                        // 3) Resolve root using world only.
                        //
                        // IMPORTANT:
                        // If the scene root marker/state was not bootstrapped (or was removed by
                        // a tool/plugin), spawning primitives must not fall back to a plain
                        // `world.spawn()` root. Some higher-level tooling assumes `SceneRoot` +
                        // `SceneState.root` exist; without them objects can look like they
                        // "replace" each other (overlap at the origin / detach from expected
                        // hierarchy).
                        let root = match root_opt {
                            Some(r) => r,
                            None => {
                                let r = spawn_named(world, "Root");
                                let _ = world.insert(r, SceneRoot);

                                // Keep SceneState consistent.
                                if world.resource::<SceneState>().is_none() {
                                    world.insert_resource(SceneState::new(Some(r), cam_opt));
                                } else if let Some(st) = world.resource_mut::<SceneState>() {
                                    st.root = Some(r);
                                }

                                r
                            }
                        };

                        // Place new primitives deterministically so they don't overlap and
                        // visually "replace" each other.
                        // Use a join query count instead of raw storage `len()`.
                        // This stays correct even if the storage contains tombstones.
                        let prim_index = world.query::<Primitive>().count();

                        let base_pos = Vec3::new(position[0], position[1], position[2]);
                        let spawn_pos = place_spawn_position(base_pos, prim_index);

                        let e = spawn_named(world, name);
                        let _ = newengine_transform::set_parent(world, e, Some(root));

                        let _ = world.insert(e, Primitive { id, color });

                        // Default material for all spawned primitives.
                        // Registry is deterministic: register_named returns existing id if present.
                        let default_mat = self
                            .materials
                            .read()
                            .register_named("Default", MaterialDescriptor::default());
                        let _ = world.insert(e, MaterialRef { id: default_mat });

                        if let Some(t) = world.get_mut_tracked::<Transform>(e) {
                            t.position = spawn_pos;
                            t.scale = Vec3::new(scale[0], scale[1], scale[2]);
                        }

                        pending_selection = Some(Some(e));
                    }

                    SceneCommand::SetTransform {
                        entity,
                        position,
                        rotation_ypr,
                        scale,
                    } => {
                        let world = scene.world_mut();
                        if let Some(t) = world.get_mut_tracked::<Transform>(entity) {
                            t.position = Vec3::new(position[0], position[1], position[2]);
                            t.rotation = Quat::from_euler(
                                EulerRot::YXZ,
                                rotation_ypr[0],
                                rotation_ypr[1],
                                rotation_ypr[2],
                            );
                            t.scale = Vec3::new(scale[0], scale[1], scale[2]);
                        }
                    }

                    SceneCommand::SetPrimitiveColor { entity, color } => {
                        let world = scene.world_mut();
                        if let Some(p) = world.get_mut_tracked::<Primitive>(entity) {
                            p.color = color;
                        }
                    }

                    SceneCommand::SetMaterial { entity, material } => {
                        let world = scene.world_mut();
                        let _ = world.insert(entity, MaterialRef { id: material });
                    }

                    SceneCommand::UpdateMaterial { material, desc } => {
                        // Registry update is editor-side shared state; do not touch the world.
                        let _ = self.materials.read().set_desc(material, desc);
                    }
                }
            }

            // Hard invariant for renderable entities: if an entity has a `Primitive`, it must have
            // a valid `MaterialRef`. This prepares the scene for future renderables (meshes, models)
            // and removes fragile fallback logic from the renderer.
            // Hard invariant for renderable entities: if an entity has a `Primitive`, it must have
            // a valid `MaterialRef`. This prepares the scene for future renderables (meshes, models)
            // and removes fragile fallback logic from the renderer.
            let default_mat = self
                .materials
                .read()
                .register_named("Default", MaterialDescriptor::default());

            let world = scene.world_mut();

            // Phase 1: collect (read-only)
            let mut fix_list: Vec<EntityId> = Vec::new();
            for (e, _p) in world.query::<Primitive>() {
                let needs_fix = match world.get::<MaterialRef>(e) {
                    None => true,
                    Some(mr) => !mr.id.is_valid(),
                };
                if needs_fix {
                    fix_list.push(e);
                }
            }

            // Phase 2: apply (mutable)
            for e in fix_list {
                let _ = world.insert(e, MaterialRef { id: default_mat });
            }

        } // scene write lock dropped here

        if let Some(sel) = pending_selection {
            self.set_selection(sel);
        }
    }
}