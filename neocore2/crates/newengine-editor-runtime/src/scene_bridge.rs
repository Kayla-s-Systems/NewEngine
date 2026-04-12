#![forbid(unsafe_op_in_unsafe_fn)]

use parking_lot::{Mutex, RwLock};
use std::sync::Arc;

use newengine_bounds::Bounds;
use newengine_ecs::EntityId;
use newengine_lighting::{AmbientLight, DirectionalLight, PointLight};
use newengine_materials::api::MaterialRegistryApi;
use newengine_materials::{
    MaterialDescriptor, MaterialDomain, MaterialId, MaterialOverrides, MaterialRef,
    MaterialRegistry, ShadingModel,
};
use newengine_math::{EulerRot, Quat, Vec3};
use newengine_primitives::{builtins, Primitive, PrimitiveId, PrimitiveRegistry};
use newengine_scene::components::SceneRoot;
use newengine_scene::{spawn_named, Scene, SceneAsset, SceneState};
use newengine_transform::Transform;

use crate::gameplay::{
    ensure_collision_body, remove_collision_body, spawn_default_player, CollisionBody,
    DisplayMode, DisplayVisibility, EditorPlayMode,
};
use crate::scene_bootstrap::bootstrap_editor_scene;

#[derive(Clone, Debug)]
pub enum SceneCommand {
    NewScene,
    LoadSceneAsset { asset: SceneAsset },

    SpawnPrimitive {
        id: PrimitiveId,
        name: String,
        position: [f32; 3],
        scale: [f32; 3],
        color: [f32; 4],
    },
    SpawnDirectionalLight {
        name: String,
        position: [f32; 3],
        direction_ws: [f32; 3],
    },
    SpawnPointLight {
        name: String,
        position: [f32; 3],
    },
    SpawnPlayer {
        name: String,
        position: [f32; 3],
    },
    SpawnImportedAsset {
        descriptor: SceneImportedAssetDescriptor,
        name: String,
        position: [f32; 3],
    },

    SetTransform {
        entity: EntityId,
        position: [f32; 3],
        rotation_ypr: [f32; 3],
        scale: [f32; 3],
    },
    SetPrimitiveColor {
        entity: EntityId,
        color: [f32; 4],
    },
    SetMaterial {
        entity: EntityId,
        material: MaterialId,
    },
    UpdateMaterial {
        material: MaterialId,
        desc: MaterialDescriptor,
    },

    SetAmbientLight {
        color: [f32; 3],
        intensity: f32,
    },
    SetDirectionalLight {
        entity: EntityId,
        direction_ws: [f32; 3],
        color: [f32; 3],
        intensity: f32,
    },
    SetPointLight {
        entity: EntityId,
        color: [f32; 3],
        intensity: f32,
        range: f32,
    },

    SetCollisionBody {
        entity: EntityId,
        body: CollisionBody,
    },
    ClearCollisionBody {
        entity: EntityId,
    },
    SetDisplayVisibility {
        entity: EntityId,
        mode: DisplayMode,
    },
    SetParent {
        child: EntityId,
        parent: Option<EntityId>,
    },

    SetPlayMode {
        mode: EditorPlayMode,
    },
    SetCollisionWireframe {
        enabled: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PrimitiveMaterialBase {
    pub id: MaterialId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SceneImportedAssetKind {
    StaticMesh,
    SceneReference,
    TextureReference,
    MaterialReference,
    OpaqueReference,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SceneImportedAssetRepresentation {
    PrimitiveCube,
    PrimitivePlane,
    PrimitiveSphere,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SceneImportedAssetAssemblyKind {
    StaticMeshActor,
    SceneAnchor,
    TextureCard,
    MaterialPreviewSphere,
    OpaqueProxy,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SceneImportedAssetAssemblyDescriptor {
    pub assembly: SceneImportedAssetAssemblyKind,
    pub primitive_id: PrimitiveId,
    pub display_mode: DisplayMode,
    pub with_collision: bool,
    pub dynamic_collision: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SceneImportedAssetDescriptor {
    pub logical_path: String,
    pub import_kind: SceneImportedAssetKind,
    pub representation: SceneImportedAssetRepresentation,
    pub assembler_key: String,
    pub assembly: SceneImportedAssetAssemblyDescriptor,
    pub default_scale: [f32; 3],
    pub tint: [f32; 4],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneImportedAssetAssembler {
    pub key: String,
    pub label: &'static str,
    pub import_kind: SceneImportedAssetKind,
    pub assembly: SceneImportedAssetAssemblyKind,
}

#[derive(Default)]
struct SceneQueue {
    cmds: Vec<SceneCommand>,
}

#[derive(Clone)]
pub struct SceneBridge {
    scene: Arc<RwLock<Scene>>,
    queue: Arc<Mutex<SceneQueue>>,
    selection: Arc<Mutex<Option<EntityId>>>,
    primitives: Arc<RwLock<PrimitiveRegistry>>,
    materials: Arc<RwLock<MaterialRegistry>>,
    asset_assemblers: Arc<RwLock<Vec<SceneImportedAssetAssembler>>>,
    play_mode: Arc<Mutex<EditorPlayMode>>,
    collision_wireframe: Arc<Mutex<bool>>,
}

#[inline]
fn place_spawn_position(base: Vec3, index: usize) -> Vec3 {
    let spacing = 1.75_f32;
    let cols = 6_usize;
    let x = (index % cols) as f32;
    let z = (index / cols) as f32;
    let cx = (cols as f32 - 1.0) * 0.5;
    base + Vec3::new((x - cx) * spacing, 0.0, z * spacing)
}

#[inline]
fn ensure_primitive_base(world: &mut newengine_ecs::World, entity: EntityId, base: MaterialId) {
    let _ = world.insert(entity, PrimitiveMaterialBase { id: base });
}

#[inline]
fn apply_primitive_instance(
    world: &mut newengine_ecs::World,
    mats: &MaterialRegistry,
    entity: EntityId,
    base: MaterialId,
    color: [f32; 4],
) {
    let inst_name = format!("__prim_{:016x}", entity.stable_u64());
    let overrides = MaterialOverrides {
        domain: Some(MaterialDomain::Surface),
        shading_model: Some(ShadingModel::Unlit),
        base_color: Some(color),
        ..MaterialOverrides::default()
    };

    let inst_id = mats.upsert_instance_named(base, &inst_name, overrides);
    let _ = world.insert(entity, MaterialRef { id: inst_id });
}

#[inline]
fn ensure_root(scene: &mut Scene) -> EntityId {
    if let Some(root) = scene.root() {
        return root;
    }

    let cam_opt = scene.active_camera();
    let world = scene.world_mut();
    let root = spawn_named(world, "Root");
    let _ = world.insert(root, SceneRoot);

    match world.resource_mut::<SceneState>() {
        Some(st) => {
            st.root = Some(root);
            if st.active_camera.is_none() {
                st.active_camera = cam_opt;
            }
        }
        None => world.insert_resource(SceneState::new(Some(root), cam_opt)),
    }

    root
}

#[inline]
fn primitive_bounds(reg: &PrimitiveRegistry, id: PrimitiveId) -> Option<Bounds> {
    let mesh = reg.build_mesh(id).ok()?;
    Some(Bounds::from_local_sphere(newengine_bounds::Sphere::new(
        mesh.bounds_center,
        mesh.bounds_radius.max(0.001),
    )))
}

#[inline]
fn imported_asset_primitive_id(descriptor: &SceneImportedAssetDescriptor) -> PrimitiveId {
    descriptor.assembly.primitive_id
}

#[inline]
fn imported_asset_collision(descriptor: &SceneImportedAssetDescriptor) -> Option<CollisionBody> {
    if !descriptor.assembly.with_collision {
        return None;
    }
    match descriptor.assembly.assembly {
        SceneImportedAssetAssemblyKind::StaticMeshActor | SceneImportedAssetAssemblyKind::OpaqueProxy => Some(CollisionBody {
            shape: crate::gameplay::CollisionShape::Box {
                half_extents: [
                    descriptor.default_scale[0].abs().max(0.5),
                    descriptor.default_scale[1].abs().max(0.5),
                    descriptor.default_scale[2].abs().max(0.5),
                ],
            },
            dynamic: descriptor.assembly.dynamic_collision,
            is_trigger: false,
        }),
        SceneImportedAssetAssemblyKind::TextureCard => Some(CollisionBody {
            shape: crate::gameplay::CollisionShape::Box {
                half_extents: [
                    descriptor.default_scale[0].abs().max(0.25),
                    0.05,
                    descriptor.default_scale[2].abs().max(0.25),
                ],
            },
            dynamic: false,
            is_trigger: true,
        }),
        SceneImportedAssetAssemblyKind::MaterialPreviewSphere => Some(CollisionBody {
            shape: crate::gameplay::CollisionShape::Sphere {
                radius: descriptor.default_scale[0].abs().max(0.5),
            },
            dynamic: false,
            is_trigger: false,
        }),
        SceneImportedAssetAssemblyKind::SceneAnchor => None,
    }
}

#[inline]
fn restore_non_collision_bounds(
    world: &mut newengine_ecs::World,
    reg: &PrimitiveRegistry,
    entity: EntityId,
) {
    if let Some(prim) = world.get::<Primitive>(entity).copied() {
        if let Some(bounds) = primitive_bounds(reg, prim.id) {
            let _ = world.insert(entity, bounds);
        }
    } else {
        let _ = world.remove::<Bounds>(entity);
    }
}


#[inline]
fn effective_material_base(material: MaterialId, fallback: MaterialId) -> MaterialId {
    if material.is_valid() {
        material
    } else {
        fallback
    }
}


#[inline]
fn builtin_asset_assemblers() -> Vec<SceneImportedAssetAssembler> {
    vec![
        SceneImportedAssetAssembler {
            key: "builtin.static_mesh_actor".to_string(),
            label: "Static Mesh Actor",
            import_kind: SceneImportedAssetKind::StaticMesh,
            assembly: SceneImportedAssetAssemblyKind::StaticMeshActor,
        },
        SceneImportedAssetAssembler {
            key: "builtin.scene_anchor".to_string(),
            label: "Scene Anchor",
            import_kind: SceneImportedAssetKind::SceneReference,
            assembly: SceneImportedAssetAssemblyKind::SceneAnchor,
        },
        SceneImportedAssetAssembler {
            key: "builtin.texture_card".to_string(),
            label: "Texture Card",
            import_kind: SceneImportedAssetKind::TextureReference,
            assembly: SceneImportedAssetAssemblyKind::TextureCard,
        },
        SceneImportedAssetAssembler {
            key: "builtin.material_preview_sphere".to_string(),
            label: "Material Preview Sphere",
            import_kind: SceneImportedAssetKind::MaterialReference,
            assembly: SceneImportedAssetAssemblyKind::MaterialPreviewSphere,
        },
        SceneImportedAssetAssembler {
            key: "builtin.opaque_proxy".to_string(),
            label: "Opaque Proxy",
            import_kind: SceneImportedAssetKind::OpaqueReference,
            assembly: SceneImportedAssetAssemblyKind::OpaqueProxy,
        },
    ]
}

#[inline]
fn resolve_asset_assembler(
    registry: &[SceneImportedAssetAssembler],
    descriptor: &SceneImportedAssetDescriptor,
) -> SceneImportedAssetAssembler {
    registry
        .iter()
        .find(|it| it.key == descriptor.assembler_key)
        .cloned()
        .or_else(|| {
            registry
                .iter()
                .find(|it| it.import_kind == descriptor.import_kind && it.assembly == descriptor.assembly.assembly)
                .cloned()
        })
        .unwrap_or_else(|| SceneImportedAssetAssembler {
            key: "builtin.fallback".to_string(),
            label: "Fallback Opaque Proxy",
            import_kind: descriptor.import_kind,
            assembly: descriptor.assembly.assembly,
        })
}

#[inline]
fn reset_editor_runtime_state(scene: &mut Scene) -> Option<EntityId> {
    bootstrap_editor_scene(scene);
    scene.active_camera()
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
            asset_assemblers: Arc::new(RwLock::new(builtin_asset_assemblers())),
            play_mode: Arc::new(Mutex::new(EditorPlayMode::Edit)),
            collision_wireframe: Arc::new(Mutex::new(true)),
        }
    }

    #[inline]
    pub fn scene(&self) -> Arc<RwLock<Scene>> {
        Arc::clone(&self.scene)
    }

    #[inline]
    pub fn primitives(&self) -> Arc<RwLock<PrimitiveRegistry>> {
        Arc::clone(&self.primitives)
    }

    #[inline]
    pub fn materials(&self) -> Arc<RwLock<MaterialRegistry>> {
        Arc::clone(&self.materials)
    }

    #[inline]
    pub fn register_imported_asset_assembler(&self, assembler: SceneImportedAssetAssembler) {
        let mut registry = self.asset_assemblers.write();
        if let Some(existing) = registry.iter_mut().find(|it| it.key == assembler.key) {
            *existing = assembler;
            return;
        }
        registry.push(assembler);
    }

    #[inline]
    pub fn imported_asset_assemblers_snapshot(&self) -> Vec<SceneImportedAssetAssembler> {
        self.asset_assemblers.read().clone()
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
    pub fn play_mode(&self) -> EditorPlayMode {
        *self.play_mode.lock()
    }

    #[inline]
    pub fn collision_wireframe_enabled(&self) -> bool {
        *self.collision_wireframe.lock()
    }

    #[inline]
    pub fn materials_snapshot(&self) -> Vec<(String, MaterialId)> {
        let reg = self.materials.read();
        let mut out: Vec<(String, MaterialId)> = reg
            .snapshot()
            .into_iter()
            .map(|it| (it.name, it.id))
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

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
    pub fn cmd_new_scene(&self) {
        self.queue.lock().cmds.push(SceneCommand::NewScene);
    }

    #[inline]
    pub fn cmd_load_scene_asset(&self, asset: SceneAsset) {
        self.queue.lock().cmds.push(SceneCommand::LoadSceneAsset { asset });
    }

    #[inline]
    pub fn cmd_spawn_primitive(&self, id: PrimitiveId, name: String, position: Vec3) {
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
    pub fn cmd_spawn_directional_light(&self, name: String, position: Vec3, direction_ws: Vec3) {
        let d = direction_ws.normalize_or_zero();
        self.queue
            .lock()
            .cmds
            .push(SceneCommand::SpawnDirectionalLight {
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
    pub fn cmd_spawn_player(&self, name: String, position: Vec3) {
        self.queue.lock().cmds.push(SceneCommand::SpawnPlayer {
            name,
            position: [position.x, position.y, position.z],
        });
    }


    #[inline]
    pub fn cmd_spawn_imported_asset(&self, descriptor: SceneImportedAssetDescriptor, name: String, position: Vec3) {
        self.queue.lock().cmds.push(SceneCommand::SpawnImportedAsset {
            descriptor,
            name,
            position: [position.x, position.y, position.z],
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
    pub fn cmd_set_ambient_light(&self, color: [f32; 3], intensity: f32) {
        self.queue
            .lock()
            .cmds
            .push(SceneCommand::SetAmbientLight { color, intensity });
    }

    #[inline]
    pub fn cmd_set_directional_light(
        &self,
        entity: EntityId,
        direction_ws: Vec3,
        color: [f32; 3],
        intensity: f32,
    ) {
        let d = direction_ws.normalize_or_zero();
        self.queue
            .lock()
            .cmds
            .push(SceneCommand::SetDirectionalLight {
                entity,
                direction_ws: [d.x, d.y, d.z],
                color,
                intensity,
            });
    }

    #[inline]
    pub fn cmd_set_point_light(
        &self,
        entity: EntityId,
        color: [f32; 3],
        intensity: f32,
        range: f32,
    ) {
        self.queue.lock().cmds.push(SceneCommand::SetPointLight {
            entity,
            color,
            intensity,
            range,
        });
    }

    #[inline]
    pub fn cmd_set_collision_body(&self, entity: EntityId, body: CollisionBody) {
        self.queue
            .lock()
            .cmds
            .push(SceneCommand::SetCollisionBody { entity, body });
    }

    #[inline]
    pub fn cmd_clear_collision_body(&self, entity: EntityId) {
        self.queue
            .lock()
            .cmds
            .push(SceneCommand::ClearCollisionBody { entity });
    }


    #[inline]
    pub fn cmd_set_parent(&self, child: EntityId, parent: Option<EntityId>) {
        self.queue.lock().cmds.push(SceneCommand::SetParent { child, parent });
    }

    #[inline]
    pub fn cmd_set_display_visibility(&self, entity: EntityId, mode: DisplayMode) {
        self.queue
            .lock()
            .cmds
            .push(SceneCommand::SetDisplayVisibility { entity, mode });
    }

    #[inline]
    pub fn cmd_set_play_mode(&self, mode: EditorPlayMode) {
        self.queue.lock().cmds.push(SceneCommand::SetPlayMode { mode });
    }

    #[inline]
    pub fn cmd_set_collision_wireframe(&self, enabled: bool) {
        self.queue
            .lock()
            .cmds
            .push(SceneCommand::SetCollisionWireframe { enabled });
    }

    pub fn apply_commands(&self) {
        let cmds = {
            let mut q = self.queue.lock();
            if q.cmds.is_empty() {
                return;
            }
            std::mem::take(&mut q.cmds)
        };

        let mut pending_selection: Option<Option<EntityId>> = None;
        let mut next_mode: Option<EditorPlayMode> = None;
        let mut next_wire: Option<bool> = None;

        let prims = self.primitives.read();
        let mats = self.materials.read();

        let default_mat = mats.register_named("Default", MaterialDescriptor::default());

        let mut scene = self.scene.write();

        for cmd in cmds {
            match cmd {
                SceneCommand::NewScene => {
                    *scene = Scene::new();
                    pending_selection = Some(reset_editor_runtime_state(&mut *scene));
                    next_mode = Some(EditorPlayMode::Edit);
                    next_wire = Some(true);
                }
                SceneCommand::LoadSceneAsset { asset } => {
                    *scene = Scene::new();
                    if let Err(e) = scene.load_asset(&asset) {
                        log::error!("scene.load_asset failed: {e}");
                    }
                    pending_selection = Some(reset_editor_runtime_state(&mut *scene));
                    next_mode = Some(EditorPlayMode::Edit);
                    next_wire = Some(true);
                }
                SceneCommand::SpawnPrimitive {
                    id,
                    name,
                    position,
                    scale,
                    color,
                } => {
                    let root = ensure_root(&mut *scene);
                    let world = scene.world_mut();
                    let prim_index = world.query::<Primitive>().count();
                    let spawn_pos = place_spawn_position(
                        Vec3::new(position[0], position[1], position[2]),
                        prim_index,
                    );

                    let e = spawn_named(world, name);
                    let _ = newengine_transform::set_parent(world, e, Some(root));
                    let _ = world.insert(e, Primitive { id, color });

                    if let Some(bounds) = primitive_bounds(&prims, id) {
                        let _ = world.insert(e, bounds);
                    }

                    ensure_primitive_base(world, e, default_mat);
                    apply_primitive_instance(world, &*mats, e, default_mat, color);

                    if let Some(t) = world.get_mut_tracked::<Transform>(e) {
                        t.position = spawn_pos;
                        t.scale = Vec3::new(scale[0], scale[1], scale[2]);
                    }

                    pending_selection = Some(Some(e));
                }
                SceneCommand::SpawnDirectionalLight {
                    name,
                    position,
                    direction_ws,
                } => {
                    let root = ensure_root(&mut *scene);
                    let world = scene.world_mut();
                    let e = spawn_named(world, name);
                    let _ = newengine_transform::set_parent(world, e, Some(root));

                    let mut dl = DirectionalLight::default();
                    dl.direction_ws = direction_ws;
                    let _ = world.insert(e, dl);
                    if let Some(t) = world.get_mut_tracked::<Transform>(e) {
                        t.position = Vec3::new(position[0], position[1], position[2]);
                    }
                    pending_selection = Some(Some(e));
                }
                SceneCommand::SpawnPointLight { name, position } => {
                    let root = ensure_root(&mut *scene);
                    let world = scene.world_mut();
                    let e = spawn_named(world, name);
                    let _ = newengine_transform::set_parent(world, e, Some(root));
                    let _ = world.insert(e, PointLight::default());
                    if let Some(t) = world.get_mut_tracked::<Transform>(e) {
                        t.position = Vec3::new(position[0], position[1], position[2]);
                    }
                    pending_selection = Some(Some(e));
                }
                SceneCommand::SpawnPlayer { name, position } => {
                    let root = ensure_root(&mut *scene);
                    let world = scene.world_mut();
                    let player = spawn_default_player(
                        world,
                        Some(root),
                        name,
                        Vec3::new(position[0], position[1], position[2]),
                    );
                    pending_selection = Some(Some(player));
                }
                SceneCommand::SpawnImportedAsset { descriptor, name, position } => {
                    let root = ensure_root(&mut *scene);
                    let world = scene.world_mut();
                    let e = spawn_named(world, name);
                    let _ = newengine_transform::set_parent(world, e, Some(root));

                    let assembler = resolve_asset_assembler(&self.asset_assemblers.read(), &descriptor);
                    let primitive_id = match assembler.assembly {
                        SceneImportedAssetAssemblyKind::StaticMeshActor => builtins::ID_CUBE,
                        SceneImportedAssetAssemblyKind::SceneAnchor => builtins::ID_PLANE,
                        SceneImportedAssetAssemblyKind::TextureCard => builtins::ID_PLANE,
                        SceneImportedAssetAssemblyKind::MaterialPreviewSphere => builtins::ID_SPHERE_UV,
                        SceneImportedAssetAssemblyKind::OpaqueProxy => imported_asset_primitive_id(&descriptor),
                    };
                    let _ = world.insert(e, Primitive {
                        id: primitive_id,
                        color: descriptor.tint,
                    });
                    let _ = world.insert(e, descriptor.clone());
                    let _ = world.insert(e, DisplayVisibility { mode: descriptor.assembly.display_mode });
                    if let Some(bounds) = primitive_bounds(&prims, primitive_id) {
                        let _ = world.insert(e, bounds);
                    }
                    ensure_primitive_base(world, e, default_mat);
                    apply_primitive_instance(world, &*mats, e, default_mat, descriptor.tint);
                    if let Some(collision) = imported_asset_collision(&descriptor) {
                        let _ = world.insert(e, collision);
                    }
                    if let Some(t) = world.get_mut_tracked::<Transform>(e) {
                        t.position = Vec3::new(position[0], position[1], position[2]);
                        t.scale = Vec3::new(
                            descriptor.default_scale[0],
                            descriptor.default_scale[1],
                            descriptor.default_scale[2],
                        );
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
                    let base = world
                        .get::<PrimitiveMaterialBase>(entity)
                        .map(|x| effective_material_base(x.id, default_mat))
                        .unwrap_or(default_mat);
                    ensure_primitive_base(world, entity, base);
                    apply_primitive_instance(world, &*mats, entity, base, color);
                }
                SceneCommand::SetMaterial { entity, material } => {
                    let world = scene.world_mut();
                    if world.get::<Primitive>(entity).is_some() {
                        let base = effective_material_base(material, default_mat);
                        let color = world
                            .get::<Primitive>(entity)
                            .map(|p| p.color)
                            .unwrap_or([1.0, 1.0, 1.0, 1.0]);
                        ensure_primitive_base(world, entity, base);
                        apply_primitive_instance(world, &*mats, entity, base, color);
                    } else {
                        let _ = world.insert(entity, MaterialRef { id: material });
                    }
                }
                SceneCommand::UpdateMaterial { material, desc } => {
                    let _ = mats.set_desc(material, desc);
                }
                SceneCommand::SetAmbientLight { color, intensity } => {
                    let world = scene.world_mut();
                    match world.resource_mut::<AmbientLight>() {
                        Some(a) => {
                            a.color = color;
                            a.intensity = intensity;
                        }
                        None => world.insert_resource(AmbientLight { color, intensity }),
                    }
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
                    }
                }
                SceneCommand::SetCollisionBody { entity, body } => {
                    let world = scene.world_mut();
                    ensure_collision_body(world, entity, body);
                }
                SceneCommand::ClearCollisionBody { entity } => {
                    let world = scene.world_mut();
                    remove_collision_body(world, entity);
                    restore_non_collision_bounds(world, &prims, entity);
                }
                SceneCommand::SetDisplayVisibility { entity, mode } => {
                    let world = scene.world_mut();
                    let _ = world.insert(entity, DisplayVisibility { mode });
                }
                SceneCommand::SetParent { child, parent } => {
                    let world = scene.world_mut();
                    let _ = newengine_transform::set_parent(world, child, parent);
                    pending_selection = Some(Some(child));
                }
                SceneCommand::SetPlayMode { mode } => {
                    next_mode = Some(mode);
                }
                SceneCommand::SetCollisionWireframe { enabled } => {
                    next_wire = Some(enabled);
                }
            }
        }

        if let Some(mode) = next_mode {
            *self.play_mode.lock() = mode;
        }
        if let Some(enabled) = next_wire {
            *self.collision_wireframe.lock() = enabled;
        }
        if let Some(sel) = pending_selection {
            self.set_selection(sel);
        }
    }
}
