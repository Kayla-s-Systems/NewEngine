#![forbid(unsafe_op_in_unsafe_fn)]

//! Thin Asset Inspector product facade.
//!
//! The product owns navigation and presentation state only. Asset discovery,
//! inspection, schema validation and mutation remain provider-owned engine
//! services routed through `engine.assets`, `engine.assets.inspect` and
//! `engine.assets.edit`.

mod facade;
mod model;
mod runtime;
mod surface;
mod syntax_preview;
mod ui_state;

pub use model::{AssetInspectorMode, InspectorEntry};
pub use runtime::AssetInspectorRuntimeModule;

pub const ASSET_INSPECTOR_ASSETS_ENV: &str = "NEWENGINE_ASSET_INSPECTOR_ASSETS_DIR";

pub const ASSET_INSPECTOR_SURFACE_ID: &str = "asset.inspector";
pub const ASSET_INSPECTOR_STATE_SOURCE: &str = "engine.assets.inspector";
pub const ASSET_INSPECTOR_STATE_CONTRACT: &str = "newengine.asset_inspector.snapshot.v2";
