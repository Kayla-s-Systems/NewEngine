use std::time::Instant;

use newengine_core::render::{RenderBackendStatus, SceneLaunchStatus};
use newengine_core::{EngineError, EngineResult};
use newengine_platform_api::PlatformStepResultV1;
use newengine_ui::{UiFrameDesc, UiProviderKind};
use newengine_ui_api::{
    UiDrawInvalidationState, UiEventDispatchFrame, UiGameLayerStackState, UiInGameEditorState,
    UiInputFrame, UiLayerCompositionPlan, UiLayerDomain, UiLayerDrawPacketSet,
    UiPresentationFlowState, UiScreenInputFocusPolicy, UiScreenProfileState,
    UI_PRESENTATION_TARGET_PRIMARY, UI_SURFACE_EDITOR_SHELL, UI_SURFACE_ENGINE_CONSOLE,
    UI_SURFACE_RUNTIME_DEBUG_OVERLAY,
};

use crate::platform_input::poll_input_frame;
use crate::platform_runtime::bootstrap_overlay::RuntimeBootstrapStage;
use newengine_render_runtime_adapter::ResolvedRenderBackendConfig;

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
use super::running_ui::{effective_scene_launch_active, provider_draw_has_active_animation};

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

#[inline]
fn should_request_shell_ui(
    provider_ui_active: bool,
    scene_launch_active: bool,
    provider_ui_needed: bool,
    provider_surface_ready: bool,
    game_ui_layer_active: bool,
    editor_overlay_active: bool,
    shell_plan_active: bool,
) -> bool {
    provider_ui_active
        && !scene_launch_active
        && shell_plan_active
        && (provider_ui_needed || provider_surface_ready)
        && (editor_overlay_active || !game_ui_layer_active)
}

#[inline]
fn presentation_surface_domain(focus: UiScreenInputFocusPolicy) -> Option<UiLayerDomain> {
    match focus {
        UiScreenInputFocusPolicy::UiSurface => Some(UiLayerDomain::System),
        UiScreenInputFocusPolicy::GameViewport => Some(UiLayerDomain::GameViewport),
        UiScreenInputFocusPolicy::EditorShell => Some(UiLayerDomain::Editor),
        UiScreenInputFocusPolicy::Headless => None,
    }
}

#[inline]
fn append_surface_once(plan: &mut UiLayerCompositionPlan, surface_id: &str) {
    let surface_id = surface_id.trim();
    if !surface_id.is_empty() && !plan.surface_ids.iter().any(|id| id == surface_id) {
        plan.surface_ids.push(surface_id.to_owned());
    }
}

#[inline]
fn runtime_debug_overlay_allowed(game_profile_active: bool) -> bool {
    !game_profile_active
}

impl HostPlatformRuntime {
    pub(crate) fn step_running(&mut self, dt_sec: f32) -> EngineResult<PlatformStepResultV1> {
        let host_frame_started = Instant::now();
        self.ui_frame_index = self.ui_frame_index.wrapping_add(1);
        let ui_frame_index = self.ui_frame_index;
        let input_poll_started = Instant::now();
        let raw_input_frame = poll_input_frame();
        let input_poll_ms = input_poll_started.elapsed().as_secs_f64() * 1000.0;
        // The runtime console owns raw keyboard/text/gameplay input before any
        // product/editor consumer. It still needs pointer position/buttons/wheel in
        // `engine.ui` for output scrolling, so keep two deliberately different views:
        // - gameplay/resource input: fully consumed while the console owns the edge;
        // - UI-provider input: mouse-only while the console remains open.
        let console_frame = crate::platform_runtime::console_overlay::prepare_frame(
            self.engine.resources_mut(),
            raw_input_frame.as_ref(),
            [self.surface.width, self.surface.height],
        );
        let ui_provider_input_frame = if console_frame.open {
            Some(
                crate::platform_runtime::console_overlay::provider_mouse_input(
                    raw_input_frame.as_ref(),
                ),
            )
        } else if console_frame.consumed_input {
            Some(UiInputFrame::default())
        } else {
            raw_input_frame.clone()
        };
        let input_frame = if console_frame.consumed_input {
            Some(UiInputFrame::default())
        } else {
            raw_input_frame
        };
        let mut ui_provider_dispatch_ms = 0.0_f64;
        let mut ui_provider_dispatch_used = false;
        // Preserve the prior provider interaction before this frame replaces the
        // retained dispatch resource. Hover enter/leave is a visual invalidation
        // edge even when no action/state patch is emitted.
        let previous_ui_hover = self
            .engine
            .resources
            .get::<UiEventDispatchFrame>()
            .and_then(|frame| frame.hovered_node.as_ref())
            .map(|hit| (hit.surface_id.clone(), hit.node_id.clone()));
        let game_profile_active = self
            .engine
            .resources
            .get::<newengine_ui_api::UiScreenProfileState>()
            .is_some_and(|state| {
                state.descriptor.profile == newengine_ui_api::UiScreenProfile::Game
            });
        // Technical render telemetry is never presented as a retained surface in
        // the shipping Game profile. It belongs to profiler/logging, not game HUD.
        if !game_profile_active {
            if let Some(telemetry) = self
                .engine
                .resources
                .get::<newengine_ui_api::UiRuntimeDebugOverlayTelemetry>()
                .cloned()
            {
                crate::platform_runtime::ui_gateway_frame::publish_debug_overlay_telemetry(
                    &telemetry,
                );
            }
        }

        if let Some(input) = input_frame.clone() {
            self.engine.resources_mut().insert::<UiInputFrame>(input);
        }
        let ui_dispatch_frame = if let Some(input) = ui_provider_input_frame.clone() {
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
                let dispatch_surface_id = if console_frame.open {
                    Some(UI_SURFACE_ENGINE_CONSOLE)
                } else {
                    self.engine
                        .resources
                        .get::<UiPresentationFlowState>()
                        .and_then(|state| state.active_surface_id.as_deref())
                        .map(str::trim)
                        .filter(|id| !id.is_empty())
                };
                let dispatch_result =
                    crate::platform_runtime::ui_gateway_frame::dispatch_input_frame(
                        ui_frame_index,
                        &input,
                        [self.surface.width, self.surface.height],
                        self.surface.pixels_per_point,
                        dispatch_surface_id,
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
        let current_ui_hover = ui_dispatch_frame
            .as_ref()
            .and_then(|frame| frame.hovered_node.as_ref())
            .map(|hit| (hit.surface_id.clone(), hit.node_id.clone()));
        let pointer_button_edge = input_frame.as_ref().is_some_and(|input| {
            !input.mouse_pressed.is_empty() || !input.mouse_released.is_empty()
        });
        let ui_interaction_refresh = previous_ui_hover != current_ui_hover || pointer_button_edge;
        let ui_dispatch_refresh = ui_dispatch_frame
            .as_ref()
            .map(|frame| !frame.actions.is_empty() || !frame.state_patches.is_empty())
            .unwrap_or(false)
            || ui_interaction_refresh;
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
        let console_overlay_active = console_frame.open
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
        let debug_force_refresh = debug_overlay_active || console_frame.draw_refresh;

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
            if let Some(input) = input_frame.clone() {
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
            game_ui_plan.clone()
        } else {
            shell_ui_plan.clone()
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
                engine_timing
                    .as_ref()
                    .is_some_and(|engine| render.frame_index.abs_diff(engine.frame_index) <= 1)
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
                let mut payload = payload;
                if let Some(object) = payload.as_object_mut() {
                    object.insert(
                        "backend_gpu_timestamps_enabled".to_owned(),
                        serde_json::json!(render_timing
                            .as_ref()
                            .map(|it| it.backend_gpu_timestamps_enabled)),
                    );
                    object.insert(
                        "backend_gpu_timing_frame_index".to_owned(),
                        serde_json::json!(render_timing
                            .as_ref()
                            .map(|it| it.backend_gpu_timing_frame_index)),
                    );
                    object.insert(
                        "backend_gpu_shadow_ms".to_owned(),
                        serde_json::json!(render_timing
                            .as_ref()
                            .map(|it| it.backend_gpu_shadow_ms)),
                    );
                    object.insert(
                        "backend_gpu_opaque_ms".to_owned(),
                        serde_json::json!(render_timing
                            .as_ref()
                            .map(|it| it.backend_gpu_opaque_ms)),
                    );
                    object.insert(
                        "backend_gpu_postfx_ms".to_owned(),
                        serde_json::json!(render_timing
                            .as_ref()
                            .map(|it| it.backend_gpu_postfx_ms)),
                    );
                    object.insert(
                        "backend_gpu_ui_ms".to_owned(),
                        serde_json::json!(render_timing.as_ref().map(|it| it.backend_gpu_ui_ms)),
                    );
                    object.insert(
                        "backend_gpu_profiled_ms".to_owned(),
                        serde_json::json!(render_timing
                            .as_ref()
                            .map(|it| it.backend_gpu_profiled_ms)),
                    );
                }
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
                "ui_layer_domain": active_ui_domain.as_str(),
                "ui_layer_surface_count": active_ui_surface_count,
                "ui_layer_invalidation_revision": active_ui_invalidation_revision,
                "gameplay_hud_refresh_due": gameplay_hud_refresh_due,
                "ui_dispatch_refresh": ui_dispatch_refresh,
                "screen_profile_refresh": screen_profile_refresh.any(),
                "screen_profile_game_refresh": screen_profile_refresh.game_ui,
                "screen_profile_shell_refresh": screen_profile_refresh.shell_ui,
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
mod game_ui_layer_tests {
    use super::*;
    use newengine_ui_api::{UiGameGuiConfig, UiGameLayerDescriptor};

    #[test]
    fn game_editor_and_debug_domains_remain_separate_packets() {
        let mut config = UiGameGuiConfig::simple_hud("ui/game/hud.neui@surface", "game.hud");
        config.layers.push(
            UiGameLayerDescriptor::menu("pause", "ui/game/pause.neui@surface", "game.pause")
                .initially_hidden(),
        );
        let state = UiGameLayerStackState::from_config(&config, 9);
        let game_plan = state.composition_plan(3);
        assert_eq!(game_plan.domain, UiLayerDomain::GameViewport);
        assert_eq!(game_plan.surface_ids, vec!["game.hud".to_owned()]);

        let mut debug_plan = UiLayerCompositionPlan::disabled(
            UiLayerDomain::Debug,
            UI_PRESENTATION_TARGET_PRIMARY,
            9,
        );
        debug_plan.surface_ids = vec![UI_SURFACE_RUNTIME_DEBUG_OVERLAY.to_owned()];

        let mut editor_plan = UiLayerCompositionPlan::disabled(
            UiLayerDomain::Editor,
            UI_PRESENTATION_TARGET_PRIMARY,
            9,
        );
        editor_plan.surface_ids = vec![UI_SURFACE_EDITOR_SHELL.to_owned()];

        let mut packets = UiLayerDrawPacketSet::new(9);
        packets.push(game_plan.draw_packet(newengine_ui_api::UiDrawList::new()));
        packets.push(editor_plan.draw_packet(newengine_ui_api::UiDrawList::new()));
        packets.push(debug_plan.draw_packet(newengine_ui_api::UiDrawList::new()));
        assert_eq!(
            packets
                .packets
                .iter()
                .map(|packet| packet.domain)
                .collect::<Vec<_>>(),
            vec![
                UiLayerDomain::GameViewport,
                UiLayerDomain::Editor,
                UiLayerDomain::Debug,
            ]
        );
    }

    #[test]
    fn shipping_game_profile_never_composites_runtime_debug_overlay() {
        assert!(!runtime_debug_overlay_allowed(true));
        assert!(runtime_debug_overlay_allowed(false));
    }

    #[test]
    fn active_editor_shell_is_not_suppressed_by_game_hud() {
        assert!(should_request_shell_ui(
            true, false, false, true, true, true, true,
        ));
        assert!(!should_request_shell_ui(
            true, false, false, true, true, false, true,
        ));
        assert!(!should_request_shell_ui(
            true, true, true, true, false, true, true,
        ));
    }

    #[test]
    fn game_viewport_without_shell_surface_never_requests_empty_shell_draw() {
        assert!(!should_request_shell_ui(
            true, false, true, true, false, false, false,
        ));
    }

    #[test]
    fn active_frontend_surface_still_requests_shell_draw() {
        assert!(should_request_shell_ui(
            true, false, true, true, false, false, true,
        ));
    }

    #[test]
    fn project_presentation_surface_routes_to_system_lane_for_frontend() {
        assert_eq!(
            presentation_surface_domain(UiScreenInputFocusPolicy::UiSurface),
            Some(UiLayerDomain::System)
        );
    }

    #[test]
    fn project_presentation_surface_routes_to_game_lane_for_hud() {
        assert_eq!(
            presentation_surface_domain(UiScreenInputFocusPolicy::GameViewport),
            Some(UiLayerDomain::GameViewport)
        );
    }

    #[test]
    fn presentation_surface_is_deduplicated_in_composition_plan() {
        let mut plan = UiLayerCompositionPlan::disabled(
            UiLayerDomain::System,
            UI_PRESENTATION_TARGET_PRIMARY,
            1,
        );
        append_surface_once(&mut plan, "forest-road.frontend.title");
        append_surface_once(&mut plan, "forest-road.frontend.title");
        assert_eq!(
            plan.surface_ids,
            vec!["forest-road.frontend.title".to_owned()]
        );
    }

    #[test]
    fn disabled_game_ui_stack_does_not_claim_a_render_lane() {
        let state = UiGameLayerStackState::default();
        assert!(!state.composition_plan(0).is_active());
    }
}
