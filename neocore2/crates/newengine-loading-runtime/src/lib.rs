#![forbid(unsafe_op_in_unsafe_fn)]

use std::f32::consts::TAU;
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub use newengine_loading_api::{
    EngineTaskEvent, LoadingScreenSnapshot, LoadingStatusEvent, LoadingSubsystemPhase, LoadingSubsystemSnapshot,
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
    visible_detail_key: String,
    visible_title: String,
    visible_status: String,
    visible_detail: String,
    visual_progress_01: f32,
}

const MIN_STATUS_HOLD_SECS: f32 = 0.55;

impl Default for LoadingAnimator {
    #[inline]
    fn default() -> Self {
        let now = Instant::now();
        Self {
            created_at: now,
            last_present_at: now,
            detail_started_at: now,
            last_detail_key: String::new(),
            visible_detail_key: String::new(),
            visible_title: String::new(),
            visible_status: String::new(),
            visible_detail: String::new(),
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

        let mut next = snapshot.clone().normalize();
        let detail_key = format!("{}\n{}\n{}", next.title, next.status, next.detail);
        if self.last_detail_key != detail_key {
            self.last_detail_key = detail_key.clone();
        }

        let visible_age = now.saturating_duration_since(self.detail_started_at).as_secs_f32();
        let can_switch_text = self.visible_detail_key.is_empty() || visible_age >= MIN_STATUS_HOLD_SECS || !next.active;
        if self.visible_detail_key != detail_key && can_switch_text {
            self.visible_detail_key = detail_key;
            self.visible_title = next.title.clone();
            self.visible_status = next.status.clone();
            self.visible_detail = next.detail.clone();
            self.detail_started_at = now;
        } else if !self.visible_detail_key.is_empty() && self.visible_detail_key != detail_key {
            next.title = self.visible_title.clone();
            next.status = self.visible_status.clone();
            next.detail = self.visible_detail.clone();
        }

        let target = next.progress_01.clamp(0.0, 1.0);
        let target = if next.active { target.max(self.visual_progress_01) } else { target };
        let speed = if target < self.visual_progress_01 { 16.0 } else { 5.6 };
        let alpha = 1.0 - (-speed * dt).exp();
        self.visual_progress_01 += (target - self.visual_progress_01) * alpha;
        self.visual_progress_01 = self.visual_progress_01.clamp(0.0, 1.0);

        let age = now.saturating_duration_since(self.created_at).as_secs_f32();
        // Spinner animation must be driven by compositor-local monotonic time,
        // not by task/status events. Event-driven phases arrive irregularly during
        // heavy startup work, which made the spinner advance in visible bursts.
        // The bus-provided phase is kept only as a tiny deterministic offset so
        // status publishers can still decorrelate multiple loading surfaces.
        let spinner_phase_offset = ((next.spinner_phase as f32 % 144.0) / 144.0) * TAU;
        let spinner_angle_rad = (spinner_phase_offset + age * 1.35 * TAU) % TAU;
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

#[derive(Clone)]
pub struct SharedLoadingSnapshot {
    inner: Arc<RwLock<LoadingScreenSnapshot>>,
    version: Arc<AtomicU64>,
    wake: Arc<(Mutex<u64>, Condvar)>,
}

impl Default for SharedLoadingSnapshot {
    #[inline]
    fn default() -> Self {
        Self::new(LoadingScreenSnapshot::inactive())
    }
}

impl SharedLoadingSnapshot {
    #[inline]
    pub fn new(initial: LoadingScreenSnapshot) -> Self {
        Self {
            inner: Arc::new(RwLock::new(initial.normalize())),
            version: Arc::new(AtomicU64::new(1)),
            wake: Arc::new((Mutex::new(1), Condvar::new())),
        }
    }

    #[inline]
    pub fn publish(&self, snapshot: LoadingScreenSnapshot) {
        let mut next = snapshot.normalize();
        match self.inner.write() {
            Ok(mut guard) => {
                if next.active && guard.active {
                    next.progress_01 = next.progress_01.max(guard.progress_01);
                }
                *guard = next;
            }
            Err(e) => {
                let mut guard = e.into_inner();
                if next.active && guard.active {
                    next.progress_01 = next.progress_01.max(guard.progress_01);
                }
                *guard = next;
            }
        }
        let version = self.version.fetch_add(1, Ordering::AcqRel) + 1;
        let (lock, cv) = &*self.wake;
        match lock.lock() {
            Ok(mut guard) => *guard = version,
            Err(e) => *e.into_inner() = version,
        }
        cv.notify_all();
    }

    #[inline]
    pub fn snapshot(&self) -> LoadingScreenSnapshot {
        match self.inner.read() {
            Ok(guard) => guard.clone(),
            Err(e) => e.into_inner().clone(),
        }
    }

    #[inline]
    pub fn version(&self) -> u64 {
        self.version.load(Ordering::Acquire)
    }

    pub fn wait_for_update_or_timeout(&self, observed_version: u64, timeout: Duration) -> u64 {
        if self.version() != observed_version {
            return self.version();
        }
        let (lock, cv) = &*self.wake;
        let guard = match lock.lock() {
            Ok(guard) => guard,
            Err(e) => e.into_inner(),
        };
        let result = cv
            .wait_timeout_while(guard, timeout, |version| *version == observed_version)
            .unwrap_or_else(|e| e.into_inner());
        *result.0
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



pub fn project_loading_snapshot_from_task_event(
    event: EngineTaskEvent,
    spinner_phase: u32,
    provider: impl Into<String>,
) -> LoadingScreenSnapshot {
    let task_progress = event.progress_01.unwrap_or_else(|| {
        if event.phase.is_terminal() { 1.0 } else { 0.0 }
    });
    let progress = match event.phase {
        newengine_loading_api::EngineTaskPhase::Scheduled => 0.04,
        newengine_loading_api::EngineTaskPhase::Running
        | newengine_loading_api::EngineTaskPhase::Blocked
        | newengine_loading_api::EngineTaskPhase::PauseRequested
        | newengine_loading_api::EngineTaskPhase::Paused
        | newengine_loading_api::EngineTaskPhase::ResumeRequested
        | newengine_loading_api::EngineTaskPhase::CancelRequested => (0.10 + task_progress * 0.72).clamp(0.0, 0.88),
        newengine_loading_api::EngineTaskPhase::Completed
        | newengine_loading_api::EngineTaskPhase::Cancelled => 0.88,
        newengine_loading_api::EngineTaskPhase::Failed => 0.88,
    };
    let mut cards = Vec::with_capacity(5);
    cards.push(LoadingSubsystemSnapshot::new(
        "platform",
        "PLATFORM",
        LoadingSubsystemPhase::Ready,
        "LIVE",
        "Native compositor is rendering independently from startup tasks.",
        Some(1.0),
    ));
    cards.push(event.to_subsystem_snapshot());
    cards.push(LoadingSubsystemSnapshot::new(
        "task-control",
        "TASK CTRL",
        if event.can_pause || event.can_cancel { LoadingSubsystemPhase::Running } else { LoadingSubsystemPhase::Waiting },
        if event.can_pause || event.can_cancel { "COOP" } else { "VIEW" },
        format!("pause={} cancel={} id={}", event.can_pause, event.can_cancel, event.task_id.as_str()),
        Some(task_progress),
    ));
    cards.push(LoadingSubsystemSnapshot::new(
        "events",
        "EVENT BUS",
        LoadingSubsystemPhase::Running,
        event.phase.state_label(),
        format!("{} · {} · {}", event.source.as_str(), event.owner.as_str(), event.lane.as_str()),
        Some(task_progress),
    ));

    LoadingScreenSnapshot {
        active: true,
        title: "NEWENGINE // TASK STREAM".to_owned(),
        status: normalize_text(event.status.clone(), "Engine task is running..."),
        detail: normalize_text(event.detail.clone(), "Task telemetry is flowing through engine.task.event.v1."),
        progress_01: progress.clamp(0.0, 1.0),
        spinner_phase,
        source: normalize_text(event.source.clone(), "engine.task.event"),
        provider: normalize_text(provider.into(), "engine-owned-native-shell"),
        view_json: String::new(),
        subsystems: cards,
    }
    .normalize()
}

pub fn project_loading_snapshot_from_status_event(
    event: LoadingStatusEvent,
    spinner_phase: u32,
    provider: impl Into<String>,
) -> LoadingScreenSnapshot {
    let subsystems = status_event_subsystems(&event);
    LoadingScreenSnapshot {
        active: true,
        title: normalize_text(event.title.clone(), "NEWENGINE // BOOTSTRAP"),
        status: normalize_text(event.status.clone(), "Preparing runtime..."),
        detail: normalize_text(event.detail.clone(), "Runtime subsystem is publishing startup telemetry."),
        progress_01: event.progress_01.clamp(0.0, 1.0),
        spinner_phase,
        source: normalize_text(event.source.clone(), "engine.loading.status"),
        provider: normalize_text(provider.into(), "engine-owned-native-shell"),
        view_json: String::new(),
        subsystems,
    }
    .normalize()
}

fn status_event_subsystems(event: &LoadingStatusEvent) -> Vec<LoadingSubsystemSnapshot> {
    let event_card = event.to_subsystem_snapshot();
    let failed = matches!(event.phase, newengine_loading_api::LoadingStatusPhase::Failed);
    let mut cards = Vec::with_capacity(4);
    if event.subsystem_id != "platform" {
        cards.push(LoadingSubsystemSnapshot::new(
            "platform",
            "PLATFORM",
            LoadingSubsystemPhase::Ready,
            "READY",
            "Native window and loading compositor are alive.",
            Some(1.0),
        ));
    }
    if event.subsystem_id != "assets" {
        cards.push(LoadingSubsystemSnapshot::new(
            "assets",
            "ASSETS",
            if event.progress_01 >= 0.34 { LoadingSubsystemPhase::Ready } else { LoadingSubsystemPhase::Running },
            if event.progress_01 >= 0.34 { "READY" } else { "SERVICES" },
            "Asset and service gateways are routed through the host bus.",
            Some((event.progress_01 / 0.34).clamp(0.0, 1.0)),
        ));
    }
    cards.push(event_card);
    cards.push(LoadingSubsystemSnapshot::new(
        "diagnostics",
        "DIAGNOSTICS",
        if failed { LoadingSubsystemPhase::Failed } else { LoadingSubsystemPhase::Running },
        if failed { "ERR" } else { "EVENT" },
        format!("{} · {}", event.domain, event.source),
        Some(event.progress_01),
    ));
    cards
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
