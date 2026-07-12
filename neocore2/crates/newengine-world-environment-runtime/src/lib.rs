#![forbid(unsafe_op_in_unsafe_fn)]

//! Runtime-hosted provider routes for `engine.world.environment`.
//!
//! Providers expose resolved environment DTOs through the gateway registry and
//! do not mutate ECS/world storage or inspect renderer/GPU state.

mod celestial;
mod constants;
mod consumer_packets;
mod default_provider;
mod math;
mod payload;
mod phenomena;
mod profile_catalog;
mod provider_state;
mod registration;
mod router;
mod visual_asset_catalog;
mod weather_profile;

pub use constants::{
    WORLD_ENVIRONMENT_DEFAULT_PROVIDER_ROUTE, WORLD_ENVIRONMENT_GATEWAY_OWNER,
    WORLD_ENVIRONMENT_NULL_PROVIDER_ROUTE, WORLD_ENVIRONMENT_SNAPSHOT_SCHEMA_V1,
};
pub use registration::register_world_environment_gateway_best_effort;

#[cfg(test)]
mod tests;
