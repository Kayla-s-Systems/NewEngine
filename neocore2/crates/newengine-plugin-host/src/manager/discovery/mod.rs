#![forbid(unsafe_op_in_unsafe_fn)]
mod graph;
mod load;
mod logging;
mod metadata;
mod scan;
mod selection;

pub use self::graph::DiscoveryGraph;
pub(super) use self::load::IncrementalLoadState;
pub use self::load::{
    resolve_plugin_discovery_dir, scan_plugin_discovery_graph, IncrementalLoadOutcome,
};
