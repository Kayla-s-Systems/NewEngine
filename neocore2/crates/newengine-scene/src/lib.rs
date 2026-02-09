#![forbid(unsafe_op_in_unsafe_fn)]

use hashbrown::HashMap;

use newengine_bounds::{
    propagate_world_bounds, union_world_bounds, Aabb, BoundingSphere, WorldBounds,
};
use newengine_ecs::{EntityId, World};
use newengine_transform::{propagate_transforms, set_parent, GlobalTransform, Transform};

/// Human-readable name of an entity.
#[derive(Clone, Debug)]
pub struct Name(pub String);

impl Name {
    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Marks the scene root.
#[derive(Clone, Copy, Debug, Default)]
pub struct SceneRoot;

/// Marks the active camera entity.
#[derive(Clone, Copy, Debug, Default)]
pub struct ActiveCamera;

/// Opaque gameplay/controller binding.
///
/// The engine core does not know what a controller "is"; concrete controllers live in gameplay
/// code (or plugins). This component only stores an identifier and optional state payload.
#[derive(Clone, Debug)]
pub struct Controller {
    /// Stable controller id (e.g. hash of a string or a plugin-provided id).
    pub kind: u64,
    /// Human-readable name for debugging/UI.
    pub kind_name: String,
    /// Opaque controller state owned by the controller implementation.
    pub state: Vec<u8>,
}

impl Controller {
    #[inline]
    pub fn new(kind: u64, kind_name: impl Into<String>, state: Vec<u8>) -> Self {
        Self {
            kind,
            kind_name: kind_name.into(),
            state,
        }
    }
}

/// Generic, editor-friendly property bag.
///
/// For gameplay performance prefer typed components (e.g. `Health`, `Armor`) instead of using a
/// string-keyed map. This type exists for scripting, UI inspection and prototyping.
#[derive(Clone, Debug, Default)]
pub struct PropertyBag {
    pub props: HashMap<String, PropertyValue>,
}

#[derive(Clone, Debug)]
pub enum PropertyValue {
    Bool(bool),
    I64(i64),
    U64(u64),
    F32(f32),
    F64(f64),
    String(String),
    Bytes(Vec<u8>),
    Vec3([f32; 3]),
}

/// Coordinate system definition for the scene.
///
/// Conventions (recommended engine-default):
/// - right-handed
/// - +Y up
/// - -Z forward
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpAxis {
    Y,
    Z,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForwardAxis {
    NegZ,
    PosZ,
    PosX,
    NegX,
}

/// Scene unit scale: how many meters are in one world unit.
#[derive(Clone, Copy, Debug)]
pub struct UnitScaleMeters(pub f32);

impl Default for UnitScaleMeters {
    #[inline]
    fn default() -> Self {
        Self(1.0)
    }
}

/// Global scene settings (renderer-agnostic, editor-agnostic).
#[derive(Clone, Copy, Debug)]
pub struct SceneSettings {
    pub up: UpAxis,
    pub forward: ForwardAxis,
    pub unit_scale_m: UnitScaleMeters,
}

impl Default for SceneSettings {
    #[inline]
    fn default() -> Self {
        Self {
            up: UpAxis::Y,
            forward: ForwardAxis::NegZ,
            unit_scale_m: UnitScaleMeters::default(),
        }
    }
}

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

    /// A deterministic demo scene used by the editor while the pipeline is being built.
    #[inline]
    pub fn demo() -> Self {
        let mut s = Self::new();
        let root = s.root().unwrap_or_else(|| s.world.spawn());

        let light = spawn_named(&mut s.world, "DirectionalLight");
        let _ = set_parent(&mut s.world, light, Some(root));

        let cube = spawn_named(&mut s.world, "Cube");
        let _ = set_parent(&mut s.world, cube, Some(root));
        if let Some(t) = s.world.get_mut::<Transform>(cube) {
            t.position = glam::Vec3::new(0.0, 0.0, 0.0);
        }

        s
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

        let old: Vec<EntityId> = self.world.query::<ActiveCamera>().map(|(eid, _)| eid).collect();
        for e in old {
            let _ = self.world.remove::<ActiveCamera>(e);
        }

        let _ = self.world.insert(id, ActiveCamera);
        true
    }
}

/// Spawns an entity with `Name` and `Transform`.
#[inline]
pub fn spawn_named(world: &mut World, name: impl Into<String>) -> EntityId {
    let e = world.spawn();
    let _ = world.insert(e, Name(name.into()));
    let _ = world.insert(e, Transform::default());
    e
}

/// Attempts to read an entity name.
#[inline]
pub fn name_or<'a>(world: &'a World, id: EntityId, fallback: &'a str) -> &'a str {
    world.get::<Name>(id).map(|n| n.as_str()).unwrap_or(fallback)
}

/// Computes union world bounds for all entities that have `WorldBounds`.
///
/// This is renderer-agnostic and editor-agnostic.
#[inline]
pub fn scene_world_bounds(world: &World) -> Option<Aabb> {
    let entities = world.query::<WorldBounds>().map(|(id, _)| id);
    union_world_bounds(world, entities)
}

/// Cached scene bounds (union of all `WorldBounds`).
#[derive(Clone, Copy, Debug, Default)]
pub struct SceneBounds {
    pub aabb: Option<Aabb>,
    pub sphere: Option<BoundingSphere>,
}

/// Updates derived scene state:
/// - propagates `Transform` -> `GlobalTransform`/`WorldPose`
/// - propagates `LocalBounds` -> `WorldBounds`
/// - caches the union bounds as a `SceneBounds` resource
#[inline]
pub fn update_scene_world(world: &mut World) {
    propagate_transforms(world);
    propagate_world_bounds(world, |w: &World, id: EntityId| {
        w.get::<GlobalTransform>(id).map(|g| g.0)
    });

    let aabb = scene_world_bounds(world);
    let sphere = aabb.map(|a| a.to_sphere());
    world.insert_resource(SceneBounds { aabb, sphere });
}

/// Computes union world bounds for the provided entities.
#[inline]
pub fn selection_world_bounds(world: &World, entities: impl Iterator<Item=EntityId>) -> Option<Aabb> {
    union_world_bounds(world, entities)
}