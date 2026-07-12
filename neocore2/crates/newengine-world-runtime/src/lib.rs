#![forbid(unsafe_op_in_unsafe_fn)]

//! Runtime-hosted `engine.world` gateway service.
//!
//! `engine.scene` is authored structure while `engine.world` is the living
//! runtime instance. The gateway exposes DTOs only.

mod apply_stage;
mod bookkeeping;
mod invoke;
mod partition;
mod payload;
mod registration;
mod router;
mod service;
mod snapshot;
mod state;
mod streaming_cells;

pub use registration::register_world_gateway_best_effort;
pub use router::world_gateway_service;
pub use service::{
    EngineWorldGatewayService, WORLD_FOUNDATION_PROVIDER_ROUTE, WORLD_GATEWAY_OWNER,
};
