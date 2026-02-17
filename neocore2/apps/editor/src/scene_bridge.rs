#![forbid(unsafe_op_in_unsafe_fn)]

use parking_lot::{Mutex, RwLock};
use std::sync::Arc;

use newengine_math::{EulerRot, Quat, Vec3};

use newengine_ecs::EntityId;
use newengine_materials::api::MaterialRegistryApi;
use newengine_materials::{MaterialDescriptor, MaterialId, MaterialRef, MaterialRegistry};
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

#[derive(Default)]
struct SceneQueue {
    cmds: Vec<SceneCommand>,
}

#[derive(Debug, Clone, Copy)]
pub struct GridSettings {
    pub auto_spacing: bool,
    pub spacing: f32,
    pub follow_camera: bool,
    pub half_lines: u32,
    pub major_every: u32,
    pub minor_color: [f32; 4],
    pub major_color: [f32; 4],
    pub background_color: [f32; 4],
}

impl Default for GridSettings {
    #[inline]
    fn default() -> Self {
        // Neutral, Blender-like defaults: readable grid, not “space”.
        Self {
            auto_spacing: true,
            spacing: 1.0,
            follow_camera: false,
            half_lines: 80,
            major_every: 10,
            minor_color: [0.32, 0.32, 0.34, 1.0],
            major_color: [0.45, 0.45, 0.48, 1.0],
            background_color: [0.10, 0.10, 0.11, 1.0],
        }
    }
}

impl GridSettings {
    /// Compute the effective grid spacing for the current camera distance.
    ///
    /// This keeps renderer logic data-driven and avoids hardcoded grid math spread across modules.
    #[inline]
    pub fn effective_spacing(&self, camera_distance: f32) -> f32 {
        if self.auto_spacing {
            let d = camera_distance.max(0.01);
            // Heuristic: quantize to powers of 10 to keep the grid stable while zooming.
            let base = (d * 0.08).max(0.05);
            let pow10 = 10.0f32.powf(base.log10().floor());
            pow10.clamp(0.05, 1000.0)
        } else {
            self.spacing.max(0.0001)
        }
    }
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
    materials: Arc<RwLock<MaterialRegistry>>,
    grid_settings: Arc<Mutex<GridSettings>>,
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
            grid_settings: Arc::new(Mutex::new(GridSettings::default())),
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

    /// Get current grid settings snapshot.
    #[inline]
    pub fn grid_settings(&self) -> GridSettings {
        *self.grid_settings.lock()
    }

    /// Replace grid settings (editor-side).
    #[inline]
    pub fn set_grid_settings(&self, settings: GridSettings) {
        *self.grid_settings.lock() = settings;
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

                        // 2) Now take world_mut (locks mutable borrow of scene).
                        let world = scene.world_mut();

                        // 3) Resolve root using world only.
                        let root = root_opt.unwrap_or_else(|| world.spawn());

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
                            t.position = Vec3::new(position[0], position[1], position[2]);
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
        } // scene write lock dropped here

        if let Some(sel) = pending_selection {
            self.set_selection(sel);
        }
    }
}