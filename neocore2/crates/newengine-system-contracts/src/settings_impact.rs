#![forbid(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SettingsApplyImpact {
    Live,
    RequiresSwapchainRecreate,
    RequiresRendererRecreate,
    RequiresDeviceReset,
    RequiresRuntimeRestart,
    RequiresFullRestart,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsApplyPlan {
    pub impact: SettingsApplyImpact,
    pub summary: String,
    pub affected_systems: Vec<String>,
}
