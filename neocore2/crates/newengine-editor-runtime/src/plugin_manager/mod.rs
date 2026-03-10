#![forbid(unsafe_op_in_unsafe_fn)]

pub mod bridge;
pub mod ui;

pub use bridge::PluginManagerBridge;
pub use ui::PluginManagerUi;
