#![forbid(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScreenOverlayStatusKind {
    Boot,
    Loading,
    Syncing,
    Applying,
    WarmingUp,
    WaitingForRenderer,
    WaitingForAssets,
    Ready,
    Degraded,
    Recovering,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScreenOverlayReason {
    Unknown,
    PlatformWindow,
    PluginDiscovery,
    AssetImport,
    ShaderCompile,
    TextureResidency,
    RenderBackendInit,
    StreamingInstall,
    StagedFilesystemApply,
    GpuDeviceLost,
    ApiCapabilityMismatch,
    JobSystem,
    Benchmark,
    Telemetry,
    Recovery,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ScreenOverlayProgress {
    pub current: u64,
    pub total: Option<u64>,
    pub percent: Option<f32>,
}

impl ScreenOverlayProgress {
    #[inline]
    pub fn percent(percent: f32) -> Self {
        Self {
            current: 0,
            total: None,
            percent: Some(percent.clamp(0.0, 1.0)),
        }
    }

    #[inline]
    pub fn ratio(current: u64, total: u64) -> Self {
        let percent = if total == 0 {
            None
        } else {
            Some((current as f32 / total as f32).clamp(0.0, 1.0))
        };
        Self {
            current,
            total: Some(total),
            percent,
        }
    }

    #[inline]
    pub fn progress_01(&self) -> f32 {
        self.percent.unwrap_or_else(|| {
            self.total
                .filter(|total| *total != 0)
                .map(|total| (self.current as f32 / total as f32).clamp(0.0, 1.0))
                .unwrap_or(0.0)
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScreenOverlayStatus {
    pub kind: ScreenOverlayStatusKind,
    pub reason: ScreenOverlayReason,
    pub title: String,
    pub status: String,
    pub detail: String,
    pub progress: Option<ScreenOverlayProgress>,
    pub terminal: bool,
}

impl ScreenOverlayStatus {
    pub fn new(
        kind: ScreenOverlayStatusKind,
        reason: ScreenOverlayReason,
        title: impl Into<String>,
        status: impl Into<String>,
        detail: impl Into<String>,
        progress: Option<ScreenOverlayProgress>,
        terminal: bool,
    ) -> Self {
        Self {
            kind,
            reason,
            title: normalize(title.into(), default_title(kind)),
            status: normalize(status.into(), "Preparing runtime..."),
            detail: normalize(detail.into(), "Runtime shell is alive."),
            progress,
            terminal,
        }
    }

    pub fn loading(
        title: impl Into<String>,
        status: impl Into<String>,
        detail: impl Into<String>,
        progress_01: f32,
    ) -> Self {
        Self::new(
            ScreenOverlayStatusKind::Loading,
            ScreenOverlayReason::Unknown,
            title,
            status,
            detail,
            Some(ScreenOverlayProgress::percent(progress_01)),
            false,
        )
    }

    pub fn ready(status: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::new(
            ScreenOverlayStatusKind::Ready,
            ScreenOverlayReason::Unknown,
            "NEWENGINE // READY",
            status,
            detail,
            Some(ScreenOverlayProgress::percent(1.0)),
            true,
        )
    }

    pub fn degraded(
        reason: ScreenOverlayReason,
        status: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self::new(
            ScreenOverlayStatusKind::Degraded,
            reason,
            "NEWENGINE // DEGRADED MODE",
            status,
            detail,
            None,
            true,
        )
    }

    pub fn error(
        reason: ScreenOverlayReason,
        status: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self::new(
            ScreenOverlayStatusKind::Error,
            reason,
            "NEWENGINE // ERROR",
            status,
            detail,
            None,
            true,
        )
    }

    #[inline]
    pub fn progress_01(&self) -> f32 {
        self.progress.as_ref().map(ScreenOverlayProgress::progress_01).unwrap_or(0.0)
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
        ScreenOverlayStatusKind::Boot => "NEWENGINE // BOOT",
        ScreenOverlayStatusKind::Loading => "NEWENGINE // BOOTSTRAP",
        ScreenOverlayStatusKind::Syncing => "NEWENGINE // SYNCHRONIZING",
        ScreenOverlayStatusKind::Applying => "NEWENGINE // APPLYING",
        ScreenOverlayStatusKind::WarmingUp => "NEWENGINE // WARMUP",
        ScreenOverlayStatusKind::WaitingForRenderer => "NEWENGINE // RENDERER",
        ScreenOverlayStatusKind::WaitingForAssets => "NEWENGINE // ASSETS",
        ScreenOverlayStatusKind::Ready => "NEWENGINE // READY",
        ScreenOverlayStatusKind::Degraded => "NEWENGINE // DEGRADED MODE",
        ScreenOverlayStatusKind::Recovering => "NEWENGINE // RECOVERY",
        ScreenOverlayStatusKind::Error => "NEWENGINE // ERROR",
    }
}
