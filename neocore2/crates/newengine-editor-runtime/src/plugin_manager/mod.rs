#![forbid(unsafe_op_in_unsafe_fn)]

pub mod bridge;
#[cfg(feature = "editor-ui")]
pub mod ui;

pub use bridge::PluginManagerBridge;
#[cfg(feature = "editor-ui")]
pub use ui::PluginManagerUi;
