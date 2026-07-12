#![forbid(unsafe_op_in_unsafe_fn)]

//! Runtime-hosted `engine.scene` gateway runtime service.
//!
//! Product profiles choose whether to register this service, but do not own its
//! scene IO transport, authored-scene queries or gateway metadata.

mod asset_io;
mod constants;
mod instantiation;
mod queries;
mod registration;
mod state;
mod transport;
mod validation;

pub use constants::SCENE_GATEWAY_OWNER;
pub use newengine_engine_runtime::SceneBridge;
pub use registration::{register_scene_gateway_best_effort, scene_gateway_service};
pub use state::{EngineSceneGatewayService, SceneGatewayAssetMounts};
