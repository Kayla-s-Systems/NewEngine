#![forbid(unsafe_op_in_unsafe_fn)]

mod config;
mod handler;
mod input_bridge;
mod resources;
mod runner;

pub use config::{WinitAppConfig, WinitWindowPlacement};
pub use resources::{WinitWindowHandles, WinitWindowInitSize};
pub use runner::{run_winit_app, run_winit_app_with_config};
