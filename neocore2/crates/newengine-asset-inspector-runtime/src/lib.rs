#![forbid(unsafe_op_in_unsafe_fn)]

//! Native asset inspection orchestration for the authored Asset Inspector GUI.
//! Runtime bytes remain owned by `engine.assets`; this crate consumes provider DTOs
//! and native parser APIs, then publishes normalized retained-UI state.

mod inspection;
mod model;
mod mounts;
mod runtime;
mod source_pair;
mod surface;
mod ui_state;

pub use model::{AssetInspectorMode, AssetInspectorReport, InspectorEntry, InspectorField};
pub use runtime::AssetInspectorRuntimeModule;
pub use source_pair::{is_source_asset_ref, source_runtime_counterpart};

pub const ASSET_INSPECTOR_ASSETS_ENV: &str = "NEWENGINE_ASSET_INSPECTOR_ASSETS_DIR";

pub const ASSET_INSPECTOR_SURFACE_ID: &str = "asset.inspector";
pub const ASSET_INSPECTOR_STATE_SOURCE: &str = "engine.assets.inspector";
pub const ASSET_INSPECTOR_STATE_CONTRACT: &str = "newengine.asset_inspector.snapshot.v1";
