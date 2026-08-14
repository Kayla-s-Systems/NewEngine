use std::time::{Duration, Instant};

use newengine_core::render::{RenderBackendStatus, SceneLaunchStatus};
use newengine_core::{EngineError, EngineResult};
use newengine_platform_api::PlatformStepResultV1;
use newengine_ui::{UiFrameDesc, UiProviderKind};
use newengine_ui_api::{
    UiEventDispatchFrame, UiInputFrame, UiPresentationFlowState, UI_SURFACE_ENGINE_LOADING,
};

use crate::platform_input::poll_input_frame;
use crate::platform_runtime::bootstrap_overlay::RuntimeBootstrapStage;
use crate::render_runtime::ResolvedRenderBackendConfig;

use super::super::HostPlatformRuntime;
use super::mapping::render_backend_label_from_id;
use super::running_frontend_feedback::{
    animate_frontend_keycap_feedback, frontend_exit_feedback_due, ui_dispatch_requests_exit,
    update_frontend_keycap_feedback,
};
use super::running_settings::{
    frontend_settings_apply_requested, frontend_settings_debounce_due, persist_frontend_settings,
    stage_frontend_setting_actions,
};
use super::running_ui::{
    effective_scene_launch_active, loading_overlay_requires_immediate_publish,
    provider_draw_has_active_animation,
};

const LOADING_OVERLAY_MIN_PUBLISH_INTERVAL: Duration = Duration::from_millis(50);
const PROFILER_SAMPLE_TOPIC: &str = "newengine.diagnostics.profiler.sample.v1";

#[inline]
fn gameplay_input_requires_ui_dispatch(input: &UiInputFrame) -> bool {
    !input.keys_pressed.is_empty()
        || !input.keys_released.is_empty()
        || !input.mouse_pressed.is_empty()
        || !input.mouse_released.is_empty()
        || input.mouse_wheel.0.abs() > f32::EPSILON
        || input.mouse_wheel.1.abs() > f32::EPSILON
        || !input.text.is_empty()
        || !input.ime_preedit.is_empty()
        || !input.ime_commit.is_empty()
        || !input.text_edit_ops.is_empty()
        || !input.gamepad_buttons_pressed.is_empty()
        || !input.gamepad_buttons_released.is_empty()
}

impl HostPlatformRuntime {
    pub(crate) fn step_running(&mut self, dt_sec: f32) -> EngineResult<PlatformStepResultV1> {
        let host_frame_started = Instant::now();
        self.ui_frame_index = self.ui_frame_index.wrapping_add(1);
        let ui_frame_index = self.ui_frame_index;
        let input_poll_started = Instant::now();
        let input_frame = poll_input_frame();
        let input_poll_ms = input_poll_started.elapsed().as_secs_f64() * 1000.0;
        let mut ui_provider_dispatch_ms = 0.0_f64;
        let mut ui_provider_dispatch_used = false;
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
            let game_profile_active = self
                .engine
                .resources
                .get::<newengine_ui_api::UiScreenProfileState>()
                .is_some_and(|state| {
                    state.descriptor.profile == newengine_ui_api::UiScreenProfile::Game
                });
            let frontend_presentation_active = self
                .engine
                .resources
                .get::<UiPresentationFlowState>()
                .is_some_and(|state| state.state_id != "gameplay");
            let ui_capture_active = self
                .engine
                .resources
                .get::<newengine_ui_api::UiInputCaptureState>()
                .is_some_and(|capture| capture.requests_capture());
            let dispatch_to_provider = !game_profile_active
                || frontend_presentation_active
                || ui_capture_active
                || gameplay_input_requires_ui_dispatch(&input);
            if dispatch_to_provider {
                ui_provider_dispatch_used = true;
                let dispatch_started = Instant::now();
                let dispatch_result =
                    crate::platform_runtime::ui_gateway_frame::dispatch_input_frame(
                        ui_frame_index,
                        &input,
                        [self.surface.width, self.surface.height],
                        self.surface.pixels_per_point,
                    );
                ui_provider_dispatch_ms = dispatch_started.elapsed().as_secs_f64() * 1000.0;
                match dispatch_result? {
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
                let _ = self.engine.resources_mut().remove::<UiEventDispatchFrame>();
                None
            }
        } else {
            let _ = self.engine.resources_mut().remove::<UiInputFrame>();
            let _ = self.engine.resources_mut().remove::<UiEventDispatchFrame>();
            None
        };
        let frontend_settings_force_save = ui_dispatch_frame
            .as_ref()
            .is_some_and(frontend_settings_apply_requested);
        if let Some(frame) = ui_dispatch_frame.as_ref() {
            stage_frontend_setting_actions(frame);
        }
        if frontend_settings_force_save || frontend_settings_debounce_due() {
            match persist_frontend_settings() {
                Ok(applied) if applied > 0 => newengine_ulog_api::ulog::info!(
                    "platform runtime: frontend settings persisted changes={} path='config.json' restart_required=true",
                    applied,
                ),
                Ok(_) => {}
                Err(error) => newengine_ulog_api::ulog::warn!(
                    "platform runtime: frontend settings persistence failed err='{}'",
                    error,
                ),
            }
        }
        let presentation_state_id = self
            .engine
            .resources
            .get::<UiPresentationFlowState>()
            .map(|state| state.state_id.as_str());
        update_frontend_keycap_feedback(
            input_frame.as_ref(),
            ui_dispatch_frame.as_ref(),
            presentation_state_id,
        );
        let ui_dispatch_refresh = ui_dispatch_frame
            .as_ref()
            .map(|frame| !frame.actions.is_empty() || !frame.state_patches.is_empty())
            .unwrap_or(false);
        let escape_requests_main_exit = input_frame.as_ref().is_some_and(|input| {
            input.is_key_pressed(newengine_ui_api::keys::ESCAPE)
                && presentation_state_id == Some("main_menu")
        });
        let exit_requested_now = ui_dispatch_frame
            .as_ref()
            .is_some_and(ui_dispatch_requests_exit)
            || escape_requests_main_exit;
        if frontend_exit_feedback_due(exit_requested_now) {
            newengine_ulog_api::ulog::info!(
                "platform runtime: native close requested after frontend keycap feedback"
            );
            self.on_close_requested()?;
            return Ok(PlatformStepResultV1 {
                exit_requested: true,
                ..PlatformStepResultV1::default()
            });
        }

        let input_dispatch_ms = host_frame_started.elapsed().as_secs_f64() * 1000.0;
        let ui_prepare_started = Instant::now();
        let scene_launch_status = self.engine.resources.get::<SceneLaunchStatus>().cloned();
        let editor_profile_active = self
            .engine
            .resources
            .get::<newengine_ui_api::UiScreenProfileState>()
            .is_some_and(|state| {
                state.descriptor.profile == newengine_ui_api::UiScreenProfile::Editor
            });
        let presentation_blocks_world_bootstrap = !editor_profile_active
            && self
                .engine
                .resources
                .get::<UiPresentationFlowState>()
                .is_some_and(|state| state.blocks_world_bootstrap);
        // SceneLaunchStatus can remain active from the final bootstrap handoff. An
        // authored frontend state owns presentation before world bootstrap, so that
        // stale status must not keep engine.ui.loading mounted or restrict the draw
        // request to the loading surface only.
        let scene_launch_active = effective_scene_launch_active(
            scene_launch_status.as_ref(),
            presentation_blocks_world_bootstrap,
        );
        let provider_ui_active =
            matches!(self.ui_selection.active(), UiProviderKind::Plugin { .. });
        let loading_surface_state_changed = if provider_ui_active && scene_launch_active {
            let status = scene_launch_status
                .as_ref()
                .expect("active scene launch status");
            let overlay = self.scene_launch_overlay(status);
            let now = Instant::now();
            let changed = self
                .last_published_loading_overlay
                .as_ref()
                .is_none_or(|previous| previous != &overlay);
            let immediate = loading_overlay_requires_immediate_publish(
                self.last_published_loading_overlay.as_ref(),
                &overlay,
            );
            let interval_elapsed = self.last_loading_overlay_publish_at.is_none_or(|last| {
                now.saturating_duration_since(last) >= LOADING_OVERLAY_MIN_PUBLISH_INTERVAL
            });

            self.loading_surface_inactive_published = false;
            if changed && (immediate || interval_elapsed) {
                crate::platform_runtime::ui_gateway_frame::publish_loading_overlay(
                    &overlay,
                    self.ui_provider_binding(),
                    ui_frame_index,
                );
                self.last_published_loading_overlay = Some(overlay);
                self.last_loading_overlay_publish_at = Some(now);
                true
            } else {
                false
            }
        } else {
            let had_running_overlay = self.last_published_loading_overlay.take().is_some();
            self.last_loading_overlay_publish_at = None;
            // The bootstrap stage publishes engine.ui.loading outside the running-loop
            // cache. Always send one explicit inactive update when entering a frontend
            // or after launch completion, otherwise the retained fullscreen surface can
            // survive indefinitely and cover authored UI with a black frame.
            if provider_ui_active && !self.loading_surface_inactive_published {
                crate::platform_runtime::ui_gateway_frame::publish_loading_overlay_inactive(
                    ui_frame_index,
                );
                self.loading_surface_inactive_published = true;
                true
            } else {
                had_running_overlay
            }
        };

        let screen_profile_refresh = {
            let screen_profile = &mut self.screen_profile;
            let resources = self.engine.resources_mut();
            screen_profile.prepare_frame(resources, ui_frame_index)
        };

        let debug_overlay_active = self
            .engine
            .resources
            .get::<newengine_ui_api::UiRuntimeDebugOverlayTelemetry>()
            .is_some();
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
            && !scene_launch_active
            && !self.minimized
            && self.surface.width > 0
            && self.surface.height > 0;
        // Authored gameplay HUD is retained UI. Rebuilding and serializing the
        // entire component graph every render frame creates a CPU/service stall and
        // directly harms mouse-look frame pacing. State patches continue to update
        // provider state; the cached draw-list is refreshed at 15 Hz at a 60 Hz
        // render cadence, immediately for interaction/layout changes, and whenever
        // no valid cache exists.
        // Gameplay HUD is retained. Refresh it on real invalidation/animation, not
        // on a periodic timer; a provider round-trip costs multiple milliseconds and
        // a fixed every-fourth-frame rebuild creates visible 30/48 ms cadence steps.
        let gameplay_hud_refresh_due = false;
        let provider_animation_refresh = self
            .cached_provider_ui_draw
            .as_ref()
            .is_some_and(provider_draw_has_active_animation);
        let provider_ui_refresh = loading_surface_state_changed
            || (debug_overlay_active && !scene_launch_active)
            || screen_profile_refresh
            || ui_dispatch_refresh
            || self.ui_build.is_some()
            || self.cached_provider_ui_draw.is_none()
            || provider_animation_refresh
            || gameplay_hud_refresh_due;
        let allow_cached_provider_ui_draw = provider_gameplay_hud
            || scene_launch_active
            || debug_overlay_active
            || screen_profile_refresh
            || ui_dispatch_refresh
            || self.ui_build.is_some();
        if !allow_cached_provider_ui_draw && self.cached_provider_ui_draw.is_some() {
            self.cached_provider_ui_draw = None;
        }

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
                    None if allow_cached_provider_ui_draw => self.cached_provider_ui_draw.clone(),
                    None => {
                        self.cached_provider_ui_draw = None;
                        None
                    }
                }
            } else if allow_cached_provider_ui_draw {
                self.cached_provider_ui_draw.clone()
            } else {
                self.cached_provider_ui_draw = None;
                None
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

            if let Some(input) = input_frame.clone() {
                desc = desc.with_input(input);
            }

            let out = self.ui.run_frame(&(), desc, build);
            if !out.draw_list.mesh.vertices.is_empty() || !out.draw_list.mesh.indices.is_empty() {
                ui_draw = Some(out.draw_list);
            }
        }

        if scene_launch_active {
            if let Some(draw_list) = ui_draw.as_mut() {
                crate::platform_runtime::ui_gateway_frame::animate_loading_draw_list(
                    draw_list,
                    crate::platform_runtime::ui_gateway_frame::loading_animation_now_ms(),
                );
            }
        }

        if let Some(draw_list) = ui_draw.as_mut() {
            animate_frontend_keycap_feedback(draw_list);
        }

        if let Some(draw_list) = ui_draw {
            self.engine.resources_mut().insert(draw_list);
        } else {
            let _ = self
                .engine
                .resources_mut()
                .remove::<newengine_ui_api::UiDrawList>();
        }

        let ui_prepare_ms = ui_prepare_started.elapsed().as_secs_f64() * 1000.0;
        let engine_step_started = Instant::now();
        let engine_step_result = self.engine.step();
        let engine_step_ms = engine_step_started.elapsed().as_secs_f64() * 1000.0;
        let engine_timing = self
            .engine
            .resources
            .get::<newengine_core::engine::EngineFrameTimingTelemetry>()
            .cloned();
        let render_timing = self
            .engine
            .resources
            .get::<newengine_core::render::RenderModuleTimingTelemetry>()
            .cloned();
        let host_total_ms = host_frame_started.elapsed().as_secs_f64() * 1000.0;

        let pacing_wait_ms = render_timing
            .as_ref()
            .filter(|render| {
                engine_timing.as_ref().is_some_and(|engine| {
                    // RenderController maintains its own presentation frame counter,
                    // which can lead/lag the core frame index by one around launch
                    // handoff. The resource itself is overwritten during the just-
                    // completed engine.step(), so a one-frame delta is current; a
                    // larger delta is stale and must never be subtracted.
                    render.frame_index.abs_diff(engine.frame_index) <= 1
                })
            })
            .map(|render| {
                f64::from(render.backend_frame_slot_wait_ms)
                    + f64::from(render.backend_surface_acquire_ms)
                    + f64::from(render.backend_image_wait_ms)
            })
            .unwrap_or(0.0);
        let host_active_cpu_ms = (host_total_ms - pacing_wait_ms).max(0.0);

        if let Some(timing) = engine_timing.as_ref() {
            let active_cpu_ms = (timing.total_ms - pacing_wait_ms).max(0.0);
            let missed_frame = timing.total_ms >= 20.0;
            if active_cpu_ms >= 12.0 || missed_frame || timing.frame_index.is_multiple_of(120) {
                let payload = serde_json::json!({
                    "schema": "newengine.diagnostics.profiler.sample.v1",
                    "category": "engine.frame",
                    "source": "newengine-core",
                    "name": "engine frame orchestration",
                    "lane": "main-frame",
                    "priority": "critical",
                    "dependency_group": format!("engine.frame.{}", timing.frame_index),
                    "frame_index": timing.frame_index,
                    "elapsed_ms": active_cpu_ms,
                    "wall_elapsed_ms": timing.total_ms,
                    "pacing_wait_ms": pacing_wait_ms,
                    "budget_ms": 16.67,
                    "frame_budget_ms": 16.67,
                    "exceeded_frame_budget": active_cpu_ms > 16.67,
                    "missed_wall_frame": missed_frame,
                    "fixed_steps": timing.fixed_steps,
                    "time_begin_ms": timing.time_begin_ms,
                    "plugin_control_ms": timing.plugin_control_ms,
                    "fixed_time_ms": timing.fixed_time_ms,
                    "fixed_scheduler_ms": timing.fixed_scheduler_ms,
                    "fixed_plugins_ms": timing.fixed_plugins_ms,
                    "fixed_modules_ms": timing.fixed_modules_ms,
                    "update_scheduler_ms": timing.update_scheduler_ms,
                    "update_plugins_ms": timing.update_plugins_ms,
                    "update_modules_ms": timing.update_modules_ms,
                    "render_scheduler_ms": timing.render_scheduler_ms,
                    "render_plugins_ms": timing.render_plugins_ms,
                    "render_modules_ms": timing.render_modules_ms,
                    "scheduler_end_ms": timing.scheduler_end_ms,
                    "render_timing_frame_index": render_timing.as_ref().map(|it| it.frame_index),
                    "render_pre_begin_ms": render_timing.as_ref().map(|it| it.pre_begin_ms),
                    "render_backend_begin_ms": render_timing.as_ref().map(|it| it.backend_begin_ms),
                    "render_playable_frame_ms": render_timing.as_ref().map(|it| it.playable_frame_ms),
                    "render_diagnostics_before_present_ms": render_timing.as_ref().map(|it| it.diagnostics_before_present_ms),
                    "render_backend_end_ms": render_timing.as_ref().map(|it| it.backend_end_ms),
                    "backend_reported_begin_ms": render_timing.as_ref().map(|it| it.backend_reported_begin_ms),
                    "backend_frame_slot_wait_ms": render_timing.as_ref().map(|it| it.backend_frame_slot_wait_ms),
                    "backend_surface_acquire_ms": render_timing.as_ref().map(|it| it.backend_surface_acquire_ms),
                    "backend_image_wait_ms": render_timing.as_ref().map(|it| it.backend_image_wait_ms),
                    "backend_reported_end_ms": render_timing.as_ref().map(|it| it.backend_reported_end_ms),
                });
                if let Ok(bytes) = serde_json::to_vec(&payload) {
                    let _ = newengine_plugin_host::host_context::publish_event(
                        PROFILER_SAMPLE_TOPIC,
                        &bytes,
                    );
                }
            }
        }
        let host_wall_slow = host_total_ms >= 20.0;
        if host_active_cpu_ms >= 12.0 || host_wall_slow || ui_frame_index.is_multiple_of(120) {
            let payload = serde_json::json!({
                "schema": "newengine.diagnostics.profiler.sample.v1",
                "category": "host.frame",
                "source": "newengine-runtime-host",
                "name": "running host frame",
                "lane": "main-frame",
                "priority": "critical",
                "dependency_group": format!("host.frame.{ui_frame_index}"),
                "frame_index": ui_frame_index,
                "elapsed_ms": host_active_cpu_ms,
                "wall_elapsed_ms": host_total_ms,
                "pacing_wait_ms": pacing_wait_ms,
                "budget_ms": 16.67,
                "frame_budget_ms": 16.67,
                "exceeded_frame_budget": host_active_cpu_ms > 16.67,
                "missed_wall_frame": host_wall_slow,
                "input_dispatch_ms": input_dispatch_ms,
                "input_poll_ms": input_poll_ms,
                "ui_provider_dispatch_ms": ui_provider_dispatch_ms,
                "ui_provider_dispatch_used": ui_provider_dispatch_used,
                "ui_prepare_ms": ui_prepare_ms,
                "engine_step_ms": engine_step_ms,
                "render_timing_frame_index": render_timing.as_ref().map(|it| it.frame_index),
                "provider_ui_refresh": provider_ui_refresh,
                "gameplay_hud_refresh_due": gameplay_hud_refresh_due,
                "ui_dispatch_refresh": ui_dispatch_refresh,
                "screen_profile_refresh": screen_profile_refresh,
            });
            if let Ok(bytes) = serde_json::to_vec(&payload) {
                let _ = newengine_plugin_host::host_context::publish_event(
                    PROFILER_SAMPLE_TOPIC,
                    &bytes,
                );
            }
        }

        match engine_step_result {
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
