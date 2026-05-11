#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::RString;
use newengine_platform_api::{PlatformLoadingOverlayV1, PlatformStepResultV1};

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScreenOverlayStatusKind {
    Loading,
    Syncing,
    Applying,
    Ready,
    Degraded,
    Error,
}

#[derive(Debug, Clone)]
pub(crate) struct ScreenOverlayStatus {
    kind: ScreenOverlayStatusKind,
    title: String,
    status: String,
    detail: String,
    progress_01: f32,
}

impl ScreenOverlayStatus {
    pub(crate) fn loading(
        title: impl Into<String>,
        status: impl Into<String>,
        detail: impl Into<String>,
        progress_01: f32,
    ) -> Self {
        Self::new(ScreenOverlayStatusKind::Loading, title, status, detail, progress_01)
    }

    pub(crate) fn ready(
        title: impl Into<String>,
        status: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self::new(ScreenOverlayStatusKind::Ready, title, status, detail, 1.0)
    }

    pub(crate) fn degraded(
        status: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self::new(
            ScreenOverlayStatusKind::Degraded,
            "NEWENGINE // DEGRADED MODE",
            status,
            detail,
            1.0,
        )
    }

    #[allow(dead_code)]
    pub(crate) fn error(
        status: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self::new(
            ScreenOverlayStatusKind::Error,
            "NEWENGINE // ERROR",
            status,
            detail,
            1.0,
        )
    }

    pub(crate) fn new(
        kind: ScreenOverlayStatusKind,
        title: impl Into<String>,
        status: impl Into<String>,
        detail: impl Into<String>,
        progress_01: f32,
    ) -> Self {
        Self {
            kind,
            title: normalize(title.into(), default_title(kind)),
            status: normalize(status.into(), "Preparing runtime..."),
            detail: normalize(detail.into(), "Runtime shell is alive."),
            progress_01: progress_01.clamp(0.0, 1.0),
        }
    }

    pub(crate) fn into_step_result(self, spinner_phase: u32) -> PlatformStepResultV1 {
        PlatformStepResultV1 {
            exit_requested: false,
            loading_overlay: self.into_platform_overlay(spinner_phase),
        }
    }

    pub(crate) fn into_platform_overlay(self, spinner_phase: u32) -> PlatformLoadingOverlayV1 {
        PlatformLoadingOverlayV1 {
            active: true,
            progress_01: self.progress_01,
            spinner_phase: if self.is_terminal() { 0 } else { spinner_phase },
            title: RString::from(self.title.as_str()),
            status: RString::from(self.status.as_str()),
            detail: RString::from(self.detail.as_str()),
        }
    }

    #[inline]
    fn is_terminal(&self) -> bool {
        matches!(
            self.kind,
            ScreenOverlayStatusKind::Ready | ScreenOverlayStatusKind::Degraded | ScreenOverlayStatusKind::Error
        )
    }
}

fn normalize(value: String, fallback: &'static str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback.to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn default_title(kind: ScreenOverlayStatusKind) -> &'static str {
    match kind {
        ScreenOverlayStatusKind::Loading => "NEWENGINE // BOOTSTRAP",
        ScreenOverlayStatusKind::Syncing => "NEWENGINE // SYNCHRONIZING",
        ScreenOverlayStatusKind::Applying => "NEWENGINE // APPLYING",
        ScreenOverlayStatusKind::Ready => "NEWENGINE // READY",
        ScreenOverlayStatusKind::Degraded => "NEWENGINE // DEGRADED MODE",
        ScreenOverlayStatusKind::Error => "NEWENGINE // ERROR",
    }
}
