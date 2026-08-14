#![forbid(unsafe_op_in_unsafe_fn)]

mod config;
mod contracts;
mod core;
mod dispatch;
mod frame_loop;
mod module_boot;
mod module_slot;
mod panic;
mod plugin_discovery;
mod plugins;
mod run_stage;
mod run_state;
mod startup_graph;
mod timing;

pub use config::{EngineConfig, ModuleFaultTolerance, PluginDiscoveryRoot, PluginFaultTolerance};
pub use core::Engine;
pub use frame_loop::EngineFrameTimingTelemetry;
pub use run_state::{EngineFsm, EngineFsmTransition, EngineRunState};
