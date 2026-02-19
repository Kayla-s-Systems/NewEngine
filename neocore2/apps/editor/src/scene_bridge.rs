#![forbid(unsafe_op_in_unsafe_fn)]

use parking_lot::{Mutex, RwLock};
use std::sync::Arc;

use newengine_math::{EulerRot, Quat, Vec3};

use newengine_ecs::EntityId;
use newengine_materials::api::MaterialRegistryApi;
use newengine_materials::{
    MaterialDescriptor, MaterialDomain, MaterialId, MaterialOverrides, MaterialRef, MaterialRegistry, ShadingModel,
};
use newengine_primitives::{builtins, Primitive, PrimitiveId, PrimitiveRegistry};
use newengine_scene::{components::SceneRoot, SceneState};
use newengine_scene::{spawn_named, Scene};
use newengine_transform::Transform;

use newengine_lighting::{AmbientLight, DirectionalLight, PointLight};

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

    /// Spawn a directional light entity.
    SpawnDirectionalLight {
        name: String,
        position: [f32; 3],
        direction_ws: [f32; 3],
    },

    /// Spawn a point light entity.
    SpawnPointLight {
        name: String,
        position: [f32; 3],
    },

    /// Set world ambient light resource.
    SetAmbientLight {
        color: [f32; 3],
        intensity: f32,
    },

    /// Update parameters of a directional light component.
    SetDirectionalLight {
        entity: EntityId,
        direction_ws: [f32; 3],
        color: [f32; 3],
        intensity: f32,
    },

    /// Update parameters of a point light component.
    SetPointLight {
        entity: EntityId,
        color: [f32; 3],
        intensity: f32,
        range: f32,
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


    #[inline]
    pub fn cmd_spawn_directional_light(&self, name: String, position: Vec3, direction_ws: Vec3) {
        let d = direction_ws.normalize_or_zero();
        self.queue.lock().cmds.push(SceneCommand::SpawnDirectionalLight {
            name,
            position: [position.x, position.y, position.z],
            direction_ws: [d.x, d.y, d.z],
        });
    }

    #[inline]
    pub fn cmd_spawn_point_light(&self, name: String, position: Vec3) {
        self.queue.lock().cmds.push(SceneCommand::SpawnPointLight {
            name,
            position: [position.x, position.y, position.z],
        });
    }

    #[inline]
    pub fn cmd_set_ambient_light(&self, color: [f32; 3], intensity: f32) {
        self.queue.lock().cmds.push(SceneCommand::SetAmbientLight { color, intensity });
    }

    #[inline]
    pub fn cmd_set_directional_light(&self, entity: EntityId, direction_ws: Vec3, color: [f32; 3], intensity: f32) {
        let d = direction_ws.normalize_or_zero();
        self.queue.lock().cmds.push(SceneCommand::SetDirectionalLight {
            entity,
            direction_ws: [d.x, d.y, d.z],
            color,
            intensity,
        });
    }

    #[inline]
    pub fn cmd_set_point_light(&self, entity: EntityId, color: [f32; 3], intensity: f32, range: f32) {
        self.queue.lock().cmds.push(SceneCommand::SetPointLight {
            entity,
            color,
            intensity,
            range,
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

            // IMPORTANT:
            // The ECS relies on a monotonically increasing tick for change tracking.
            // `get_mut_tracked()` tags writes with the current tick. If tick does not advance,
            // downstream systems that use `query_changed(since_tick)` (e.g. transform propagation)
            // can miss updates, causing the gizmo/overlay to move while geometry stays put.
            scene.world_mut().advance_tick();

            // Shared default base material (asset). Instances will reference it.
            let default_mat = self
                .materials
                .read()
                .register_named("Default", MaterialDescriptor::default());

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

                        // Material is bound later via the invariant pass (entity-local instance).
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

                        // Ensure color changes are local to the entity by using a material instance.
                        // This avoids mutating global material assets.
                        let base = world
                            .get::<MaterialRef>(entity)
                            .map(|mr| mr.id)
                            .filter(|id| id.is_valid())
                            .unwrap_or(default_mat);

                        let inst_name = format!("__prim_{:016x}", entity.stable_u64());
                        let inst_id = self.materials.read().register_instance_named(
                            base,
                            &inst_name,
                            MaterialOverrides {
                                domain: Some(MaterialDomain::Surface),
                                shading_model: Some(ShadingModel::Unlit),
                                base_color: Some(color),
                                ..MaterialOverrides::default()
                            },
                        );
                        let _ = world.insert(entity, MaterialRef { id: inst_id });
                    }

                    SceneCommand::SetMaterial { entity, material } => {
                        let world = scene.world_mut();
                        let _ = world.insert(entity, MaterialRef { id: material });
                    }

                    SceneCommand::UpdateMaterial { material, desc } => {
                        // Registry update is editor-side shared state; do not touch the world.
                        let _ = self.materials.read().set_desc(material, desc);
                    }

                    SceneCommand::SpawnDirectionalLight {
                        name,
                        position,
                        direction_ws,
                    } => {
                        let root_opt = scene.root();
                        let cam_opt = scene.active_camera();
                        let world = scene.world_mut();

                        let root = match root_opt {
                            Some(r) => r,
                            None => {
                                let r = spawn_named(world, "Root");
                                let _ = world.insert(r, SceneRoot);
                                if world.resource::<SceneState>().is_none() {
                                    world.insert_resource(SceneState::new(Some(r), cam_opt));
                                } else if let Some(st) = world.resource_mut::<SceneState>() {
                                    st.root = Some(r);
                                }
                                r
                            }
                        };

                        let e = spawn_named(world, name);
                        let _ = newengine_transform::set_parent(world, e, Some(root));
                        let mut dl = DirectionalLight::default();
                        dl.direction_ws = direction_ws;
                        let _ = world.insert(e, dl);

                        if let Some(t) = world.get_mut_tracked::<Transform>(e) {
                            t.position = Vec3::new(position[0], position[1], position[2]);
                        }

                        log::debug!(
                            "scene: spawned DirectionalLight id={:016x} pos=({:.3},{:.3},{:.3}) dir=({:.3},{:.3},{:.3})",
                            e.stable_u64(),
                            position[0],
                            position[1],
                            position[2],
                            direction_ws[0],
                            direction_ws[1],
                            direction_ws[2]
                        );
                        pending_selection = Some(Some(e));
                    }

                    SceneCommand::SpawnPointLight { name, position } => {
                        let root_opt = scene.root();
                        let cam_opt = scene.active_camera();
                        let world = scene.world_mut();

                        let root = match root_opt {
                            Some(r) => r,
                            None => {
                                let r = spawn_named(world, "Root");
                                let _ = world.insert(r, SceneRoot);
                                if world.resource::<SceneState>().is_none() {
                                    world.insert_resource(SceneState::new(Some(r), cam_opt));
                                } else if let Some(st) = world.resource_mut::<SceneState>() {
                                    st.root = Some(r);
                                }
                                r
                            }
                        };

                        let e = spawn_named(world, name);
                        let _ = newengine_transform::set_parent(world, e, Some(root));
                        let _ = world.insert(e, PointLight::default());

                        if let Some(t) = world.get_mut_tracked::<Transform>(e) {
                            t.position = Vec3::new(position[0], position[1], position[2]);
                        }

                        log::debug!(
                            "scene: spawned PointLight id={:016x} pos=({:.3},{:.3},{:.3})",
                            e.stable_u64(),
                            position[0],
                            position[1],
                            position[2]
                        );
                        pending_selection = Some(Some(e));
                    }

                    SceneCommand::SetAmbientLight { color, intensity } => {
                        let world = scene.world_mut();
                        match world.resource_mut::<AmbientLight>() {
                            Some(a) => {
                                a.color = color;
                                a.intensity = intensity;
                            }
                            None => {
                                world.insert_resource(AmbientLight { color, intensity });
                            }
                        }
                        log::debug!(
                            "scene: set AmbientLight color=({:.3},{:.3},{:.3}) intensity={:.3}",
                            color[0], color[1], color[2], intensity
                        );
                    }

                    SceneCommand::SetDirectionalLight {
                        entity,
                        direction_ws,
                        color,
                        intensity,
                    } => {
                        let world = scene.world_mut();
                        if let Some(dl) = world.get_mut_tracked::<DirectionalLight>(entity) {
                            dl.direction_ws = direction_ws;
                            dl.color = color;
                            dl.intensity = intensity;
                            log::debug!(
                                "scene: set DirectionalLight id={:016x} dir=({:.3},{:.3},{:.3}) color=({:.3},{:.3},{:.3}) intensity={:.3}",
                                entity.stable_u64(),
                                direction_ws[0], direction_ws[1], direction_ws[2],
                                color[0], color[1], color[2],
                                intensity
                            );
                        } else {
                            log::warn!(
                                "scene: SetDirectionalLight ignored (missing component) id={:016x}",
                                entity.stable_u64()
                            );
                        }
                    }

                    SceneCommand::SetPointLight {
                        entity,
                        color,
                        intensity,
                        range,
                    } => {
                        let world = scene.world_mut();
                        if let Some(pl) = world.get_mut_tracked::<PointLight>(entity) {
                            pl.color = color;
                            pl.intensity = intensity;
                            pl.range = range;
                            log::debug!(
                                "scene: set PointLight id={:016x} color=({:.3},{:.3},{:.3}) intensity={:.3} range={:.3}",
                                entity.stable_u64(),
                                color[0], color[1], color[2],
                                intensity,
                                range
                            );
                        } else {
                            log::warn!(
                                "scene: SetPointLight ignored (missing component) id={:016x}",
                                entity.stable_u64()
                            );
                        }
                    }
                }
            }

            // Hard invariant for renderable entities: if an entity has a `Primitive`, it must have
            // a valid `MaterialRef`. This prepares the scene for future renderables (meshes, models)
            // and removes fragile fallback logic from the renderer.
            let world = scene.world_mut();

            // Pass 1: collect material instance ids for primitives without mutating `world` during query iteration.
            let mut prim_updates = Vec::new();
            for (e, p) in world.query::<Primitive>() {
                // Always bind primitives through an entity-local material instance.
                // This makes the renderer fully material-driven.
                let color = p.color;

                let base = world
                    .get::<MaterialRef>(e)
                    .map(|mr| mr.id)
                    .filter(|id| id.is_valid() && id.is_asset())
                    .unwrap_or(default_mat);

                let inst_name = format!("__prim_{:016x}", e.stable_u64());
                let inst_id = self.materials.read().register_instance_named(
                    base,
                    &inst_name,
                    MaterialOverrides {
                        domain: Some(MaterialDomain::Surface),
                        shading_model: Some(ShadingModel::Unlit),
                        base_color: Some(color),
                        ..MaterialOverrides::default()
                    },
                );

                prim_updates.push((e, inst_id));
            }

            // Pass 2: apply updates after the immutable query borrow is dropped.
            for (e, inst_id) in prim_updates {
                let _ = world.insert(e, MaterialRef { id: inst_id });
            }
        } // scene write lock dropped here

        if let Some(sel) = pending_selection {
            self.set_selection(sel);
        }
    }
}