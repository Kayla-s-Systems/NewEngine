#![forbid(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryActionKind {
    None,
    Retry,
    RecreateSwapchain,
    RecreateRenderer,
    SwitchToNullRenderer,
    RestartRuntime,
    Abort,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryPlan {
    pub action: RecoveryActionKind,
    pub reason: String,
    pub user_visible: bool,
}
