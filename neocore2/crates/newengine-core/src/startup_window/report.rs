#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::PathBuf;

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
        }
    }

    pub(crate) fn unavailable(details: impl Into<String>) -> Self {
        Self {
            decision: StartupWindowDecision::Unavailable,
            config_path: None,
            disabled_by: None,
            details: details.into(),
            warnings: Vec::new(),
        }
    }

    pub(crate) fn unavailable_with_warnings(
        config_path: Option<PathBuf>,
        details: impl Into<String>,
        warnings: Vec<String>,
    ) -> Self {
        Self {
            decision: StartupWindowDecision::Unavailable,
            config_path,
            disabled_by: None,
            details: details.into(),
            warnings,
        }
    }
}
