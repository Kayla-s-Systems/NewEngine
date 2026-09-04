use newengine_loading_api::{
    EngineTaskEvent, LoadingScreenSnapshot, LoadingStatusEvent, LoadingSubsystemPhase,
    LoadingSubsystemSnapshot,
};

#[allow(clippy::too_many_arguments)]
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
        title: normalize_text(title.into(), "NORTH STAR ENGINE // BOOTSTRAP"),
        status: normalize_text(status.into(), "Preparing runtime..."),
        detail: normalize_text(
            detail.into(),
            "The loading status bridge is waiting for startup telemetry.",
        ),
        progress_01: progress_01.clamp(0.0, 1.0),
        spinner_phase,
        source: normalize_text(source.into(), "engine.ui.loading"),
        provider: normalize_text(provider.into(), "engine-loading-data"),
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
    let task_progress =
        event
            .progress_01
            .unwrap_or_else(|| if event.phase.is_terminal() { 1.0 } else { 0.0 });
    let progress = match event.phase {
        newengine_loading_api::EngineTaskPhase::Scheduled => 0.04,
        newengine_loading_api::EngineTaskPhase::Running
        | newengine_loading_api::EngineTaskPhase::Blocked
        | newengine_loading_api::EngineTaskPhase::PauseRequested
        | newengine_loading_api::EngineTaskPhase::Paused
        | newengine_loading_api::EngineTaskPhase::ResumeRequested
        | newengine_loading_api::EngineTaskPhase::CancelRequested => {
            (0.10 + task_progress * 0.72).clamp(0.0, 0.88)
        }
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
        if event.can_pause || event.can_cancel {
            LoadingSubsystemPhase::Running
        } else {
            LoadingSubsystemPhase::Waiting
        },
        if event.can_pause || event.can_cancel {
            "COOP"
        } else {
            "VIEW"
        },
        format!(
            "pause={} cancel={} id={}",
            event.can_pause,
            event.can_cancel,
            event.task_id.as_str()
        ),
        Some(task_progress),
    ));
    cards.push(LoadingSubsystemSnapshot::new(
        "events",
        "EVENT BUS",
        LoadingSubsystemPhase::Running,
        event.phase.state_label(),
        format!(
            "{} · {} · {}",
            event.source.as_str(),
            event.owner.as_str(),
            event.lane.as_str()
        ),
        Some(task_progress),
    ));

    LoadingScreenSnapshot {
        active: true,
        title: "NORTH STAR ENGINE // TASK STREAM".to_owned(),
        status: normalize_text(event.status.clone(), "Engine task is running..."),
        detail: normalize_text(
            event.detail.clone(),
            "Task telemetry is flowing through engine.task.event.v1.",
        ),
        progress_01: progress.clamp(0.0, 1.0),
        spinner_phase,
        source: normalize_text(event.source.clone(), "engine.task.event"),
        provider: normalize_text(provider.into(), "engine.ui.loading-projection"),
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
        title: normalize_text(event.title.clone(), "NORTH STAR ENGINE // BOOTSTRAP"),
        status: normalize_text(event.status.clone(), "Preparing runtime..."),
        detail: normalize_text(
            event.detail.clone(),
            "Runtime subsystem is publishing startup telemetry.",
        ),
        progress_01: event.progress_01.clamp(0.0, 1.0),
        spinner_phase,
        source: normalize_text(event.source.clone(), "engine.ui.loading.status"),
        provider: normalize_text(provider.into(), "engine.ui.loading-projection"),
        view_json: String::new(),
        subsystems,
    }
    .normalize()
}

fn status_event_subsystems(event: &LoadingStatusEvent) -> Vec<LoadingSubsystemSnapshot> {
    let event_card = event.to_subsystem_snapshot();
    let failed = matches!(
        event.phase,
        newengine_loading_api::LoadingStatusPhase::Failed
    );
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
            if event.progress_01 >= 0.34 {
                LoadingSubsystemPhase::Ready
            } else {
                LoadingSubsystemPhase::Running
            },
            if event.progress_01 >= 0.34 {
                "READY"
            } else {
                "SERVICES"
            },
            "Asset and service gateways are routed through the host bus.",
            Some((event.progress_01 / 0.34).clamp(0.0, 1.0)),
        ));
    }
    cards.push(event_card);
    cards.push(LoadingSubsystemSnapshot::new(
        "diagnostics",
        "DIAGNOSTICS",
        if failed {
            LoadingSubsystemPhase::Failed
        } else {
            LoadingSubsystemPhase::Running
        },
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

    items.iter().filter_map(parse_subsystem).take(8).collect()
}

fn parse_subsystem(value: &serde_json::Value) -> Option<LoadingSubsystemSnapshot> {
    let id = value.get("id").and_then(|v| v.as_str()).unwrap_or("Other");
    let label = value.get("label").and_then(|v| v.as_str()).unwrap_or(id);
    let phase = value
        .get("phase")
        .and_then(|v| v.as_str())
        .map(parse_phase)
        .unwrap_or_default();
    let state_label = value
        .get("state_label")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| default_phase_label(phase));
    let detail = value.get("detail").and_then(|v| v.as_str()).unwrap_or("");
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
    if trimmed.is_empty() {
        fallback.to_owned()
    } else {
        trimmed.to_owned()
    }
}
