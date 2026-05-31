#![forbid(unsafe_op_in_unsafe_fn)]

mod components;

pub use newengine_entity_api::EntityHandle;
pub use components::{Children, GlobalTransform, Parent, Transform, TransformDirty, WorldPose};

use newengine_service_api::{hash_u128, InterfaceId, ServiceKey};

/// Service key for the transform runtime service (systems + utilities).
pub const TRANSFORM_SERVICE: ServiceKey =
    ServiceKey::new(hash_u128("kalitech.transform.service.v1"));

/// Interface id for `ITransformRuntime` v1.
pub const ITRANSFORM_RUNTIME_V1: InterfaceId =
    InterfaceId::new(hash_u128("kalitech.transform.ITransformRuntime.v1"));
