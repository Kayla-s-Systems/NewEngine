mod input;
mod profiler;
mod ui_helpers;

use std::time::Instant;

use newengine_core::render::{RenderBackendStatus, SceneLaunchStatus};
use newengine_core::{EngineError, EngineResult};
use newengine_platform_api::PlatformStepResultV1;
use newengine_render_runtime_adapter::ResolvedRenderBackendConfig;
use newengine_ui::{UiFrameDesc, UiProviderKind};
use newengine_ui_api::{
    UiDrawInvalidationState, UiGameLayerStackState, UiInGameEditorState, UiLayerCompositionPlan,
    UiLayerDomain, UiLayerDrawPacketSet, UiPresentationFlowState, UiScreenProfileState,
    UI_PRESENTATION_TARGET_PRIMARY, UI_SURFACE_EDITOR_SHELL, UI_SURFACE_ENGINE_CONSOLE,
    UI_SURFACE_RUNTIME_DEBUG_OVERLAY,
};

use crate::platform_runtime::bootstrap_overlay::RuntimeBootstrapStage;

use super::super::HostPlatformRuntime;
use super::mapping::render_backend_label_from_id;
use super::running_frontend_feedback::animate_frontend_keycap_feedback;
use super::running_ui::{effective_scene_launch_active, provider_draw_has_active_animation};
use input::{prepare_running_input, RunningInputOutcome, RunningInputState};
use profiler::{publish_running_frame_samples, HostFrameProfileInput};
use ui_helpers::{
    append_surface_once, presentation_surface_domain, runtime_debug_overlay_allowed,
    should_request_shell_ui,
};

impl HostPlatformRuntime {
    pub(crate) fn step_running(&mut self, dt_sec: f32) -> EngineResult<PlatformStepResultV1> {
        let host_frame_started = Instant::now();
        self.ui_frame_index = self.ui_frame_index.wrapping_add(1);
        let ui_frame_index = self.ui_frame_index;
        let input_state = match prepare_running_input(self, ui_frame_index)? {
            RunningInputOutcome::Continue(state) => state,
            RunningInputOutcome::Exit(result) => return Ok(result),
        };
        let RunningInputState {
            input_frame,
            input_poll_ms,
            ui_provider_dispatch_ms,
            ui_provider_dispatch_used,
            ui_dispatch_refresh,
            game_profile_active,
            console_open,
            console_draw_refresh,
        } = input_state;
        let input_dispatch_ms = host_frame_started.elapsed().as_secs_f64() * 1000.0;
        let ui_prepare_started = Instant::now();
        let scene_launch_status = self.engine.resources.get::<SceneLaunchStatus>().cloned();
        let editing_tools_available = self
            .engine
            .resources
            .get::<newengine_plugin_host::PluginsSnapshot>()
            .is_some_and(|snapshot| {
                snapshot.has_running_capability(newengine_plugin_api::CAPABILITY_ID_EDITING_TOOLS)
            });
        let presentation_blocks_world_bootstrap = self
            .engine
            .resources
            .get::<UiPresentationFlowState>()
            .is_some_and(|state| state.blocks_world_bootstrap);
        let scene_launch_active = effective_scene_launch_active(
            scene_launch_status.as_ref(),
            presentation_blocks_world_bootstrap,
        );
        let provider_ui_active =
            matches!(self.ui_selection.active(), UiProviderKind::Plugin { .. });
        // Scene-launch progress is presented exclusively by the platform-native
        // loader. The retained UI system must never mount or animate a second
        // fullscreen loading surface in parallel.
        let loading_surface_state_changed = false;

        let screen_profile_refresh = {
            let screen_profile = &mut self.screen_profile;
            let resources = self.engine.resources_mut();
            screen_profile.prepare_frame(resources, ui_frame_index)
        };
        let editor_overlay_active = !scene_launch_active
            && editing_tools_available
            && self
                .engine
                .resources
                .get::<UiInGameEditorState>()
                .is_some_and(|state| state.enabled);
        if !editor_overlay_active {
            self.editor_ui_cache.clear();
        }

        let debug_overlay_active = runtime_debug_overlay_allowed(game_profile_active)
            && self
                .engine
                .resources
                .get::<newengine_ui_api::UiRuntimeDebugOverlayTelemetry>()
                .is_some();
        let console_overlay_active = console_open
            || crate::platform_runtime::console_overlay::is_open(&self.engine.resources);
        let debug_layer_active = debug_overlay_active || console_overlay_active;
        let invalidation = self
            .engine
            .resources
            .get::<UiDrawInvalidationState>()
            .copied()
            .unwrap_or_default();

        let presentation_surface = self
            .engine
            .resources
            .get::<UiPresentationFlowState>()
            .and_then(|state| state.active_surface_id.as_deref())
            .map(str::trim)
            .filter(|surface| !surface.is_empty())
            .map(str::to_owned);
        let presentation_focus = self
            .engine
            .resources
            .get::<UiScreenProfileState>()
            .map(|state| state.descriptor.input_focus_policy);

        let game_ui_invalidation_revision = invalidation.revision_for(UiLayerDomain::GameViewport);
        let mut game_ui_plan = self
            .engine
            .resources
            .get::<UiGameLayerStackState>()
            .map(|state| state.composition_plan(game_ui_invalidation_revision))
            .unwrap_or_else(|| {
                UiLayerCompositionPlan::disabled(
                    UiLayerDomain::GameViewport,
                    newengine_ui_api::UI_GAME_VIEWPORT_SURFACE_PRIMARY,
                    ui_frame_index,
                )
            });
        if presentation_focus.and_then(presentation_surface_domain)
            == Some(UiLayerDomain::GameViewport)
        {
            if let Some(surface_id) = presentation_surface.as_deref() {
                append_surface_once(&mut game_ui_plan, surface_id);
            }
        }

        let shell_domain = if !scene_launch_active && editor_overlay_active {
            UiLayerDomain::Editor
        } else {
            presentation_focus
                .and_then(presentation_surface_domain)
                .filter(|domain| *domain != UiLayerDomain::GameViewport)
                .unwrap_or(UiLayerDomain::System)
        };
        let mut shell_ui_plan = UiLayerCompositionPlan::disabled(
            shell_domain,
            UI_PRESENTATION_TARGET_PRIMARY,
            ui_frame_index,
        );
        shell_ui_plan.invalidation_revision = invalidation.revision_for(shell_domain);
        if editor_overlay_active {
            shell_ui_plan.surface_ids = vec![UI_SURFACE_EDITOR_SHELL.to_owned()];
        } else if presentation_focus
            .and_then(presentation_surface_domain)
            .is_some_and(|domain| domain == shell_domain)
        {
            if let Some(surface_id) = presentation_surface.as_deref() {
                append_surface_once(&mut shell_ui_plan, surface_id);
            }
        }
        if screen_profile_refresh.shell_ui {
            newengine_ulog_api::ulog::info!(
                "platform runtime: presentation render plan surface={:?} focus={:?} shell_domain={:?} shell_surfaces={:?} shell_surface_count={} provider_ui_active={} scene_launch_active={} editor_overlay_active={}",
                presentation_surface,
                presentation_focus,
                shell_domain,
                shell_ui_plan.surface_ids,
                shell_ui_plan.surface_ids.len(),
                provider_ui_active,
                scene_launch_active,
                editor_overlay_active,
            );
        }
        let mut debug_ui_plan = UiLayerCompositionPlan::disabled(
            UiLayerDomain::Debug,
            UI_PRESENTATION_TARGET_PRIMARY,
            ui_frame_index,
        );
        debug_ui_plan.invalidation_revision = invalidation.revision_for(UiLayerDomain::Debug);
        if debug_overlay_active {
            append_surface_once(&mut debug_ui_plan, UI_SURFACE_RUNTIME_DEBUG_OVERLAY);
        }
        if console_overlay_active {
            append_surface_once(&mut debug_ui_plan, UI_SURFACE_ENGINE_CONSOLE);
        }

        let provider_ui_needed =
            self.ui_build.is_some() || screen_profile_refresh.any() || ui_dispatch_refresh;
        let provider_surface_ready = provider_ui_active
            && !scene_launch_active
            && !self.minimized
            && self.surface.width > 0
            && self.surface.height > 0;
        let game_ui_layer_active = provider_surface_ready && game_ui_plan.is_active();

        let gameplay_hud_refresh_due = false;
        let game_ui_animation_refresh = self
            .game_ui_cache
            .draw()
            .is_some_and(provider_draw_has_active_animation);
        let shell_ui_animation_refresh = match shell_domain {
            UiLayerDomain::Editor => self
                .editor_ui_cache
                .draw()
                .is_some_and(provider_draw_has_active_animation),
            _ => self
                .system_ui_cache
                .draw()
                .is_some_and(provider_draw_has_active_animation),
        };
        let debug_ui_animation_refresh = self
            .debug_ui_cache
            .draw()
            .is_some_and(provider_draw_has_active_animation);

        let game_force_refresh =
            screen_profile_refresh.game_ui || ui_dispatch_refresh || gameplay_hud_refresh_due;
        let shell_force_refresh = loading_surface_state_changed
            || screen_profile_refresh.shell_ui
            || ui_dispatch_refresh
            || self.ui_build.is_some()
            || gameplay_hud_refresh_due;
        let debug_force_refresh = debug_overlay_active || console_draw_refresh;

        let game_ui_refresh = self.game_ui_cache.needs_refresh(
            &game_ui_plan,
            game_force_refresh,
            game_ui_animation_refresh,
        );
        let shell_ui_refresh = match shell_domain {
            UiLayerDomain::Editor => self.editor_ui_cache.needs_refresh(
                &shell_ui_plan,
                shell_force_refresh,
                shell_ui_animation_refresh,
            ),
            _ => self.system_ui_cache.needs_refresh(
                &shell_ui_plan,
                shell_force_refresh,
                shell_ui_animation_refresh,
            ),
        };
        let debug_ui_refresh = self.debug_ui_cache.needs_refresh(
            &debug_ui_plan,
            debug_force_refresh,
            debug_ui_animation_refresh,
        );
        let provider_ui_refresh = game_ui_refresh || shell_ui_refresh || debug_ui_refresh;
        let allow_cached_shell_draw = provider_surface_ready
            || screen_profile_refresh.shell_ui
            || ui_dispatch_refresh
            || self.ui_build.is_some();

        let mut game_ui_draw = None;
        if provider_ui_active && game_ui_layer_active {
            game_ui_draw = if game_ui_refresh {
                match crate::platform_runtime::ui_gateway_frame::request_ui_draw_list(
                    ui_frame_index,
                    dt_sec,
                    [self.surface.width, self.surface.height],
                    self.surface.pixels_per_point,
                    &game_ui_plan.surface_ids,
                    &self.ui_frame_policy,
                )? {
                    Some(draw_list) => {
                        self.game_ui_cache.store(game_ui_plan.clone(), &draw_list);
                        Some(draw_list)
                    }
                    None if self.game_ui_cache.plan_matches(&game_ui_plan) => {
                        self.game_ui_cache.cloned_draw()
                    }
                    None => None,
                }
            } else {
                self.game_ui_cache.cloned_draw()
            };
        }

        let shell_requested = should_request_shell_ui(
            provider_ui_active,
            scene_launch_active,
            provider_ui_needed,
            provider_surface_ready,
            game_ui_layer_active,
            editor_overlay_active,
            !shell_ui_plan.surface_ids.is_empty(),
        );
        let mut shell_ui_draw = if shell_requested {
            if shell_ui_refresh {
                let requested = crate::platform_runtime::ui_gateway_frame::request_ui_draw_list(
                    ui_frame_index,
                    dt_sec,
                    [self.surface.width, self.surface.height],
                    self.surface.pixels_per_point,
                    &shell_ui_plan.surface_ids,
                    &self.ui_frame_policy,
                )?;
                match (shell_domain, requested) {
                    (UiLayerDomain::Editor, Some(draw_list)) => {
                        self.editor_ui_cache
                            .store(shell_ui_plan.clone(), &draw_list);
                        Some(draw_list)
                    }
                    (_, Some(draw_list)) => {
                        self.system_ui_cache
                            .store(shell_ui_plan.clone(), &draw_list);
                        Some(draw_list)
                    }
                    (UiLayerDomain::Editor, None) if allow_cached_shell_draw => {
                        self.editor_ui_cache.cloned_draw()
                    }
                    (_, None) if allow_cached_shell_draw => self.system_ui_cache.cloned_draw(),
                    (UiLayerDomain::Editor, None) => {
                        self.editor_ui_cache.clear();
                        None
                    }
                    (_, None) => {
                        self.system_ui_cache.clear();
                        None
                    }
                }
            } else {
                match shell_domain {
                    UiLayerDomain::Editor => self.editor_ui_cache.cloned_draw(),
                    _ => self.system_ui_cache.cloned_draw(),
                }
            }
        } else {
            None
        };

        if screen_profile_refresh.shell_ui {
            newengine_ulog_api::ulog::info!(
                "platform runtime: presentation draw result requested={} refresh={} draw_present={} mesh_vertices={} mesh_indices={} paint_commands={}",
                shell_requested,
                shell_ui_refresh,
                shell_ui_draw.is_some(),
                shell_ui_draw.as_ref().map(|draw| draw.mesh.vertices.len()).unwrap_or(0),
                shell_ui_draw.as_ref().map(|draw| draw.mesh.indices.len()).unwrap_or(0),
                shell_ui_draw.as_ref().map(|draw| draw.paint.commands.len()).unwrap_or(0),
            );
        }

        let mut debug_ui_draw = if provider_ui_active && debug_layer_active {
            if debug_ui_refresh {
                match crate::platform_runtime::ui_gateway_frame::request_ui_draw_list(
                    ui_frame_index,
                    dt_sec,
                    [self.surface.width, self.surface.height],
                    self.surface.pixels_per_point,
                    &debug_ui_plan.surface_ids,
                    &self.ui_frame_policy,
                )? {
                    Some(draw_list) => {
                        self.debug_ui_cache.store(debug_ui_plan.clone(), &draw_list);
                        Some(draw_list)
                    }
                    None if self.debug_ui_cache.plan_matches(&debug_ui_plan) => {
                        self.debug_ui_cache.cloned_draw()
                    }
                    None => None,
                }
            } else {
                self.debug_ui_cache.cloned_draw()
            }
        } else {
            self.debug_ui_cache.clear();
            None
        };

        let mut built_system_ui_draw = None;
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
                built_system_ui_draw = Some(out.draw_list);
            }
        }

        if let Some(draw_list) = shell_ui_draw.as_mut() {
            animate_frontend_keycap_feedback(draw_list);
        }
        if let Some(draw_list) = built_system_ui_draw.as_mut() {
            animate_frontend_keycap_feedback(draw_list);
        }

        let mut ui_layers = UiLayerDrawPacketSet::new(ui_frame_index);
        if let Some(draw_list) = game_ui_draw.take() {
            ui_layers.push(game_ui_plan.draw_packet(draw_list));
        }
        if let Some(draw_list) = shell_ui_draw.take() {
            ui_layers.push(shell_ui_plan.draw_packet(draw_list));
        }
        if let Some(draw_list) = built_system_ui_draw {
            let mut build_plan = UiLayerCompositionPlan::disabled(
                UiLayerDomain::System,
                UI_PRESENTATION_TARGET_PRIMARY,
                ui_frame_index,
            );
            build_plan.invalidation_revision = invalidation.revision_for(UiLayerDomain::System);
            ui_layers.push(build_plan.draw_packet(draw_list));
        }
        if let Some(draw_list) = debug_ui_draw.take() {
            ui_layers.push(debug_ui_plan.draw_packet(draw_list));
        }

        let active_ui_plan = if game_ui_layer_active {
            game_ui_plan
        } else {
            shell_ui_plan
        };
        let active_ui_domain = active_ui_plan.domain;
        let active_ui_surface_count = active_ui_plan.surface_ids.len();
        let active_ui_invalidation_revision = active_ui_plan.invalidation_revision;
        self.engine.resources_mut().insert(active_ui_plan);

        if ui_layers.is_empty() {
            let _ = self
                .engine
                .resources_mut()
                .remove::<newengine_ui_api::UiLayerDrawPacketSet>();
        } else {
            self.engine.resources_mut().insert(ui_layers);
        }

        let ui_prepare_ms = ui_prepare_started.elapsed().as_secs_f64() * 1000.0;
        let engine_step_started = Instant::now();
        let engine_step_result = self.engine.step();
        let engine_step_ms = engine_step_started.elapsed().as_secs_f64() * 1000.0;
        let engine_timing = self
            .engine
            .resources
            .get::<newengine_core::engine::EngineFrameTimingTelemetry>();
        let render_timing = self
            .engine
            .resources
            .get::<newengine_core::render::RenderModuleTimingTelemetry>();
        let host_total_ms = host_frame_started.elapsed().as_secs_f64() * 1000.0;
        publish_running_frame_samples(
            engine_timing,
            render_timing,
            HostFrameProfileInput {
                ui_frame_index,
                host_total_ms,
                input_dispatch_ms,
                input_poll_ms,
                ui_provider_dispatch_ms,
                ui_provider_dispatch_used,
                ui_prepare_ms,
                engine_step_ms,
                provider_ui_refresh,
                active_ui_domain,
                active_ui_surface_count,
                active_ui_invalidation_revision,
                gameplay_hud_refresh_due,
                ui_dispatch_refresh,
                screen_profile_refresh: screen_profile_refresh.any(),
                screen_profile_game_refresh: screen_profile_refresh.game_ui,
                screen_profile_shell_refresh: screen_profile_refresh.shell_ui,
            },
        );

        match engine_step_result {
            Ok(()) => {
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

#[cfg(test)]
mod tests;
