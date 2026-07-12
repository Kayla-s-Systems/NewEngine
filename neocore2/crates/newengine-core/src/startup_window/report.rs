#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::PathBuf;

use crate::loading::{BootFrameDto, ResolvedLoadingAssignment};
use crate::startup::WindowPlacement;

use super::settings::StartupLaunchSettings;

pub type StartupLoadingAssignmentReport = ResolvedLoadingAssignment;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StartupWindowDecision {
    Presented,
    Skipped,
    Unavailable,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StartupWindowSelection {
    pub launch_settings: StartupLaunchSettings,
    pub window_size: (u32, u32),
    pub window_placement: WindowPlacement,
}

#[derive(Clone, Debug)]
pub struct StartupWindowReport {
    pub decision: StartupWindowDecision,
    pub config_path: Option<PathBuf>,
    pub disabled_by: Option<String>,
    pub details: String,
    pub warnings: Vec<String>,
    pub loading_assignment: Option<StartupLoadingAssignmentReport>,
    pub boot_frame: Option<BootFrameDto>,
    pub selection: Option<StartupWindowSelection>,
}

impl StartupWindowReport {
    #[cfg(feature = "startup-window-egui")]
    pub(crate) fn presented_with_selection(
        config_path: PathBuf,
        details: impl Into<String>,
        warnings: Vec<String>,
        selection: StartupWindowSelection,
    ) -> Self {
        Self {
            decision: StartupWindowDecision::Presented,
            config_path: Some(config_path),
            disabled_by: None,
            details: details.into(),
            warnings,
            loading_assignment: None,
            boot_frame: None,
            selection: Some(selection),
        }
    }

    pub(crate) fn presented_with_loading_assignment(
        config_path: PathBuf,
        details: impl Into<String>,
        warnings: Vec<String>,
        loading_assignment: StartupLoadingAssignmentReport,
        boot_frame: BootFrameDto,
    ) -> Self {
        Self {
            decision: StartupWindowDecision::Presented,
            config_path: Some(config_path),
            disabled_by: None,
            details: details.into(),
            warnings,
            loading_assignment: Some(loading_assignment),
            boot_frame: Some(boot_frame),
            selection: None,
        }
    }

    pub(crate) fn attach_loading_handoff(&mut self, handoff: StartupWindowReport) {
        self.loading_assignment = handoff.loading_assignment;
        self.boot_frame = handoff.boot_frame;
        self.warnings.extend(handoff.warnings);
    }

    pub(crate) fn skipped(disabled_by: impl Into<String>) -> Self {
        let disabled_by = disabled_by.into();
        Self {
            decision: StartupWindowDecision::Skipped,
            config_path: None,
            disabled_by: Some(disabled_by.clone()),
            details: format!("disabled by {disabled_by}"),
            warnings: Vec::new(),
            loading_assignment: None,
            boot_frame: None,
            selection: None,
        }
    }

    #[cfg(feature = "startup-window-egui")]
    pub(crate) fn cancelled(config_path: PathBuf, details: impl Into<String>) -> Self {
        Self {
            decision: StartupWindowDecision::Cancelled,
            config_path: Some(config_path),
            disabled_by: None,
            details: details.into(),
            warnings: Vec::new(),
            loading_assignment: None,
            boot_frame: None,
            selection: None,
        }
    }

    pub(crate) fn unavailable(details: impl Into<String>) -> Self {
        Self {
            decision: StartupWindowDecision::Unavailable,
            config_path: None,
            disabled_by: None,
            details: details.into(),
            warnings: Vec::new(),
            loading_assignment: None,
            boot_frame: None,
            selection: None,
        }
    }
}
