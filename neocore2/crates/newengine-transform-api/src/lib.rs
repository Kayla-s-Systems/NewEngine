#![forbid(unsafe_op_in_unsafe_fn)]

mod components;

pub use components::{GlobalTransform, Transform, WorldPose};

#[cfg(feature = "ecs")]
pub use components::{Children, Parent, TransformDirty};

#[cfg(feature = "ecs")]
use newengine_ecs::{EntityId, World};

use newengine_service_api::{hash_u128, InterfaceId, ServiceInterface, ServiceKey};

/// Service key for the transform runtime service (systems + utilities).
pub const TRANSFORM_SERVICE: ServiceKey =
    ServiceKey::new(hash_u128("kalitech.transform.service.v1"));

/// Interface id for `ITransformRuntime` v1.
pub const ITRANSFORM_RUNTIME_V1: InterfaceId =
    InterfaceId::new(hash_u128("kalitech.transform.ITransformRuntime.v1"));

/// Sets (or clears) a parent link and maintains `Children` lists.
///
/// This helper is **API-level** and intentionally depends only on ECS + transform components.
/// Runtime systems may implement more advanced invariants (cycle checks, ordering policies).
#[cfg(feature = "ecs")]
#[inline]
pub fn set_parent(world: &mut World, child: EntityId, parent: Option<EntityId>) -> bool {
    if !world.exists(child) {
        return false;
    }

    if parent == Some(child) {
        return false;
    }

    // Remove from previous parent's children list.
    let prev_parent = world.get::<Parent>(child).map(|p| p.0);
    if let Some(pp) = prev_parent {
        if let Some(ch) = world.get_mut::<Children>(pp) {
            if let Some(pos) = ch.0.iter().position(|&e| e == child) {
                ch.0.swap_remove(pos);
                world.mark_changed::<Children>(pp);
            }
        }
    }

    // Update parent component.
    match parent {
        Some(p) if world.exists(p) => {
            let _ = world.insert(child, Parent(p));

            // Ensure parent has children list.
            if world.get::<Children>(p).is_none() {
                let _ = world.insert(p, Children::default());
            }

            if let Some(ch) = world.get_mut::<Children>(p) {
                if !ch.0.iter().any(|&e| e == child) {
                    ch.0.push(child);
                    world.mark_changed::<Children>(p);
                }
            }
        }
        _ => {
            let _ = world.remove::<Parent>(child);
        }
    }

    true
}

/// Runtime interface is available only with ECS enabled.
#[cfg(feature = "ecs")]
pub mod runtime {
    use super::{InterfaceId, ServiceInterface, ITRANSFORM_RUNTIME_V1};
    use newengine_ecs::{EntityId, World};

    /// VTable for transform runtime integration.
    ///
    /// Notes:
    /// - In-process contract (not a stable cross-dylib ABI).
    /// - Consumers must not rely on impl types; only use this vtable + wrappers.
    #[repr(C)]
    pub struct TransformRuntimeVTable {
        pub ensure_outputs: unsafe fn(*mut (), *mut World),
        pub propagate: unsafe fn(*mut (), *mut World),
        pub set_parent: unsafe fn(*mut (), *mut World, EntityId, Option<EntityId>),
    }

    /// Thin, typed wrapper over `(instance_ptr, vtable_ptr)`.
    #[derive(Clone, Copy)]
    pub struct TransformRuntimeApi {
        instance: *mut (),
        vtbl: *const TransformRuntimeVTable,
    }

    unsafe impl Send for TransformRuntimeApi {}
    unsafe impl Sync for TransformRuntimeApi {}

    impl TransformRuntimeApi {
        #[inline]
        pub fn ensure_outputs(&self, world: &mut World) {
            unsafe { ((*self.vtbl).ensure_outputs)(self.instance, world as *mut _) }
        }

        #[inline]
        pub fn propagate(&self, world: &mut World) {
            unsafe { ((*self.vtbl).propagate)(self.instance, world as *mut _) }
        }

        #[inline]
        pub fn set_parent(&self, world: &mut World, child: EntityId, parent: Option<EntityId>) {
            unsafe { ((*self.vtbl).set_parent)(self.instance, world as *mut _, child, parent) }
        }
    }

    impl ServiceInterface for TransformRuntimeApi {
        type VTable = TransformRuntimeVTable;
        const INTERFACE_ID: InterfaceId = ITRANSFORM_RUNTIME_V1;

        unsafe fn from_raw(instance: *mut (), vtable: *const Self::VTable) -> Self {
            Self {
                instance,
                vtbl: vtable,
            }
        }
    }
}
