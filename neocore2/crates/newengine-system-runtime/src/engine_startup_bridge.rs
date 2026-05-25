#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::{
    EngineStartupPhase, EngineStartupSnapshot, EngineStartupSystemPhase,
    EngineStartupSystemStatus,
};
use newengine_system_contracts::{
    ScreenOverlayProgress, ScreenOverlayReason, ScreenOverlayStatus, ScreenOverlayStatusKind,
    ScreenOverlaySubsystem, ScreenOverlaySubsystemId, ScreenOverlaySubsystemPhase,
};

/// Converts the core-owned startup snapshot into the platform overlay model.
///
/// Important boundary rule: `newengine-core` owns the lifecycle truth and the
/// per-system startup facts. This bridge may add platform/runtime-host facts
/// that core cannot know about, but it must not reconstruct core subsystem state
/// from percentages.
pub fn overlay_from_engine_startup_snapshot(
    snapshot: &EngineStartupSnapshot,
    platform_ready: bool,
    renderer_label: impl AsRef<str>,
    loaded_engine_plugins: Option<usize>,
) -> ScreenOverlayStatus {
    let subsystems = platform_subsystems(
        snapshot,
        platform_ready,
        renderer_label.as_ref(),
        loaded_engine_plugins,
    );

    if snapshot.error.is_some()
        || snapshot.terminal && snapshot.phase == EngineStartupPhase::Faulted
    {
        return ScreenOverlayStatus::new(
            ScreenOverlayStatusKind::Error,
            ScreenOverlayReason::Recovery,
            "NORTH STAR ENGINE // ERROR",
            snapshot.status.as_str(),
            snapshot.detail.as_str(),
            Some(ScreenOverlayProgress::percent(snapshot.progress_01)),
            true,
        )
        .with_subsystems(subsystems);
    }

    ScreenOverlayStatus::new(
        overlay_kind_for_phase(snapshot.phase),
        reason_for_phase(snapshot.phase),
        "NORTH STAR ENGINE // BOOTSTRAP",
        snapshot.status.as_str(),
        snapshot.detail.as_str(),
        Some(ScreenOverlayProgress::percent(snapshot.progress_01)),
        snapshot.terminal,
    )
    .with_subsystems(subsystems)
}

/// Platform/runtime-host facts + core FSM facts, in that order.
///
/// The old implementation duplicated the startup graph by deriving ASSETS,
/// SIMULATION and DIAGNOSTICS from `progress_01` thresholds. That made the
/// loading screen a second lifecycle model. Now the bridge preserves
/// `EngineStartupSnapshot::systems` and only injects the two facts that are not
/// owned by core: native window readiness and selected renderer backend label.
pub fn platform_subsystems(
    snapshot: &EngineStartupSnapshot,
    platform_ready: bool,
    renderer_label: &str,
    loaded_engine_plugins: Option<usize>,
) -> Vec<ScreenOverlaySubsystem> {
    let mut subsystems = Vec::with_capacity(snapshot.systems.len() + 2);
    subsystems.push(platform_subsystem(platform_ready));
    subsystems.push(renderer_subsystem(snapshot, renderer_label));
    subsystems.extend(snapshot.systems.iter().map(subsystem_from_engine_system));

    if let Some(count) = loaded_engine_plugins {
        attach_plugin_service_count(&mut subsystems, count);
    }

    subsystems
}

pub fn subsystem_from_engine_system(system: &EngineStartupSystemStatus) -> ScreenOverlaySubsystem {
    ScreenOverlaySubsystem::new(
        subsystem_id_from_core_id(system.id.as_str()),
        system.label.as_str(),
        phase_from_core(system.phase),
        system.state_label.as_str(),
        system.detail.as_str(),
        system.progress_01.map(ScreenOverlayProgress::percent),
    )
}

fn platform_subsystem(platform_ready: bool) -> ScreenOverlaySubsystem {
    ScreenOverlaySubsystem::new(
        ScreenOverlaySubsystemId::Platform,
        ScreenOverlaySubsystemId::Platform.default_label(),
        if platform_ready {
            ScreenOverlaySubsystemPhase::Ready
        } else {
            ScreenOverlaySubsystemPhase::Running
        },
        if platform_ready { "READY" } else { "WINDOW" },
        if platform_ready {
            "Native window and surface metrics are available."
        } else {
            "Waiting for native platform window callback."
        },
        Some(ScreenOverlayProgress::percent(if platform_ready { 1.0 } else { 0.2 })),
    )
}

fn renderer_subsystem(snapshot: &EngineStartupSnapshot, renderer_label: &str) -> ScreenOverlaySubsystem {
    let renderer_label = normalize_label(renderer_label, "WAIT");
    let failed = snapshot.error.is_some()
        && snapshot.status.to_ascii_lowercase().contains("render");
    let ready = snapshot.phase == EngineStartupPhase::Running || snapshot.progress_01 >= 0.92;

    let phase = if failed {
        ScreenOverlaySubsystemPhase::Failed
    } else if ready {
        ScreenOverlaySubsystemPhase::Ready
    } else {
        ScreenOverlaySubsystemPhase::Waiting
    };

    ScreenOverlaySubsystem::new(
        ScreenOverlaySubsystemId::Renderer,
        ScreenOverlaySubsystemId::Renderer.default_label(),
        phase,
        state_label_for_phase(phase, renderer_label),
        "Renderer backend binding is tracked through runtime resources and readiness gates.",
        Some(ScreenOverlayProgress::percent(if ready { 1.0 } else { snapshot.progress_01 })),
    )
}

fn attach_plugin_service_count(subsystems: &mut [ScreenOverlaySubsystem], count: usize) {
    let Some(assets) = subsystems
        .iter_mut()
        .find(|s| s.id == ScreenOverlaySubsystemId::Assets)
    else {
        return;
    };

    assets.detail = format!(
        "{count} engine plugin service(s) loaded. AssetManager/importers are visible through host services."
    );

    if matches!(
        assets.phase,
        ScreenOverlaySubsystemPhase::Waiting | ScreenOverlaySubsystemPhase::Running
    ) {
        assets.phase = ScreenOverlaySubsystemPhase::Ready;
        assets.state_label = "READY".to_owned();
        assets.progress = Some(ScreenOverlayProgress::percent(1.0));
    }
}

#[inline]
fn normalize_label<'a>(value: &'a str, fallback: &'static str) -> &'a str {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback
    } else {
        trimmed
    }
}

fn state_label_for_phase<'a>(
    phase: ScreenOverlaySubsystemPhase,
    running_label: &'a str,
) -> &'a str {
    match phase {
        ScreenOverlaySubsystemPhase::Waiting | ScreenOverlaySubsystemPhase::Running => running_label,
        ScreenOverlaySubsystemPhase::Ready => "READY",
        ScreenOverlaySubsystemPhase::Degraded => "DEGRADED",
        ScreenOverlaySubsystemPhase::Failed => "ERR",
    }
}

fn phase_from_core(phase: EngineStartupSystemPhase) -> ScreenOverlaySubsystemPhase {
    match phase {
        EngineStartupSystemPhase::Waiting => ScreenOverlaySubsystemPhase::Waiting,
        EngineStartupSystemPhase::Running => ScreenOverlaySubsystemPhase::Running,
        EngineStartupSystemPhase::Ready => ScreenOverlaySubsystemPhase::Ready,
        EngineStartupSystemPhase::Degraded => ScreenOverlaySubsystemPhase::Degraded,
        EngineStartupSystemPhase::Failed => ScreenOverlaySubsystemPhase::Failed,
    }
}

fn subsystem_id_from_core_id(id: &str) -> ScreenOverlaySubsystemId {
    match id.to_ascii_lowercase().as_str() {
        "platform" => ScreenOverlaySubsystemId::Platform,
        "assets" | "plugins" => ScreenOverlaySubsystemId::Assets,
        "renderer" => ScreenOverlaySubsystemId::Renderer,
        "simulation" | "modules" | "readiness" | "fsm" => ScreenOverlaySubsystemId::Simulation,
        "diagnostics" => ScreenOverlaySubsystemId::Diagnostics,
        _ => ScreenOverlaySubsystemId::Other,
    }
}

fn overlay_kind_for_phase(phase: EngineStartupPhase) -> ScreenOverlayStatusKind {
    match phase {
        EngineStartupPhase::RuntimePlugins => ScreenOverlayStatusKind::Syncing,
        EngineStartupPhase::PluginStart | EngineStartupPhase::ReadinessEvents => {
            ScreenOverlayStatusKind::WarmingUp
        }
        EngineStartupPhase::Running => ScreenOverlayStatusKind::Ready,
        EngineStartupPhase::Faulted => ScreenOverlayStatusKind::Error,
        _ => ScreenOverlayStatusKind::Loading,
    }
}

fn reason_for_phase(phase: EngineStartupPhase) -> ScreenOverlayReason {
    match phase {
        EngineStartupPhase::RuntimePlugins | EngineStartupPhase::PluginStart => {
            ScreenOverlayReason::PluginDiscovery
        }
        EngineStartupPhase::ModuleInit
        | EngineStartupPhase::StartupGraph
        | EngineStartupPhase::ReadinessEvents => ScreenOverlayReason::JobSystem,
        EngineStartupPhase::Faulted => ScreenOverlayReason::Recovery,
        _ => ScreenOverlayReason::PlatformWindow,
    }
}
