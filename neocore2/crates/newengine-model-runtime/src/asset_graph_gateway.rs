//! Runtime implementation for `engine.assets.graph`.
//!
//! The public module is a facade; traversal, semantic ref extraction, VFS projection and
//! ServiceV1 routing are isolated in dedicated implementation modules.

use super::*;

#[path = "asset_graph_refs.rs"]
mod refs;
#[path = "asset_graph_resolver.rs"]
mod resolver;
#[path = "asset_graph_service.rs"]
mod service;
#[path = "asset_graph_vfs.rs"]
mod vfs;

pub use service::register_asset_graph_gateway_best_effort;
