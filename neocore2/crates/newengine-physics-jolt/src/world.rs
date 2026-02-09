#![forbid(unsafe_op_in_unsafe_fn)]

use core::ffi::c_int;
use core::ptr;

use glam::{Quat, Vec3};
use thiserror::Error;

use joltc_sys as sys;
use rolt::{
    BroadPhaseLayer, BroadPhaseLayerInterface, BroadPhaseLayerInterfaceImpl, ObjectLayer,
    ObjectLayerPairFilter, ObjectLayerPairFilterImpl, ObjectVsBroadPhaseLayerFilter,
    ObjectVsBroadPhaseLayerFilterImpl, PhysicsSystem,
};

#[derive(Debug, Clone, Copy)]
pub struct WorldLimits {
    pub max_bodies: u32,
    pub num_body_mutexes: u32,
    pub max_body_pairs: u32,
    pub max_contact_constraints: u32,
}

impl Default for WorldLimits {
    fn default() -> Self {
        Self {
            // Safe defaults for editor/game bootstrap; tune later per project.
            max_bodies: 16 * 1024,
            num_body_mutexes: 0, // 0 lets Jolt pick a default
            max_body_pairs: 16 * 1024,
            max_contact_constraints: 8 * 1024,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct JoltInitDesc {
    pub limits: WorldLimits,
    pub temp_allocator_bytes: usize,
    pub job_system_max_jobs: u32,
    pub job_system_max_barriers: u32,
    pub collision_steps: i32,
}

impl Default for JoltInitDesc {
    fn default() -> Self {
        Self {
            limits: WorldLimits::default(),
            temp_allocator_bytes: 32 * 1024 * 1024,
            job_system_max_jobs: 1024,
            job_system_max_barriers: 1024,
            collision_steps: 1,
        }
    }
}

#[derive(Error, Debug)]
pub enum PhysicsError {
    #[error("Jolt init failed: {0}")]
    Init(&'static str),
    #[error("Jolt update returned error code: {0:?}")]
    UpdateError(sys::JPC_PhysicsUpdateError),
}

const LAYER_STATIC: sys::JPC_ObjectLayer = 0;
const LAYER_DYNAMIC: sys::JPC_ObjectLayer = 1;

const BP_STATIC: sys::JPC_BroadPhaseLayer = 0;
const BP_DYNAMIC: sys::JPC_BroadPhaseLayer = 1;

/// Minimal 2-layer setup (Static/Dynamic), good baseline for editor + game.
struct NeBroadPhaseLayers;

impl BroadPhaseLayerInterface for NeBroadPhaseLayers {
    fn num_broad_phase_layers(&self) -> u32 {
        2
    }

    fn broad_phase_layer(&self, layer: ObjectLayer) -> BroadPhaseLayer {
        match layer.raw() {
            LAYER_STATIC => BroadPhaseLayer::new(BP_STATIC),
            _ => BroadPhaseLayer::new(BP_DYNAMIC),
        }
    }
}

struct NeObjectVsBroadPhase;

impl ObjectVsBroadPhaseLayerFilter for NeObjectVsBroadPhase {
    fn should_collide(&self, layer: ObjectLayer, broad_phase_layer: BroadPhaseLayer) -> bool {
        match layer.raw() {
            LAYER_STATIC => broad_phase_layer.raw() == BP_DYNAMIC, // static only vs dynamic
            _ => true,                                            // dynamic vs all
        }
    }
}

struct NeLayerPairFilter;

impl ObjectLayerPairFilter for NeLayerPairFilter {
    fn should_collide(&self, a: ObjectLayer, b: ObjectLayer) -> bool {
        let a = a.raw();
        let b = b.raw();
        // Disable static-static to save work.
        !(a == LAYER_STATIC && b == LAYER_STATIC)
    }
}

/// Owns all Jolt global init and per-world resources.
/// All unsafe is encapsulated and audited at this boundary.
pub struct PhysicsWorld {
    physics: PhysicsSystem,

    // Required by JPC_PhysicsSystem_Update. :contentReference[oaicite:3]{index=3}
    temp_allocator: *mut sys::JPC_TempAllocatorImpl,
    job_system: *mut sys::JPC_JobSystemThreadPool,

    collision_steps: c_int,

    // Keep filter vtables alive for the lifetime of PhysicsSystem.
    _bp_iface: BroadPhaseLayerInterfaceImpl,
    _obj_vs_bp: ObjectVsBroadPhaseLayerFilterImpl,
    _pair_filter: ObjectLayerPairFilterImpl,
}

impl PhysicsWorld {
    /// Initializes Jolt (allocator/factory/types) and creates a PhysicsSystem.
    pub fn new(desc: JoltInitDesc) -> Result<Self, PhysicsError> {
        // Global init required by Jolt. :contentReference[oaicite:4]{index=4}
        unsafe {
            sys::JPC_RegisterDefaultAllocator();
            sys::JPC_FactoryInit();
            sys::JPC_RegisterTypes();
        }

        let temp_allocator = unsafe { sys::JPC_TempAllocatorImpl_new(desc.temp_allocator_bytes) };
        if temp_allocator.is_null() {
            return Err(PhysicsError::Init("TempAllocatorImpl_new returned null"));
        }

        // Prefer new3 if available (it is in this binding version). :contentReference[oaicite:5]{index=5}
        let job_system = unsafe {
            sys::JPC_JobSystemThreadPool_new3(
                desc.job_system_max_jobs,
                desc.job_system_max_barriers,
                0, // num_threads: 0 => auto
            )
        };
        if job_system.is_null() {
            unsafe { sys::JPC_TempAllocatorImpl_delete(temp_allocator) };
            return Err(PhysicsError::Init("JobSystemThreadPool_new3 returned null"));
        }

        let bp = BroadPhaseLayerInterfaceImpl::new(NeBroadPhaseLayers);
        let obj_vs_bp = ObjectVsBroadPhaseLayerFilterImpl::new(NeObjectVsBroadPhase);
        let pair = ObjectLayerPairFilterImpl::new(NeLayerPairFilter);

        let mut physics = PhysicsSystem::new();
        physics.init(
            desc.limits.max_bodies,
            desc.limits.num_body_mutexes,
            desc.limits.max_body_pairs,
            desc.limits.max_contact_constraints,
            &bp,
            &obj_vs_bp,
            &pair,
        );

        Ok(Self {
            physics,
            temp_allocator,
            job_system,
            collision_steps: desc.collision_steps as c_int,
            _bp_iface: bp,
            _obj_vs_bp: obj_vs_bp,
            _pair_filter: pair,
        })
    }

    #[inline]
    pub fn raw(&mut self) -> &mut PhysicsSystem {
        &mut self.physics
    }

    /// Advances the simulation by dt seconds.
    ///
    /// Uses the low-level update to pass TempAllocator + JobSystem explicitly. :contentReference[oaicite:6]{index=6}
    pub fn step(&mut self, dt: f32) -> Result<(), PhysicsError> {
        let err = unsafe {
            sys::JPC_PhysicsSystem_Update(
                self.physics.as_mut_ptr(),
                dt,
                self.collision_steps,
                self.temp_allocator,
                self.job_system,
            )
        };

        if err != sys::JPC_PHYSICS_UPDATE_ERROR_NONE {
            return Err(PhysicsError::UpdateError(err));
        }

        Ok(())
    }

    /// Helper: compose a world transform from position/rotation.
    #[inline]
    pub fn compose_transform(position: Vec3, rotation: Quat) -> (sys::JPC_Vec3, sys::JPC_Quat) {
        let p = sys::JPC_Vec3 {
            x: position.x,
            y: position.y,
            z: position.z,
        };
        let q = sys::JPC_Quat {
            x: rotation.x,
            y: rotation.y,
            z: rotation.z,
            w: rotation.w,
        };
        (p, q)
    }
}

impl Drop for PhysicsWorld {
    fn drop(&mut self) {
        unsafe {
            if !self.job_system.is_null() {
                sys::JPC_JobSystemThreadPool_delete(self.job_system);
            }
            if !self.temp_allocator.is_null() {
                sys::JPC_TempAllocatorImpl_delete(self.temp_allocator);
            }

            // Global teardown (best-effort; for multi-world we’ll later refcount this).
            sys::JPC_UnregisterTypes();
            sys::JPC_FactoryDelete();
        }
    }
}