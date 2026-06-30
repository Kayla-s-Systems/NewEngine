use newengine_core::render::{RenderBackendStatus, SceneLaunchStatus};
use newengine_core::{EngineError, EngineResult};
use newengine_platform_api::PlatformStepResultV1;
use newengine_ui::{UiFrameDesc, UiProviderKind};
use newengine_ui_api::{UiEventDispatchFrame, UiInputFrame, UI_SURFACE_ENGINE_LOADING};

use crate::platform_input::poll_input_frame;
use crate::platform_runtime::bootstrap_overlay::RuntimeBootstrapStage;
use crate::render_runtime::ResolvedRenderBackendConfig;

use super::super::HostPlatformRuntime;
use super::mapping::render_backend_label_from_id;

impl HostPlatformRuntime {
    pub(crate) fn step_running(&mut self, dt_sec: f32) -> EngineResult<PlatformStepResultV1> {
        self.ui_frame_index = self.ui_frame_index.wrapping_add(1);
        let ui_frame_index = self.ui_frame_index;
        let input_frame = poll_input_frame();
        if let Some(telemetry) = self
            .engine
            .resources
            .get::<newengine_ui_api::UiRuntimeDebugOverlayTelemetry>()
            .cloned()
        {
            crate::platform_runtime::ui_gateway_frame::publish_debug_overlay_telemetry(&telemetry);
        }
        // Modal UI state is produced inside engine.step() by render_controller and
        // requires same-frame refresh. Do not publish/request the previous frame's
        // primary UI node here: that duplicates engine.ui work and forces stale UI traffic
        // before the real modal owner has updated animation/navigation state.

        let ui_dispatch_frame = if let Some(input) = input_frame.clone() {
            self.engine
                .resources_mut()
                .insert::<UiInputFrame>(input.clone());
            match crate::platform_runtime::ui_gateway_frame::dispatch_input_frame(
                ui_frame_index,
                &input,
                [self.surface.width, self.surface.height],
                self.surface.pixels_per_point,
            )? {
                Some(frame) => {
                    self.engine
                        .resources_mut()
                        .insert::<UiEventDispatchFrame>(frame.clone());
                    Some(frame)
                }
                None => {
                    let _ = self.engine.resources_mut().remove::<UiEventDispatchFrame>();
                    None
                }
            }
        } else {
            let _ = self.engine.resources_mut().remove::<UiInputFrame>();
            let _ = self.engine.resources_mut().remove::<UiEventDispatchFrame>();
            None
        };
        let ui_dispatch_refresh = ui_dispatch_frame
            .as_ref()
            .map(|frame| !frame.actions.is_empty() || !frame.state_patches.is_empty())
            .unwrap_or(false);

        if let Some(status) = self.engine.resources.get::<SceneLaunchStatus>().cloned() {
            if status.active && matches!(self.ui_selection.active(), UiProviderKind::Plugin { .. })
            {
                let overlay = self.scene_launch_overlay(&status);
                crate::platform_runtime::ui_gateway_frame::publish_loading_overlay(
                    &overlay,
                    self.ui_provider_binding(),
                    ui_frame_index,
                );
            }
        }

        let screen_profile_refresh = {
            let screen_profile = &mut self.screen_profile;
            let resources = self.engine.resources_mut();
            screen_profile.prepare_frame(resources, ui_frame_index)
        };

        let provider_ui_active =
            matches!(self.ui_selection.active(), UiProviderKind::Plugin { .. });
        let debug_overlay_active = self
            .engine
            .resources
            .get::<newengine_ui_api::UiRuntimeDebugOverlayTelemetry>()
            .is_some();
        let scene_launch_active = self
            .engine
            .resources
            .get::<SceneLaunchStatus>()
            .map(|status| status.active)
            .unwrap_or(false);

        // Provider UI is a persistent overlay contract, not only a debug-overlay side effect.
        // The 1000-fps hot-path pass accidentally skipped engine.ui after launch unless
        // runtime-debug telemetry was enabled, so the gameplay HUD vanished and the frame graph
        // legitimately collapsed to `ui=none`. Keep UI visible by using a cached provider draw
        // list for idle gameplay, and refresh it only when state can change.
        let provider_ui_needed = self.ui_build.is_some()
            || debug_overlay_active
            || scene_launch_active
            || screen_profile_refresh
            || ui_dispatch_refresh;
        let provider_gameplay_hud = provider_ui_active
            && !self.minimized
            && self.surface.width > 0
            && self.surface.height > 0;
        let provider_ui_refresh = provider_gameplay_hud
            || provider_ui_needed
            || screen_profile_refresh
            || ui_dispatch_refresh
            || self.cached_provider_ui_draw.is_none()
            || ui_frame_index <= 4
            || ui_frame_index % 30 == 1;

        let mut ui_draw = if provider_ui_active && (provider_ui_needed || provider_gameplay_hud) {
            if provider_ui_refresh {
                let render_surface_ids = if scene_launch_active {
                    vec![UI_SURFACE_ENGINE_LOADING.to_owned()]
                } else {
                    Vec::new()
                };
                match crate::platform_runtime::ui_gateway_frame::request_ui_draw_list(
                    ui_frame_index,
                    dt_sec,
                    [self.surface.width, self.surface.height],
                    self.surface.pixels_per_point,
                    &render_surface_ids,
                    &self.ui_frame_policy,
                )? {
                    Some(draw_list) => {
                        let mut cached = draw_list.clone();
                        cached.texture_delta.clear();
                        self.cached_provider_ui_draw = Some(cached);
                        Some(draw_list)
                    }
                    None if provider_ui_needed => {
                        self.cached_provider_ui_draw = None;
                        None
                    }
                    None => self.cached_provider_ui_draw.clone(),
                }
            } else {
                self.cached_provider_ui_draw.clone()
            }
        } else {
            self.cached_provider_ui_draw = None;
            None
        };

        if let Some(build) = self.ui_build.as_deref_mut() {
            let mut desc = UiFrameDesc::new(dt_sec).with_surface(
                self.surface.width,
                self.surface.height,
                self.surface.pixels_per_point,
            );

            if let Some(input) = input_frame {
                desc = desc.with_input(input);
            }

            let out = self.ui.run_frame(&(), desc, build);
            if !out.draw_list.mesh.vertices.is_empty() || !out.draw_list.mesh.indices.is_empty() {
                ui_draw = Some(out.draw_list);
            }
        }

        if let Some(draw_list) = ui_draw {
            self.engine.resources_mut().insert(draw_list);
        } else {
            let _ = self
                .engine
                .resources_mut()
                .remove::<newengine_ui_api::UiDrawList>();
        }

        match self.engine.step() {
            Ok(()) => {
                // ModuleCtx::request_exit() may be raised during the frame and
                // converted into the shared shutdown token after Engine::step().
                // Do not wait for a later redraw/input event: return an explicit
                // platform exit now so winit tears down the window and engine.shutdown
                // runs, allowing profiler plugins to flush final reports.
                if self.engine.shutdown_token().is_requested() {
                    newengine_ulog_api::ulog::info!("platform runtime: shutdown requested by engine module; requesting native exit");
                    return Ok(PlatformStepResultV1 {
                        exit_requested: true,
                        ..PlatformStepResultV1::default()
                    });
                }

                if let Some(status) = self.engine.resources.get::<SceneLaunchStatus>().cloned() {
                    if status.active {
                        return Ok(self.scene_launch_step_result(&status));
                    }
                    if matches!(self.ui_selection.active(), UiProviderKind::Plugin { .. }) {
                        crate::platform_runtime::ui_gateway_frame::publish_loading_overlay_inactive(
                            ui_frame_index,
                        );
                    }
                }

                if let Some(status) = self.engine.resources.get::<RenderBackendStatus>() {
                    if status.degraded {
                        return Ok(self.degraded_backend_step_result(status));
                    }
                }
                Ok(PlatformStepResultV1::default())
            }
            Err(EngineError::ExitRequested) => Ok(PlatformStepResultV1 {
                exit_requested: true,
                ..PlatformStepResultV1::default()
            }),
            Err(e) => {
                let message = e.to_string();
                newengine_ulog_api::ulog::error!("platform runtime: engine.step failed in running state; entering soft degradation instead of exiting: {message}");
                Ok(self.enter_runtime_soft_degraded_step("engine.step", message))
            }
        }
    }

    pub(crate) fn platform_window_ready(&self) -> bool {
        self.surface.width > 0
            && self.surface.height > 0
            && self.bootstrap_stage != RuntimeBootstrapStage::AwaitingWindow
    }

    pub(crate) fn render_backend_label(&self) -> String {
        self.engine
            .resources
            .get::<ResolvedRenderBackendConfig>()
            .map(|resolved| render_backend_label_from_id(resolved.backend_id.as_str()))
            .unwrap_or_else(|| "WAIT".to_owned())
    }
}
