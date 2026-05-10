#![forbid(unsafe_op_in_unsafe_fn)]

mod config;
mod contracts;
mod core;
mod dispatch;
mod frame_loop;
mod module_boot;
mod module_slot;
mod panic;
mod plugins;
mod run_stage;
mod startup_graph;
mod timing;

pub use config::{EngineConfig, ModuleFaultTolerance, PluginFaultTolerance};
pub use core::Engine;
