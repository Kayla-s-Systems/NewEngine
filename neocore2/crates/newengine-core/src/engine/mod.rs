#![forbid(unsafe_op_in_unsafe_fn)]

mod config;
mod contracts;
mod core;
mod external_event;
mod frame_loop;
mod module_boot;
mod panic;
mod plugins;
mod run_stage;
mod timing;

pub use config::EngineConfig;
pub use core::Engine;
