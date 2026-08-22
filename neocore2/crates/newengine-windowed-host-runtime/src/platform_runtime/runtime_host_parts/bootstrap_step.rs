use newengine_core::host_events::{HostEvent, WindowHostEvent};
use newengine_core::EngineResult;
use newengine_platform_api::PlatformStepResultV1;

use crate::platform_runtime::bootstrap_overlay::{
    map_engine_startup_progress_to_bootstrap, RuntimeBootstrapStage, OVERLAY_LOG_PROGRESS_EPSILON,
    START_ENGINE_BOOTSTRAP_BASE_PROGRESS,
};

use super::super::HostPlatformRuntime;

impl HostPlatformRuntime {
    pub(crate) fn step_bootstrap(&mut self) -> EngineResult<PlatformStepResultV1> {
        self.bootstrap_spinner_phase = self.bootstrap_spinner_phase.wrapping_add(1);

        match self.bootstrap_stage {
            RuntimeBootstrapStage::AwaitingWindow => {
                self.set_bootstrap_overlay(
                    "Waiting for platform window...",
                    "The runtime shell is preparing the first visible frame.",
                    0.0,
                );
                Ok(self.loading_step_result())
            }
            RuntimeBootstrapStage::AnnounceLoadEnginePlugins => {
                self.set_bootstrap_overlay(
                    "Loading engine plugins...",
                    "Discovering runtime providers, services and renderer bridge.",
                    0.22,
                );
                self.bootstrap_stage = RuntimeBootstrapStage::LoadEnginePlugins;
                Ok(self.loading_step_result())
            }
            RuntimeBootstrapStage::LoadEnginePlugins => {
                match self.engine.load_engine_plugins_incremental_step() {
                    Ok(outcome) => {
                        let progress = (0.22 + outcome.progress_01 * 0.34).clamp(0.22, 0.56);
                        let detail = outcome
                            .current_path
                            .as_ref()
                            .and_then(|path| {
                                path.file_name()
                                    .map(|name| name.to_string_lossy().to_string())
                            })
                            .map(|name| format!("Loading plugin DLL '{name}'."))
                            .unwrap_or_else(|| {
                                "Discovering runtime providers, services and renderer bridge."
                                    .to_owned()
                            });

                        self.set_bootstrap_overlay(
                            format!(
                                "Loading engine plugins... ({}/{})",
                                outcome.completed, outcome.pending_total
                            ),
                            detail,
                            progress,
                        );

                        if outcome.finished {
                            let count = outcome.loaded_total;
                            newengine_ulog_api::ulog::info!(
                                "platform runtime: engine plugins init completed loaded_count={}",
                                count
                            );
                            self.loaded_engine_plugins = Some(count);
                            self.refresh_ui_provider_binding("engine-plugins-loaded");
                            self.set_bootstrap_overlay(
                                format!("Engine plugins loaded ({count})."),
                                "Runtime services are registered. Preparing startup graph.",
                                0.56,
                            );
                            self.bootstrap_stage = RuntimeBootstrapStage::AnnounceStartEngine;
                        }

                        Ok(self.loading_step_result())
                    }
                    Err(e) => {
                        newengine_ulog_api::ulog::error!(
                            "platform runtime: incremental engine plugins init failed: {}",
                            e
                        );
                        Err(e)
                    }
                }
            }
            RuntimeBootstrapStage::AnnounceStartEngine => {
                self.set_bootstrap_overlay(
                    "Starting engine modules...",
                    "Dispatching startup graph, readiness gates and scene bootstrap.",
                    0.74,
                );
                self.bootstrap_stage = RuntimeBootstrapStage::StartEngine;
                Ok(self.loading_step_result())
            }
            RuntimeBootstrapStage::StartEngine => match self.engine.start_incremental_step() {
                Ok(outcome) => {
                    let snapshot = outcome.snapshot;
                    let overlay_progress =
                        map_engine_startup_progress_to_bootstrap(snapshot.progress_01)
                            .clamp(START_ENGINE_BOOTSTRAP_BASE_PROGRESS, 0.94);
                    self.set_bootstrap_overlay(
                        snapshot.status.clone(),
                        snapshot.detail.clone(),
                        overlay_progress,
                    );

                    if outcome.finished {
                        self.started = true;
                        newengine_ulog_api::ulog::info!(
                            "platform runtime: engine.start incremental pump completed"
                        );
                        self.set_bootstrap_overlay(
                            "Engine runtime started.",
                            "Finalizing gated scene readiness and host window events.",
                            0.90,
                        );
                        self.bootstrap_stage = RuntimeBootstrapStage::AnnounceEnterRuntime;
                    }

                    Ok(self.loading_step_result())
                }
                Err(e) => {
                    newengine_ulog_api::ulog::error!(
                        "platform runtime: engine.start incremental pump failed: {}",
                        e
                    );
                    Err(e)
                }
            },
            RuntimeBootstrapStage::AnnounceEnterRuntime => {
                self.set_bootstrap_overlay(
                    "Preparing playable world...",
                    "Native loading remains active while scene resources become resident.",
                    0.95,
                );
                self.bootstrap_stage = RuntimeBootstrapStage::EmitWindowReady;
                Ok(self.loading_step_result())
            }
            RuntimeBootstrapStage::EmitWindowReady => {
                self.emit_window_ready_event()?;
                self.window_ready_emitted = true;
                self.set_bootstrap_overlay(
                "Finalizing runtime handoff...",
                "Player control and world presentation remain locked until the scene launch gate opens.",
                0.97,
            );
                self.ready_overlay_frames_left = 1;
                self.bootstrap_stage = RuntimeBootstrapStage::ReadyOverlay;
                Ok(self.loading_step_result())
            }
            RuntimeBootstrapStage::ReadyOverlay => {
                let result = self.loading_step_result();
                if self.ready_overlay_frames_left == 0 {
                    self.bootstrap_stage = RuntimeBootstrapStage::Running;
                } else {
                    self.ready_overlay_frames_left =
                        self.ready_overlay_frames_left.saturating_sub(1);
                }
                Ok(result)
            }
            RuntimeBootstrapStage::Running => self.step_running(0.0),
        }
    }

    pub(crate) fn emit_window_ready_event(&mut self) -> EngineResult<()> {
        newengine_ulog_api::ulog::info!(
            "platform runtime bootstrap: emitting WindowReady width={} height={}",
            self.surface.width,
            self.surface.height
        );
        self.engine.emit(HostEvent::Window(WindowHostEvent::Ready {
            width: self.surface.width,
            height: self.surface.height,
        }))
    }

    pub(crate) fn set_bootstrap_overlay(
        &mut self,
        status: impl Into<String>,
        detail: impl Into<String>,
        progress_01: f32,
    ) {
        let next_status = status.into();
        let next_detail = detail.into();
        let requested_progress = progress_01.clamp(0.0, 1.0);
        let next_progress = requested_progress.max(self.bootstrap_overlay.progress_01);

        let text_changed = self.bootstrap_overlay.status != next_status
            || self.bootstrap_overlay.detail != next_detail;
        let progress_changed = (self.bootstrap_overlay.progress_01 - next_progress).abs()
            >= OVERLAY_LOG_PROGRESS_EPSILON;

        self.bootstrap_overlay.status = next_status;
        self.bootstrap_overlay.detail = next_detail;
        self.bootstrap_overlay.progress_01 = next_progress;

        if text_changed || progress_changed {
            newengine_ulog_api::ulog::info!(
                "platform runtime bootstrap: overlay status='{}' detail='{}' progress={:.0}%",
                self.bootstrap_overlay.status,
                self.bootstrap_overlay.detail,
                self.bootstrap_overlay.progress_01 * 100.0
            );
        }
    }
}
