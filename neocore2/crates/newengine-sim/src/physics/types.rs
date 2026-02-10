#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Mutex;

use glam::{Quat, Vec3};
use hashbrown::HashMap;

use joltc_sys as sys;
use newengine_physics_jolt::{JoltInitDesc, PhysicsWorld};

/// Physics singleton stored inside ECS.
///
/// The simulation layer owns the physics world so both editor and game can reuse the same pipeline.
pub struct PhysicsCtx {
    pub(crate) world: Mutex<PhysicsWorld>,
    pub(crate) bodies: HashMap<u64, sys::JPC_BodyID>,
}

impl PhysicsCtx {
    #[inline]
    pub fn new(desc: JoltInitDesc) -> Result<Self, newengine_physics_jolt::PhysicsError> {
        Ok(Self {
            world: Mutex::new(PhysicsWorld::new(desc)?),
            bodies: HashMap::new(),
        })
    }
}

/// Physics initialization bundle.
///
/// This is a host-facing description of the physics world configuration.
/// Keep it deterministic and explicit.
#[derive(Clone, Copy, Debug)]
pub struct PhysicsInitDesc {
    pub jolt: JoltInitDesc,
    pub settings: PhysicsSettings,
}

impl Default for PhysicsInitDesc {
    #[inline]
    fn default() -> Self {
        Self {
            jolt: JoltInitDesc::default(),
            settings: PhysicsSettings::default(),
        }
    }
}

/// Read-only debug snapshot for editor/game diagnostics.
#[derive(Clone, Copy, Debug, Default)]
pub struct PhysicsDebugStats {
    pub tick: u64,
    pub alpha: f32,
    pub steps_last: u32,
    pub bodies_total: u32,
}

/// Simulation stepping settings.
#[derive(Clone, Copy, Debug)]
pub struct PhysicsSettings {
    pub fixed_dt: f32,
    pub max_substeps: u32,
    pub max_frame_dt: f32,
}

impl Default for PhysicsSettings {
    #[inline]
    fn default() -> Self {
        Self {
            fixed_dt: 1.0 / 60.0,
            max_substeps: 8,
            max_frame_dt: 0.25,
        }
    }
}

/// Runtime step state.
#[derive(Clone, Copy, Debug, Default)]
pub struct PhysicsStepState {
    pub(crate) accum: f32,
    pub alpha: f32,
    pub tick: u64,
    pub steps_last: u32,
}

/// Rigidbody kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RigidBodyKind {
    Static,
    Dynamic,
    Kinematic,
}

impl Default for RigidBodyKind {
    #[inline]
    fn default() -> Self {
        Self::Dynamic
    }
}

/// Rigidbody component.
#[derive(Clone, Copy, Debug, Default)]
pub struct RigidBody {
    pub kind: RigidBodyKind,
    pub object_layer: u16,
}

/// Collider component.
#[derive(Clone, Copy, Debug)]
pub enum Collider {
    Sphere { radius: f32 },
    Box {
        half_extents: Vec3,
        convex_radius: f32,
    },
}

impl Default for Collider {
    #[inline]
    fn default() -> Self {
        Self::Sphere { radius: 0.5 }
    }
}

/// Baked physics body handle stored on an entity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysicsBody {
    pub id: sys::JPC_BodyID,
}

/// Cached physics pose for interpolation.
#[derive(Clone, Copy, Debug, Default)]
pub struct PhysicsPose {
    pub prev_pos: Vec3,
    pub prev_rot: Quat,
    pub curr_pos: Vec3,
    pub curr_rot: Quat,
}