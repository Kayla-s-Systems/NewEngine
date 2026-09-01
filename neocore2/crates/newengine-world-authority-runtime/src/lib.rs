#![forbid(unsafe_op_in_unsafe_fn)]

//! World-authority gateway adapter kept outside the Engine Host.
//!
//! The host owns process/bootstrap orchestration only. This crate owns the
//! domain-specific ECS/entity/scene authority topology client used by runtime
//! systems that need to reason about provider ownership.

mod ecs_runtime;
mod entity_runtime;
mod runtime_bridge;
mod world_authority;

pub use ecs_runtime::EcsServiceClient;
pub use entity_runtime::EntityServiceClient;
pub use world_authority::{
    WorldAuthorityClient, WorldAuthorityGatewayRoute, WorldAuthoritySnapshot,
};

pub use runtime_bridge::{
    current_entity_authority_map, current_world_authority_frame, RuntimeEntityAuthorityMap,
    RuntimeWorldAuthorityBridge, RuntimeWorldAuthorityFrame, RuntimeWorldAuthorityMode,
    RuntimeWorldAuthorityResource,
};
