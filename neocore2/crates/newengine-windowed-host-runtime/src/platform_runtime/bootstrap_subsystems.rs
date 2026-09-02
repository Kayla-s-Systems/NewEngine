use newengine_core::render::SceneLaunchStatus;
use newengine_system_contracts::{ScreenOverlaySubsystem, ScreenOverlaySubsystemId};

use crate::platform_runtime::bootstrap_overlay::{
    subsystem_failed, subsystem_ready, subsystem_run, subsystem_wait, RuntimeBootstrapStage,
};

pub(crate) struct BootstrapSubsystemInput<'a> {
    pub fatal_error: Option<&'a str>,
    pub render_backend: String,
    pub loaded_engine_plugins: Option<usize>,
    pub bootstrap_stage: RuntimeBootstrapStage,
    pub bootstrap_progress: f32,
}

pub(crate) struct SceneLaunchSubsystemInput<'a> {
    pub status: &'a SceneLaunchStatus,
    pub render_backend: String,
}

pub(crate) fn build_bootstrap_subsystems(
    input: BootstrapSubsystemInput<'_>,
) -> Vec<ScreenOverlaySubsystem> {
    if let Some(error) = input.fatal_error {
        return vec![
            subsystem_ready(
                ScreenOverlaySubsystemId::Platform,
                "READY",
                "Native window remained alive for safe-stop diagnostics.",
            ),
            subsystem_ready(
                ScreenOverlaySubsystemId::Assets,
                "READY",
                "Asset service state was already published before the failure, or is not the failing gate.",
            ),
            subsystem_run(
                ScreenOverlaySubsystemId::Renderer,
                input.render_backend,
                "Renderer state is preserved while the loading screen reports the failure.",
                None,
            ),
            subsystem_failed(
                ScreenOverlaySubsystemId::Simulation,
                "ERR",
                "Engine startup FSM did not reach playable runtime.",
            ),
            subsystem_failed(ScreenOverlaySubsystemId::Diagnostics, "ERR", error),
        ];
    }

    let plugin_detail = input
        .loaded_engine_plugins
        .map(|count| format!("{count} engine plugin service(s) loaded."))
        .unwrap_or_else(|| "Engine plugin services are not loaded yet.".to_owned());

    match input.bootstrap_stage {
        RuntimeBootstrapStage::AwaitingWindow | RuntimeBootstrapStage::StartupIntro => {
            awaiting_window_subsystems()
        }
        RuntimeBootstrapStage::AnnounceLoadEnginePlugins
        | RuntimeBootstrapStage::LoadEnginePlugins => {
            loading_engine_plugins_subsystems(input.bootstrap_progress)
        }
        RuntimeBootstrapStage::AnnounceStartEngine | RuntimeBootstrapStage::StartEngine => {
            starting_engine_subsystems(
                input.render_backend,
                plugin_detail,
                input.bootstrap_progress,
            )
        }
        RuntimeBootstrapStage::AnnounceEnterRuntime
        | RuntimeBootstrapStage::EmitWindowReady
        | RuntimeBootstrapStage::ReadyOverlay => runtime_handoff_subsystems(
            input.render_backend,
            plugin_detail,
            input.bootstrap_progress,
        ),
        RuntimeBootstrapStage::Running => running_subsystems(input.render_backend),
    }
}

pub(crate) fn build_scene_launch_subsystems(
    input: SceneLaunchSubsystemInput<'_>,
) -> Vec<ScreenOverlaySubsystem> {
    let progress = input.status.progress_01.clamp(0.0, 0.995);
    let assets_ready =
        progress >= 0.96 || !input.status.detail.to_ascii_lowercase().contains("waiting");
    let simulation_ready = progress >= 0.90;

    vec![
        subsystem_ready(
            ScreenOverlaySubsystemId::Platform,
            "READY",
            "Platform window remains alive while launch gate is active.",
        ),
        if assets_ready {
            subsystem_ready(
                ScreenOverlaySubsystemId::Assets,
                "READY",
                input.status.detail.clone(),
            )
        } else {
            subsystem_run(
                ScreenOverlaySubsystemId::Assets,
                "STREAMING",
                input.status.detail.clone(),
                Some(progress),
            )
        },
        subsystem_ready(
            ScreenOverlaySubsystemId::Renderer,
            input.render_backend,
            "Renderer backend accepted the launch scene frame package.",
        ),
        if simulation_ready {
            subsystem_ready(
                ScreenOverlaySubsystemId::Simulation,
                "READY",
                "Simulation handoff is ready for playable control.",
            )
        } else {
            subsystem_run(
                ScreenOverlaySubsystemId::Simulation,
                "LOCKED",
                "Player control is locked until the scene launch gate opens.",
                Some(progress),
            )
        },
        subsystem_run(
            ScreenOverlaySubsystemId::Diagnostics,
            "CHECKING",
            input.status.status.clone(),
            Some(progress),
        ),
    ]
}

fn awaiting_window_subsystems() -> Vec<ScreenOverlaySubsystem> {
    vec![
        subsystem_wait(
            ScreenOverlaySubsystemId::Platform,
            "WINDOW",
            "Waiting for the platform window callback.",
        ),
        subsystem_wait(
            ScreenOverlaySubsystemId::Assets,
            "WAIT",
            "AssetManager is not guaranteed to be online yet.",
        ),
        subsystem_wait(
            ScreenOverlaySubsystemId::Renderer,
            "WAIT",
            "Renderer backend starts after window handles are published.",
        ),
        subsystem_wait(
            ScreenOverlaySubsystemId::Simulation,
            "WAIT",
            "Simulation modules are blocked by bootstrap.",
        ),
        subsystem_run(
            ScreenOverlaySubsystemId::Diagnostics,
            "BOOT",
            "Runtime-host bootstrap is alive.",
            None,
        ),
    ]
}

fn loading_engine_plugins_subsystems(progress: f32) -> Vec<ScreenOverlaySubsystem> {
    vec![
        subsystem_ready(
            ScreenOverlaySubsystemId::Platform,
            "READY",
            "Native window and surface metrics are available.",
        ),
        subsystem_run(
            ScreenOverlaySubsystemId::Assets,
            "SERVICES",
            "Loading AssetManager/importer services through plugin host.",
            Some(progress),
        ),
        subsystem_wait(
            ScreenOverlaySubsystemId::Renderer,
            "WAIT",
            "Renderer backend is waiting for engine plugin services.",
        ),
        subsystem_wait(
            ScreenOverlaySubsystemId::Simulation,
            "WAIT",
            "Simulation starts after engine plugin discovery.",
        ),
        subsystem_run(
            ScreenOverlaySubsystemId::Diagnostics,
            "CHECKING",
            "Plugin discovery and capability checks are running.",
            None,
        ),
    ]
}

fn starting_engine_subsystems(
    render_backend: String,
    plugin_detail: String,
    progress: f32,
) -> Vec<ScreenOverlaySubsystem> {
    vec![
        subsystem_ready(
            ScreenOverlaySubsystemId::Platform,
            "READY",
            "Native window and surface metrics are available.",
        ),
        subsystem_ready(ScreenOverlaySubsystemId::Assets, "READY", plugin_detail),
        subsystem_run(
            ScreenOverlaySubsystemId::Renderer,
            render_backend,
            "Renderer backend is being bound to runtime resources.",
            None,
        ),
        subsystem_run(
            ScreenOverlaySubsystemId::Simulation,
            "STARTING",
            "Engine startup graph is dispatching modules.",
            Some(progress),
        ),
        subsystem_run(
            ScreenOverlaySubsystemId::Diagnostics,
            "CHECKING",
            "Startup readiness gates are being evaluated.",
            None,
        ),
    ]
}

fn runtime_handoff_subsystems(
    render_backend: String,
    plugin_detail: String,
    progress: f32,
) -> Vec<ScreenOverlaySubsystem> {
    vec![
        subsystem_ready(
            ScreenOverlaySubsystemId::Platform,
            "READY",
            "WindowReady event is emitted to the engine host.",
        ),
        subsystem_ready(ScreenOverlaySubsystemId::Assets, "READY", plugin_detail),
        subsystem_ready(
            ScreenOverlaySubsystemId::Renderer,
            render_backend,
            "Renderer backend is available for the first world frame.",
        ),
        subsystem_run(
            ScreenOverlaySubsystemId::Simulation,
            "HANDOFF",
            "Scene launch gate owns final playable-world readiness.",
            Some(progress),
        ),
        subsystem_run(
            ScreenOverlaySubsystemId::Diagnostics,
            "CHECKING",
            "Final handoff diagnostics are collecting runtime status.",
            None,
        ),
    ]
}

fn running_subsystems(render_backend: String) -> Vec<ScreenOverlaySubsystem> {
    vec![
        subsystem_ready(
            ScreenOverlaySubsystemId::Platform,
            "READY",
            "Platform runtime is running.",
        ),
        subsystem_ready(
            ScreenOverlaySubsystemId::Assets,
            "READY",
            "Asset services are online.",
        ),
        subsystem_ready(
            ScreenOverlaySubsystemId::Renderer,
            render_backend,
            "Renderer backend is active.",
        ),
        subsystem_ready(
            ScreenOverlaySubsystemId::Simulation,
            "READY",
            "Simulation is accepting frame ticks.",
        ),
        subsystem_ready(
            ScreenOverlaySubsystemId::Diagnostics,
            "READY",
            "Bootstrap diagnostics are complete.",
        ),
    ]
}
