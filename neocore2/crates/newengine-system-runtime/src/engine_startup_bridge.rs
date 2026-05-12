#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::{
    EngineStartupPhase, EngineStartupSnapshot, EngineStartupSystemPhase,
    EngineStartupSystemStatus,
};
use newengine_system_contracts::{
    ScreenOverlayProgress, ScreenOverlayReason, ScreenOverlayStatus, ScreenOverlayStatusKind,
    ScreenOverlaySubsystem, ScreenOverlaySubsystemId, ScreenOverlaySubsystemPhase,
};

pub fn overlay_from_engine_startup_snapshot(
    snapshot: &EngineStartupSnapshot,
    platform_ready: bool,
    renderer_label: impl AsRef<str>,
    loaded_engine_plugins: Option<usize>,
) -> ScreenOverlayStatus {
    let renderer_label = renderer_label.as_ref();
    let subsystems = platform_subsystems(snapshot, platform_ready, renderer_label, loaded_engine_plugins);

    if snapshot.error.is_some() || snapshot.terminal && snapshot.phase == EngineStartupPhase::Faulted {
        return ScreenOverlayStatus::new(
            ScreenOverlayStatusKind::Error,
            ScreenOverlayReason::Recovery,
            "NEWENGINE // ERROR",
            snapshot.status.as_str(),
            snapshot.detail.as_str(),
            Some(ScreenOverlayProgress::percent(snapshot.progress_01)),
            true,
        )
        .with_subsystems(subsystems);
    }

    let kind = match snapshot.phase {
        EngineStartupPhase::RuntimePlugins => ScreenOverlayStatusKind::Syncing,
        EngineStartupPhase::PluginStart | EngineStartupPhase::ReadinessEvents => ScreenOverlayStatusKind::WarmingUp,
        EngineStartupPhase::Running => ScreenOverlayStatusKind::Ready,
        EngineStartupPhase::Faulted => ScreenOverlayStatusKind::Error,
        _ => ScreenOverlayStatusKind::Loading,
    };

    ScreenOverlayStatus::new(
        kind,
        reason_for_phase(snapshot.phase),
        "NEWENGINE // BOOTSTRAP",
        snapshot.status.as_str(),
        snapshot.detail.as_str(),
        Some(ScreenOverlayProgress::percent(snapshot.progress_01)),
        snapshot.terminal,
    )
    .with_subsystems(subsystems)
}

pub fn platform_subsystems(
    snapshot: &EngineStartupSnapshot,
    platform_ready: bool,
    renderer_label: &str,
    loaded_engine_plugins: Option<usize>,
) -> Vec<ScreenOverlaySubsystem> {
    let assets_detail = loaded_engine_plugins
        .map(|count| format!("{count} engine plugin service(s) loaded. AssetManager/importers are visible through host services."))
        .unwrap_or_else(|| "Waiting for AssetManager and importer services from plugin host.".to_owned());

    let assets_phase = if snapshot.error.is_some() && matches!(snapshot.phase, EngineStartupPhase::RuntimePlugins | EngineStartupPhase::PluginStart) {
        ScreenOverlaySubsystemPhase::Failed
    } else if loaded_engine_plugins.is_some() || snapshot.progress_01 >= 0.56 {
        ScreenOverlaySubsystemPhase::Ready
    } else if snapshot.progress_01 >= 0.18 {
        ScreenOverlaySubsystemPhase::Running
    } else {
        ScreenOverlaySubsystemPhase::Waiting
    };

    let simulation_phase = simulation_phase(snapshot);
    let diagnostics_phase = if snapshot.error.is_some() {
        ScreenOverlaySubsystemPhase::Failed
    } else if snapshot.terminal || snapshot.phase == EngineStartupPhase::Running {
        ScreenOverlaySubsystemPhase::Ready
    } else {
        ScreenOverlaySubsystemPhase::Running
    };

    vec![
        ScreenOverlaySubsystem::new(
            ScreenOverlaySubsystemId::Platform,
            "PLATFORM",
            if platform_ready { ScreenOverlaySubsystemPhase::Ready } else { ScreenOverlaySubsystemPhase::Running },
            if platform_ready { "READY" } else { "WINDOW" },
            if platform_ready { "Native window and surface metrics are available." } else { "Waiting for native platform window callback." },
            Some(ScreenOverlayProgress::percent(if platform_ready { 1.0 } else { 0.2 })),
        ),
        ScreenOverlaySubsystem::new(
            ScreenOverlaySubsystemId::Assets,
            "ASSETS",
            assets_phase,
            state_label_for_phase(assets_phase, if assets_phase == ScreenOverlaySubsystemPhase::Running { "SERVICES" } else { "READY" }),
            assets_detail,
            Some(ScreenOverlayProgress::percent(match assets_phase {
                ScreenOverlaySubsystemPhase::Ready => 1.0,
                ScreenOverlaySubsystemPhase::Running => snapshot.progress_01.clamp(0.0, 0.95),
                _ => 0.0,
            })),
        ),
        ScreenOverlaySubsystem::new(
            ScreenOverlaySubsystemId::Renderer,
            "RENDERER",
            if snapshot.error.is_some() && snapshot.status.to_ascii_lowercase().contains("render") {
                ScreenOverlaySubsystemPhase::Failed
            } else if snapshot.progress_01 >= 0.74 || snapshot.phase == EngineStartupPhase::Running {
                ScreenOverlaySubsystemPhase::Ready
            } else {
                ScreenOverlaySubsystemPhase::Waiting
            },
            if renderer_label.trim().is_empty() { "WAIT" } else { renderer_label },
            "Renderer backend binding is tracked through runtime resources and readiness gates.",
            Some(ScreenOverlayProgress::percent(if snapshot.progress_01 >= 0.74 { 1.0 } else { snapshot.progress_01 })),
        ),
        ScreenOverlaySubsystem::new(
            ScreenOverlaySubsystemId::Simulation,
            "SIMULATION",
            simulation_phase,
            simulation_state_label(snapshot),
            simulation_detail(snapshot),
            Some(ScreenOverlayProgress::percent(snapshot.progress_01)),
        ),
        ScreenOverlaySubsystem::new(
            ScreenOverlaySubsystemId::Diagnostics,
            "DIAGNOSTICS",
            diagnostics_phase,
            if diagnostics_phase == ScreenOverlaySubsystemPhase::Failed { "ERR" } else if diagnostics_phase == ScreenOverlaySubsystemPhase::Ready { "READY" } else { snapshot.phase.human_label() },
            diagnostics_detail(snapshot),
            Some(ScreenOverlayProgress::percent(snapshot.progress_01)),
        ),
    ]
}

#[allow(dead_code)]
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

fn simulation_phase(snapshot: &EngineStartupSnapshot) -> ScreenOverlaySubsystemPhase {
    if snapshot.error.is_some() {
        ScreenOverlaySubsystemPhase::Failed
    } else if snapshot.phase == EngineStartupPhase::Running || snapshot.progress_01 >= 0.94 {
        ScreenOverlaySubsystemPhase::Ready
    } else if matches!(snapshot.phase, EngineStartupPhase::GameInit | EngineStartupPhase::ModuleOrder | EngineStartupPhase::ModuleInit | EngineStartupPhase::StartupGraph | EngineStartupPhase::ReadinessEvents) {
        ScreenOverlaySubsystemPhase::Running
    } else {
        ScreenOverlaySubsystemPhase::Waiting
    }
}

fn simulation_state_label(snapshot: &EngineStartupSnapshot) -> &'static str {
    if snapshot.error.is_some() {
        "ERR"
    } else if snapshot.phase == EngineStartupPhase::Running || snapshot.progress_01 >= 0.94 {
        "READY"
    } else {
        match snapshot.phase {
            EngineStartupPhase::GameInit => "INIT",
            EngineStartupPhase::ModuleOrder => "ORDER",
            EngineStartupPhase::ModuleInit => "MODULES",
            EngineStartupPhase::StartupGraph => "GRAPH",
            EngineStartupPhase::ReadinessEvents => "EVENTS",
            _ => "WAIT",
        }
    }
}

fn simulation_detail(snapshot: &EngineStartupSnapshot) -> String {
    match snapshot.current_module.as_deref() {
        Some(module) => format!("Core FSM='{}'; current module='{}' ({}/{}).", snapshot.run_state, module, snapshot.module_index, snapshot.module_total),
        None => format!("Core FSM='{}'; startup phase='{}'.", snapshot.run_state, snapshot.phase.as_str()),
    }
}

fn diagnostics_detail(snapshot: &EngineStartupSnapshot) -> String {
    match snapshot.error.as_deref() {
        Some(error) => format!("{} // {}", snapshot.detail, error),
        None => format!("{} // phase={} progress={:.0}%", snapshot.detail, snapshot.phase.as_str(), snapshot.progress_01 * 100.0),
    }
}

fn state_label_for_phase(phase: ScreenOverlaySubsystemPhase, running_label: &'static str) -> &'static str {
    match phase {
        ScreenOverlaySubsystemPhase::Waiting => "WAIT",
        ScreenOverlaySubsystemPhase::Running => running_label,
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

fn reason_for_phase(phase: EngineStartupPhase) -> ScreenOverlayReason {
    match phase {
        EngineStartupPhase::RuntimePlugins | EngineStartupPhase::PluginStart => ScreenOverlayReason::PluginDiscovery,
        EngineStartupPhase::ModuleInit | EngineStartupPhase::StartupGraph | EngineStartupPhase::ReadinessEvents => ScreenOverlayReason::JobSystem,
        EngineStartupPhase::Faulted => ScreenOverlayReason::Recovery,
        _ => ScreenOverlayReason::PlatformWindow,
    }
}
