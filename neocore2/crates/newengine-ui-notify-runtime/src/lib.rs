#![forbid(unsafe_op_in_unsafe_fn)]

//! Message-pipeline-backed baseline provider for `engine.ui.notify`.

mod module;
mod service;
mod state;

pub use module::{install_ui_notify_runtime, UiNotifyInstallReport, UiNotifyModule};
pub use service::register_ui_notify_gateway_best_effort;
pub use state::{request_from_game_message, UiNotifyPolicy, UiNotifyRuntime};
