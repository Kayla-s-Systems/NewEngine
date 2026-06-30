#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::PathBuf;

use crate::loading::{BootFrameDto, ResolvedLoadingAssignment};

pub type StartupLoadingAssignmentReport = ResolvedLoadingAssignment;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StartupWindowDecision {
    Presented,
    Skipped,
    Unavailable,
    Cancelled,
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
}

impl StartupWindowReport {
    #[allow(dead_code)]
    pub(crate) fn presented(
        config_path: PathBuf,
        details: impl Into<String>,
        warnings: Vec<String>,
    ) -> Self {
        Self {
            decision: StartupWindowDecision::Presented,
            config_path: Some(config_path),
            disabled_by: None,
            details: details.into(),
            warnings,
            loading_assignment: None,
            boot_frame: None,
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
        }
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
        }
    }

    #[allow(dead_code)]
    pub(crate) fn cancelled(config_path: PathBuf, details: impl Into<String>) -> Self {
        Self {
            decision: StartupWindowDecision::Cancelled,
            config_path: Some(config_path),
            disabled_by: None,
            details: details.into(),
            warnings: Vec::new(),
            loading_assignment: None,
            boot_frame: None,
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
        }
    }
}
