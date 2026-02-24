#![forbid(unsafe_op_in_unsafe_fn)]

use core::ffi::c_int;
use core::sync::atomic::{AtomicUsize, Ordering};
use std::ffi::c_uint;
use std::sync::Once;

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
    #[inline]
    fn default() -> Self {
        Self {
            max_bodies: 16 * 1024,
            num_body_mutexes: 0,
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
    #[inline]
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

/// Object layers (Jolt).
pub const LAYER_STATIC: sys::JPC_ObjectLayer = 0;
pub const LAYER_DYNAMIC: sys::JPC_ObjectLayer = 1;

/// Broad phase layers.
pub const BP_STATIC: sys::JPC_BroadPhaseLayer = 0;
pub const BP_DYNAMIC: sys::JPC_BroadPhaseLayer = 1;

// -----------------------------------------------------------------------------
// Global init/shutdown (ref-counted)
// -----------------------------------------------------------------------------

static JOLT_INIT_ONCE: Once = Once::new();
static JOLT_WORLD_REFS: AtomicUsize = AtomicUsize::new(0);

#[inline]
fn jolt_global_acquire() -> Result<(), PhysicsError> {
    let mut ok = true;

    JOLT_INIT_ONCE.call_once(|| unsafe {
        sys::JPC_RegisterDefaultAllocator();
        sys::JPC_FactoryInit();
        sys::JPC_RegisterTypes();
    });

    // We assume once-init succeeded if we reached here.
    // If you need "hard" validation, you must expose a JoltC probe API.
    if !ok {
        return Err(PhysicsError::Init("global init failed"));
    }

    JOLT_WORLD_REFS.fetch_add(1, Ordering::Relaxed);
    Ok(())
}

#[inline]
fn jolt_global_release() {
    let prev = JOLT_WORLD_REFS.fetch_sub(1, Ordering::AcqRel);
    if prev == 1 {
        unsafe {
            sys::JPC_UnregisterTypes();
            //sys::JPC_FactoryDestroy();
        }
    }
}

// -----------------------------------------------------------------------------
// Filters / layers
// -----------------------------------------------------------------------------

/// Minimal 2-layer setup (Static/Dynamic), good baseline for editor + game.
struct NeBroadPhaseLayers;

impl BroadPhaseLayerInterface for NeBroadPhaseLayers {
    #[inline]
    fn get_num_broad_phase_layers(&self) -> u32 {
        2
    }

    #[inline]
    fn get_broad_phase_layer(&self, layer: ObjectLayer) -> BroadPhaseLayer {
        match layer.raw() {
            LAYER_STATIC => BroadPhaseLayer::new(BP_STATIC),
            _ => BroadPhaseLayer::new(BP_DYNAMIC),
        }
    }
}

struct NeObjectVsBroadPhase;

impl ObjectVsBroadPhaseLayerFilter for NeObjectVsBroadPhase {
    #[inline]
    fn should_collide(&self, layer: ObjectLayer, bp: BroadPhaseLayer) -> bool {
        let layer = layer.raw();
        let bp = bp.raw();

        match layer {
            LAYER_STATIC => bp == BP_DYNAMIC,
            _ => true,
        }
    }
}

struct NeObjectLayerPairs;

impl ObjectLayerPairFilter for NeObjectLayerPairs {
    #[inline]
    fn should_collide(&self, a: ObjectLayer, b: ObjectLayer) -> bool {
        let a = a.raw();
        let b = b.raw();
        !(a == LAYER_STATIC && b == LAYER_STATIC)
    }
}

// -----------------------------------------------------------------------------
// PhysicsWorld
// -----------------------------------------------------------------------------

/// Owns per-world resources required by `JPC_PhysicsSystem_Update`.
///
/// This is the boundary where all raw pointers live.
/// Higher layers should treat this as an opaque simulation backend.
pub struct PhysicsWorld {
    physics: PhysicsSystem,

    temp_allocator: *mut sys::JPC_TempAllocatorImpl,
    job_system: *mut sys::JPC_JobSystemThreadPool,

    collision_steps: c_int,
}

// SAFETY:
// This is an owning handle to JoltC resources.
// The engine scheduler must serialize all access (single-writer semantics).
unsafe impl Send for PhysicsWorld {}

impl PhysicsWorld {
    /// Initializes (ref-counted) Jolt global state and creates a new physics system.
    pub fn new(desc: JoltInitDesc) -> Result<Self, PhysicsError> {
        jolt_global_acquire()?;

        let temp_allocator =
            unsafe { sys::JPC_TempAllocatorImpl_new(desc.temp_allocator_bytes as c_uint) };
        if temp_allocator.is_null() {
            jolt_global_release();
            return Err(PhysicsError::Init("TempAllocatorImpl_new returned null"));
        }

        let job_system = unsafe {
            sys::JPC_JobSystemThreadPool_new3(
                desc.job_system_max_jobs,
                desc.job_system_max_barriers,
                0,
            )
        };
        if job_system.is_null() {
            unsafe { sys::JPC_TempAllocatorImpl_delete(temp_allocator) };
            jolt_global_release();
            return Err(PhysicsError::Init("JobSystemThreadPool_new3 returned null"));
        }

        let bp_interface = BroadPhaseLayerInterfaceImpl::new(NeBroadPhaseLayers);
        let obj_vs_bp = ObjectVsBroadPhaseLayerFilterImpl::new(NeObjectVsBroadPhase);
        let layer_pairs = ObjectLayerPairFilterImpl::new(NeObjectLayerPairs);

        let limits = desc.limits;

        let mut physics = PhysicsSystem::new();
        physics.init(
            limits.max_bodies,
            limits.num_body_mutexes,
            limits.max_body_pairs,
            limits.max_contact_constraints,
            bp_interface,
            obj_vs_bp,
            layer_pairs,
        );

        // Optional: do it once right after init to reduce first-step spikes.
        physics.optimize_broad_phase();

        Ok(Self {
            physics,
            temp_allocator,
            job_system,
            collision_steps: desc.collision_steps as c_int,
        })
    }

    /// Steps the world by `dt` seconds.
    #[inline]
    pub fn step(&mut self, dt: f32) -> Result<(), PhysicsError> {
        let system = self.physics.as_raw();

        let err = unsafe {
            sys::JPC_PhysicsSystem_Update(
                system,
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

    /// Raw pointer to the underlying `JPC_PhysicsSystem`.
    #[inline]
    pub fn system_raw(&mut self) -> *mut sys::JPC_PhysicsSystem {
        self.physics.as_raw()
    }

    #[inline]
    pub fn temp_allocator_raw(&self) -> *mut sys::JPC_TempAllocatorImpl {
        self.temp_allocator
    }

    #[inline]
    pub fn job_system_raw(&self) -> *mut sys::JPC_JobSystemThreadPool {
        self.job_system
    }
}

impl Drop for PhysicsWorld {
    fn drop(&mut self) {
        unsafe {
            sys::JPC_JobSystemThreadPool_delete(self.job_system);
            sys::JPC_TempAllocatorImpl_delete(self.temp_allocator);

            // Global shutdown:
            // `JPC_FactoryDestroy` is not present in current joltc_sys bindings.
            // Keep only what exists; factory teardown can be left to process lifetime.
            sys::JPC_UnregisterTypes();
        }
    }
}
