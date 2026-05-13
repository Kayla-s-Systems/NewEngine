#![forbid(unsafe_op_in_unsafe_fn)]

//! AssetManager service contract constants.
//!
//! This module intentionally re-exports the canonical constants from
//! `newengine-assets-api` so existing imports keep compiling while the API crate
//! remains the only source of truth.

pub use newengine_assets_api::{method, ASSET_SERVICE_ID, REQUIRED_RUNTIME_METHODS_V1};
