use std::time::Instant;

use newengine_core::EngineResult;
use newengine_platform_api::PlatformStepResultV1;
use newengine_ui_api::{
    UiEventDispatchFrame, UiInputFrame, UiPresentationFlowState, UI_SURFACE_ENGINE_CONSOLE,
};

use crate::platform_input::poll_input_frame;
use crate::platform_runtime::HostPlatformRuntime;

use super::super::running_frontend_feedback::{
    frontend_exit_feedback_due, ui_dispatch_requests_exit, update_frontend_keycap_feedback,
};
use super::super::running_settings::{
    frontend_settings_apply_requested, frontend_settings_debounce_due, persist_frontend_settings,
    stage_frontend_setting_actions,
};
use super::ui_helpers::runtime_debug_overlay_allowed;

pub(super) struct RunningInputState {
    pub(super) input_frame: Option<UiInputFrame>,
    pub(super) input_poll_ms: f64,
    pub(super) ui_provider_dispatch_ms: f64,
    pub(super) ui_provider_dispatch_used: bool,
    pub(super) ui_dispatch_refresh: bool,
    pub(super) game_profile_active: bool,
    pub(super) console_open: bool,
    pub(super) console_draw_refresh: bool,
}

pub(super) enum RunningInputOutcome {
    Continue(RunningInputState),
    Exit(PlatformStepResultV1),
}

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

pub(super) fn prepare_running_input(
    runtime: &mut HostPlatformRuntime,
    ui_frame_index: u64,
) -> EngineResult<RunningInputOutcome> {
    let input_poll_started = Instant::now();
    let raw_input_frame = poll_input_frame();
    let input_poll_ms = input_poll_started.elapsed().as_secs_f64() * 1000.0;

    let console_frame = crate::platform_runtime::console_overlay::prepare_frame(
        runtime.engine.resources_mut(),
        raw_input_frame.as_ref(),
        [runtime.surface.width, runtime.surface.height],
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
    let previous_ui_hover = runtime
        .engine
        .resources
        .get::<UiEventDispatchFrame>()
        .and_then(|frame| frame.hovered_node.as_ref())
        .map(|hit| (hit.surface_id.clone(), hit.node_id.clone()));
    let game_profile_active = runtime
        .engine
        .resources
        .get::<newengine_ui_api::UiScreenProfileState>()
        .is_some_and(|state| state.descriptor.profile == newengine_ui_api::UiScreenProfile::Game);

    if runtime_debug_overlay_allowed(game_profile_active) {
        if let Some(telemetry) = runtime
            .engine
            .resources
            .get::<newengine_ui_api::UiRuntimeDebugOverlayTelemetry>()
        {
            crate::platform_runtime::ui_gateway_frame::publish_debug_overlay_telemetry(telemetry);
        }
    }

    if let Some(input) = input_frame.clone() {
        runtime.engine.resources_mut().insert::<UiInputFrame>(input);
    }

    // The provider-side input view is read-only in this stage; borrowing it avoids
    // cloning the full key/text/gamepad frame on every dispatched host frame.
    let ui_dispatch_frame = if let Some(input) = ui_provider_input_frame.as_ref() {
        let frontend_presentation_active = runtime
            .engine
            .resources
            .get::<UiPresentationFlowState>()
            .is_some_and(|state| state.state_id != "gameplay");
        let ui_capture_active = runtime
            .engine
            .resources
            .get::<newengine_ui_api::UiInputCaptureState>()
            .is_some_and(|capture| capture.requests_capture());
        let dispatch_to_provider = !game_profile_active
            || frontend_presentation_active
            || ui_capture_active
            || gameplay_input_requires_ui_dispatch(input);
        if dispatch_to_provider {
            ui_provider_dispatch_used = true;
            let dispatch_started = Instant::now();
            let dispatch_surface_id = if console_frame.open {
                Some(UI_SURFACE_ENGINE_CONSOLE)
            } else {
                runtime
                    .engine
                    .resources
                    .get::<UiPresentationFlowState>()
                    .and_then(|state| state.active_surface_id.as_deref())
                    .map(str::trim)
                    .filter(|id| !id.is_empty())
            };
            let dispatch_result = crate::platform_runtime::ui_gateway_frame::dispatch_input_frame(
                ui_frame_index,
                input,
                [runtime.surface.width, runtime.surface.height],
                runtime.surface.pixels_per_point,
                dispatch_surface_id,
            );
            ui_provider_dispatch_ms = dispatch_started.elapsed().as_secs_f64() * 1000.0;
            match dispatch_result? {
                Some(frame) => {
                    runtime
                        .engine
                        .resources_mut()
                        .insert::<UiEventDispatchFrame>(frame.clone());
                    Some(frame)
                }
                None => {
                    let _ = runtime
                        .engine
                        .resources_mut()
                        .remove::<UiEventDispatchFrame>();
                    None
                }
            }
        } else {
            let _ = runtime
                .engine
                .resources_mut()
                .remove::<UiEventDispatchFrame>();
            None
        }
    } else {
        let _ = runtime.engine.resources_mut().remove::<UiInputFrame>();
        let _ = runtime
            .engine
            .resources_mut()
            .remove::<UiEventDispatchFrame>();
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

    let presentation_state_id = runtime
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
    let pointer_button_edge = input_frame
        .as_ref()
        .is_some_and(|input| !input.mouse_pressed.is_empty() || !input.mouse_released.is_empty());
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
        runtime.on_close_requested()?;
        return Ok(RunningInputOutcome::Exit(PlatformStepResultV1 {
            exit_requested: true,
            ..PlatformStepResultV1::default()
        }));
    }

    Ok(RunningInputOutcome::Continue(RunningInputState {
        input_frame,
        input_poll_ms,
        ui_provider_dispatch_ms,
        ui_provider_dispatch_used,
        ui_dispatch_refresh,
        game_profile_active,
        console_open: console_frame.open,
        console_draw_refresh: console_frame.draw_refresh,
    }))
}
