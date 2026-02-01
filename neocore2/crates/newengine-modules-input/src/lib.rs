#![forbid(unsafe_op_in_unsafe_fn)]
#![allow(non_local_definitions)]

mod module;
mod plugins;

pub use crate::plugins::export_plugin_root;