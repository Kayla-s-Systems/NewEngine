#![forbid(unsafe_op_in_unsafe_fn)]

use std::f32::consts::TAU;
use std::sync::{Arc, RwLock};
use std::time::Instant;

pub use newengine_loading_api::{
    LoadingScreenSnapshot, LoadingSubsystemPhase, LoadingSubsystemSnapshot,
};

#[derive(Debug, Clone)]
pub struct LoadingCompositorFrame {
    pub snapshot: LoadingScreenSnapshot,
    pub visual_progress_01: f32,
    pub spinner_angle_rad: f32,
    pub pulse_01: f32,
    pub age_secs: f32,
    pub detail_age_secs: f32,
}

#[derive(Debug, Clone)]
pub struct LoadingAnimator {
    created_at: Instant,
    last_present_at: Instant,
    detail_started_at: Instant,
    last_detail_key: String,
    visual_progress_01: f32,
}

impl Default for LoadingAnimator {
    #[inline]
    fn default() -> Self {
        let now = Instant::now();
        Self {
            created_at: now,
            last_present_at: now,
            detail_started_at: now,
            last_detail_key: String::new(),
            visual_progress_01: 0.0,
        }
    }
}

impl LoadingAnimator {
    pub fn present(&mut self, snapshot: &LoadingScreenSnapshot) -> LoadingCompositorFrame {
        let now = Instant::now();
        let dt = now
            .saturating_duration_since(self.last_present_at)
            .as_secs_f32()
            .clamp(1.0 / 240.0, 1.0 / 20.0);
        self.last_present_at = now;

        let next = snapshot.clone().normalize();
        let detail_key = format!("{}\n{}\n{}", next.title, next.status, next.detail);
        if self.last_detail_key != detail_key {
            self.last_detail_key = detail_key;
            self.detail_started_at = now;
        }

        let target = next.progress_01.clamp(0.0, 1.0);
        let speed = if target < self.visual_progress_01 { 16.0 } else { 7.5 };
        let alpha = 1.0 - (-speed * dt).exp();
        self.visual_progress_01 += (target - self.visual_progress_01) * alpha;
        self.visual_progress_01 = self.visual_progress_01.clamp(0.0, 1.0);

        let age = now.saturating_duration_since(self.created_at).as_secs_f32();
        let spinner_angle_rad = if next.spinner_phase == 0 {
            (age * 0.72 * TAU) % TAU
        } else {
            ((next.spinner_phase as f32 % 144.0) / 144.0) * TAU
        };
        let pulse_01 = ((age * 1.25 * TAU).sin() * 0.5 + 0.5).clamp(0.0, 1.0);

        LoadingCompositorFrame {
            snapshot: next,
            visual_progress_01: self.visual_progress_01,
            spinner_angle_rad,
            pulse_01,
            age_secs: age,
            detail_age_secs: now
                .saturating_duration_since(self.detail_started_at)
                .as_secs_f32(),
        }
    }
}

#[derive(Clone, Default)]
pub struct SharedLoadingSnapshot {
    inner: Arc<RwLock<LoadingScreenSnapshot>>,
}

impl SharedLoadingSnapshot {
    #[inline]
    pub fn new(initial: LoadingScreenSnapshot) -> Self {
        Self { inner: Arc::new(RwLock::new(initial.normalize())) }
    }

    #[inline]
    pub fn publish(&self, snapshot: LoadingScreenSnapshot) {
        match self.inner.write() {
            Ok(mut guard) => *guard = snapshot.normalize(),
            Err(e) => *e.into_inner() = snapshot.normalize(),
        }
    }

    #[inline]
    pub fn snapshot(&self) -> LoadingScreenSnapshot {
        match self.inner.read() {
            Ok(guard) => guard.clone(),
            Err(e) => e.into_inner().clone(),
        }
    }
}

pub fn project_loading_snapshot_from_overlay_fields(
    active: bool,
    title: impl Into<String>,
    status: impl Into<String>,
    detail: impl Into<String>,
    progress_01: f32,
    spinner_phase: u32,
    view_json: impl Into<String>,
    source: impl Into<String>,
    provider: impl Into<String>,
) -> LoadingScreenSnapshot {
    let view_json = view_json.into();
    let subsystems = parse_subsystems_from_view_json(view_json.as_str());
    LoadingScreenSnapshot {
        active,
        title: normalize_text(title.into(), "NEWENGINE // BOOTSTRAP"),
        status: normalize_text(status.into(), "Preparing runtime..."),
        detail: normalize_text(detail.into(), "The native loading shell is waiting for startup telemetry."),
        progress_01: progress_01.clamp(0.0, 1.0),
        spinner_phase,
        source: normalize_text(source.into(), "engine.loading"),
        provider: normalize_text(provider.into(), "native-shell"),
        view_json,
        subsystems,
    }
    .normalize()
}

pub fn parse_subsystems_from_view_json(view_json: &str) -> Vec<LoadingSubsystemSnapshot> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(view_json) else {
        return Vec::new();
    };
    let state = value.get("state").unwrap_or(&value);
    let Some(items) = state.get("subsystems").and_then(|v| v.as_array()) else {
        return Vec::new();
    };

    items
        .iter()
        .filter_map(parse_subsystem)
        .take(8)
        .collect()
}

fn parse_subsystem(value: &serde_json::Value) -> Option<LoadingSubsystemSnapshot> {
    let id = value
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("Other");
    let label = value
        .get("label")
        .and_then(|v| v.as_str())
        .unwrap_or(id);
    let phase = value
        .get("phase")
        .and_then(|v| v.as_str())
        .map(parse_phase)
        .unwrap_or_default();
    let state_label = value
        .get("state_label")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| default_phase_label(phase));
    let detail = value
        .get("detail")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let progress_01 = value
        .get("progress")
        .and_then(|progress| {
            progress
                .get("percent")
                .and_then(|v| v.as_f64())
                .or_else(|| {
                    let current = progress.get("current")?.as_f64()?;
                    let total = progress.get("total")?.as_f64()?;
                    (total > 0.0).then_some(current / total)
                })
        })
        .map(|v| v as f32);

    Some(LoadingSubsystemSnapshot::new(
        id.to_ascii_lowercase(),
        label,
        phase,
        state_label,
        detail,
        progress_01,
    ))
}

fn parse_phase(value: &str) -> LoadingSubsystemPhase {
    match value.trim().to_ascii_lowercase().as_str() {
        "running" => LoadingSubsystemPhase::Running,
        "ready" => LoadingSubsystemPhase::Ready,
        "degraded" => LoadingSubsystemPhase::Degraded,
        "failed" | "error" | "err" => LoadingSubsystemPhase::Failed,
        _ => LoadingSubsystemPhase::Waiting,
    }
}

#[inline]
fn default_phase_label(phase: LoadingSubsystemPhase) -> &'static str {
    match phase {
        LoadingSubsystemPhase::Waiting => "WAIT",
        LoadingSubsystemPhase::Running => "RUN",
        LoadingSubsystemPhase::Ready => "READY",
        LoadingSubsystemPhase::Degraded => "DEGRADED",
        LoadingSubsystemPhase::Failed => "ERR",
    }
}

#[inline]
fn normalize_text(value: String, fallback: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() { fallback.to_owned() } else { trimmed.to_owned() }
}
