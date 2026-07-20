use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use newengine_core::render::{RenderBackendStatus, SceneLaunchStatus};
use newengine_core::{EngineError, EngineResult};
use newengine_platform_api::PlatformStepResultV1;
use newengine_system_contracts::ScreenOverlayStatus;
use newengine_ui::{UiFrameDesc, UiProviderKind};
use newengine_ui_api::{
    UiEventDispatchFrame, UiInputFrame, UiPresentationFlowState, UI_SURFACE_ENGINE_LOADING,
};

use crate::platform_input::poll_input_frame;
use crate::platform_runtime::bootstrap_overlay::RuntimeBootstrapStage;
use crate::render_runtime::ResolvedRenderBackendConfig;

use super::super::HostPlatformRuntime;
use super::mapping::render_backend_label_from_id;

const LOADING_OVERLAY_MIN_PUBLISH_INTERVAL: Duration = Duration::from_millis(50);
const FRONTEND_SETTINGS_SAVE_DEBOUNCE: Duration = Duration::from_millis(300);
const FRONTEND_KEYCAP_FEEDBACK_DURATION: Duration = Duration::from_millis(360);
const FRONTEND_EXIT_FEEDBACK_HOLD: Duration = Duration::from_millis(240);

fn ui_dispatch_requests_exit(frame: &UiEventDispatchFrame) -> bool {
    frame.actions.iter().any(|action| {
        action.trigger == newengine_ui_api::UiNodeEventTrigger::Click
            && matches!(
                action.action_id.as_str(),
                "engine.lifecycle.exit" | "engine.exit.request" | "app.exit"
            )
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FrontendKeycapKind {
    Select,
    Back,
}

#[derive(Clone, Debug)]
struct FrontendKeycapFeedback {
    kind: FrontendKeycapKind,
    label: String,
    started_at: Instant,
}

fn frontend_keycap_feedback() -> &'static Mutex<Option<FrontendKeycapFeedback>> {
    static FEEDBACK: OnceLock<Mutex<Option<FrontendKeycapFeedback>>> = OnceLock::new();
    FEEDBACK.get_or_init(|| Mutex::new(None))
}

fn begin_frontend_keycap_feedback(kind: FrontendKeycapKind, label: impl Into<String>) {
    *frontend_keycap_feedback()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(FrontendKeycapFeedback {
        kind,
        label: label.into(),
        started_at: Instant::now(),
    });
}

fn frontend_exit_pending() -> &'static Mutex<Option<Instant>> {
    static PENDING: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(None))
}

fn frontend_exit_feedback_due(requested_now: bool) -> bool {
    let mut pending = frontend_exit_pending()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if requested_now && pending.is_none() {
        *pending = Some(Instant::now());
    }
    let due = pending
        .as_ref()
        .is_some_and(|started| started.elapsed() >= FRONTEND_EXIT_FEEDBACK_HOLD);
    if due {
        *pending = None;
    }
    due
}

fn update_frontend_keycap_feedback(
    input: Option<&UiInputFrame>,
    dispatch: Option<&UiEventDispatchFrame>,
    presentation_state: Option<&str>,
) {
    if let Some(action) = dispatch.and_then(|frame| {
        frame.actions.iter().find(|action| {
            matches!(
                action.trigger,
                newengine_ui_api::UiNodeEventTrigger::Click
                    | newengine_ui_api::UiNodeEventTrigger::ValueChanged
            )
        })
    }) {
        let (kind, label) = frontend_action_keycap(action.action_id.as_str());
        begin_frontend_keycap_feedback(kind, label);
        return;
    }
    let Some(input) = input else {
        return;
    };
    if input.is_key_pressed(newengine_ui_api::keys::KEY_E) {
        begin_frontend_keycap_feedback(FrontendKeycapKind::Select, "SELECT");
    } else if input.is_key_pressed(newengine_ui_api::keys::ESCAPE) {
        let label = if presentation_state == Some("main_menu") {
            "EXIT"
        } else {
            "BACK"
        };
        begin_frontend_keycap_feedback(FrontendKeycapKind::Back, label);
    }
}

fn frontend_action_keycap(action_id: &str) -> (FrontendKeycapKind, &'static str) {
    match action_id {
        "engine.lifecycle.exit" | "engine.exit.request" | "app.exit" => {
            (FrontendKeycapKind::Back, "EXITING")
        }
        "ui.back" => (FrontendKeycapKind::Back, "RETURN"),
        "game.start" => (FrontendKeycapKind::Select, "START"),
        "engine.settings.open" | "game.credits" => (FrontendKeycapKind::Select, "OPEN"),
        "settings.apply" => (FrontendKeycapKind::Select, "APPLY"),
        action if action.starts_with("settings.") => (FrontendKeycapKind::Select, "CHANGE"),
        _ => (FrontendKeycapKind::Select, "SELECT"),
    }
}

fn animate_frontend_keycap_feedback(draw: &mut newengine_ui_api::UiDrawList) {
    let feedback = {
        let mut feedback = frontend_keycap_feedback()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(current) = feedback.as_ref() else {
            return;
        };
        if current.started_at.elapsed() >= FRONTEND_KEYCAP_FEEDBACK_DURATION {
            *feedback = None;
            return;
        }
        current.clone()
    };
    let elapsed = feedback.started_at.elapsed();
    let press = frontend_keycap_press_amount(elapsed);
    let key_token = match feedback.kind {
        FrontendKeycapKind::Select => ".hint.select.",
        FrontendKeycapKind::Back => ".hint.back.",
    };

    let mut transformed = Vec::with_capacity(draw.paint.commands.len() + 2);
    for mut command in std::mem::take(&mut draw.paint.commands) {
        match &mut command {
            newengine_ui_api::UiPaintCommand::Image(image)
                if image.node.node_id.contains(key_token)
                    && image.node.node_id.ends_with("keycap") =>
            {
                let original = image.rect;
                if press > 0.001 {
                    let mut well_node = image.node.clone();
                    well_node.node_id = format!("{}.pressed-well", well_node.node_id);
                    well_node.role = "keycap-pressed-well".to_owned();
                    well_node.z_index = well_node.z_index.saturating_sub(1);
                    transformed.push(newengine_ui_api::UiPaintCommand::Rect(
                        newengine_ui_api::UiRectPaintCommand {
                            node: well_node,
                            rect: [
                                original[0] + 1.0,
                                original[1] + 2.0,
                                (original[2] - 2.0).max(1.0),
                                (original[3] - 2.0).max(1.0),
                            ],
                            color: lerp_rgba_u32(
                                rgba_u32(32, 20, 12, 90),
                                rgba_u32(124, 78, 43, 225),
                                press,
                            ),
                            clip_rect: image.clip_rect,
                        },
                    ));
                }

                let target_w = original[2] * (1.0 - 0.08 * press);
                let target_h = original[3] * (1.0 - 0.18 * press);
                image.rect[0] = original[0] + (original[2] - target_w) * 0.5;
                // Bottom-anchored compression plus a small downward travel creates
                // a readable physical key press at 1600x900.
                image.rect[1] = original[1] + (original[3] - target_h) + 1.5 * press;
                image.rect[2] = target_w.max(1.0);
                image.rect[3] = target_h.max(1.0);
                image.tint_rgba =
                    lerp_rgba_u32(image.tint_rgba, rgba_u32(255, 216, 174, 255), 0.92 * press);
            }
            newengine_ui_api::UiPaintCommand::Text(text)
                if text.node.node_id.contains(key_token) && text.node.node_id.ends_with("text") =>
            {
                text.text = feedback.label.clone();
                text.rect[0] += 1.0 * press;
                text.rect[1] += 3.5 * press;
                text.color = lerp_rgba_u32(text.color, rgba_u32(255, 232, 204, 255), 0.96 * press);
                text.letter_spacing_px += 0.28 * press;
            }
            _ => {}
        }
        transformed.push(command);
    }
    draw.paint.commands = transformed;
}

fn frontend_keycap_press_amount(elapsed: Duration) -> f32 {
    let elapsed_ms = elapsed.as_secs_f32() * 1_000.0;
    const ATTACK_MS: f32 = 45.0;
    const HOLD_UNTIL_MS: f32 = 190.0;
    let duration_ms = FRONTEND_KEYCAP_FEEDBACK_DURATION.as_secs_f32() * 1_000.0;
    if elapsed_ms <= ATTACK_MS {
        smoothstep01(elapsed_ms / ATTACK_MS)
    } else if elapsed_ms <= HOLD_UNTIL_MS {
        1.0
    } else {
        smoothstep01((duration_ms - elapsed_ms) / (duration_ms - HOLD_UNTIL_MS).max(1.0))
    }
}

fn smoothstep01(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

const fn rgba_u32(r: u8, g: u8, b: u8, a: u8) -> u32 {
    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16) | ((a as u32) << 24)
}

fn lerp_rgba_u32(from: u32, to: u32, amount: f32) -> u32 {
    let amount = amount.clamp(0.0, 1.0);
    let channel = |shift: u32| -> u8 {
        let a = ((from >> shift) & 0xff) as f32;
        let b = ((to >> shift) & 0xff) as f32;
        (a + (b - a) * amount).round().clamp(0.0, 255.0) as u8
    };
    rgba_u32(channel(0), channel(8), channel(16), channel(24))
}

fn frontend_settings_pending() -> &'static Mutex<BTreeMap<String, serde_json::Value>> {
    static PENDING: OnceLock<Mutex<BTreeMap<String, serde_json::Value>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn frontend_settings_last_changed() -> &'static Mutex<Option<Instant>> {
    static LAST_CHANGED: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
    LAST_CHANGED.get_or_init(|| Mutex::new(None))
}

fn mark_frontend_settings_changed() {
    *frontend_settings_last_changed()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(Instant::now());
}

fn frontend_settings_debounce_due() -> bool {
    frontend_settings_last_changed()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .is_some_and(|changed_at| changed_at.elapsed() >= FRONTEND_SETTINGS_SAVE_DEBOUNCE)
}

fn clear_frontend_settings_changed_at() {
    *frontend_settings_last_changed()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
}

fn lock_frontend_settings_pending(
) -> std::sync::MutexGuard<'static, BTreeMap<String, serde_json::Value>> {
    frontend_settings_pending()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn stage_frontend_setting_actions(frame: &UiEventDispatchFrame) {
    let mut changed = false;
    let mut pending = lock_frontend_settings_pending();
    for action in &frame.actions {
        if action.trigger != newengine_ui_api::UiNodeEventTrigger::ValueChanged
            || !action.action_id.starts_with("settings.")
        {
            continue;
        }
        let Some(value) = action.payload.get("value") else {
            continue;
        };
        pending.insert(action.action_id.clone(), value.clone());
        changed = true;
    }
    drop(pending);
    if changed {
        mark_frontend_settings_changed();
    }
}

fn frontend_settings_apply_requested(frame: &UiEventDispatchFrame) -> bool {
    frame.actions.iter().any(|action| {
        action.trigger == newengine_ui_api::UiNodeEventTrigger::Click
            && action.action_id == "settings.apply"
    })
}

fn persist_frontend_settings() -> Result<usize, String> {
    let changes = {
        let mut pending = lock_frontend_settings_pending();
        std::mem::take(&mut *pending)
    };
    clear_frontend_settings_changed_at();
    if changes.is_empty() {
        return Ok(0);
    }

    let config_path = std::env::current_dir()
        .map_err(|error| format!("resolve current directory: {error}"))?
        .join("config.json");
    let source = std::fs::read_to_string(&config_path)
        .map_err(|error| format!("read '{}': {error}", config_path.display()))?;
    let mut document: serde_json::Value = serde_json::from_str(&source)
        .map_err(|error| format!("parse '{}': {error}", config_path.display()))?;

    let mut applied = 0usize;
    for (action_id, value) in changes {
        if apply_frontend_setting_value(&mut document, action_id.as_str(), &value) {
            applied += 1;
        }
    }
    if applied == 0 {
        return Ok(0);
    }
    let encoded = serde_json::to_string_pretty(&document)
        .map_err(|error| format!("encode settings config: {error}"))?;
    std::fs::write(&config_path, format!("{encoded}\n"))
        .map_err(|error| format!("write '{}': {error}", config_path.display()))?;
    Ok(applied)
}

fn apply_frontend_setting_value(
    document: &mut serde_json::Value,
    action_id: &str,
    value: &serde_json::Value,
) -> bool {
    match action_id {
        "settings.display.fullscreen" => {
            let Some(enabled) = json_setting_bool(value) else {
                return false;
            };
            let mode = serde_json::Value::String(
                if enabled {
                    "exclusive_fullscreen"
                } else {
                    "windowed"
                }
                .to_owned(),
            );
            set_json_pointer(
                document,
                "/startup_settings/display/window_mode",
                mode.clone(),
            );
            set_json_pointer(document, "/window/display/window_mode", mode.clone());
            set_json_pointer(
                document,
                "/plugins/newengine/startup_window/display/window_mode",
                mode.clone(),
            );
            set_json_pointer(
                document,
                "/plugins/newengine/startup_window/display/fullscreen",
                serde_json::json!(enabled),
            );
            set_json_pointer(
                document,
                "/plugins/engine.platform.winit/display/window_mode",
                mode,
            );
            set_json_pointer(
                document,
                "/plugins/engine.platform.winit/display/fullscreen",
                serde_json::json!(enabled),
            );
            true
        }
        "settings.display.vsync" => {
            let Some(enabled) = json_setting_bool(value) else {
                return false;
            };
            for pointer in [
                "/startup_settings/display/vsync",
                "/window/display/vsync",
                "/plugins/newengine/startup_window/display/vsync",
                "/plugins/engine.platform.winit/display/vsync",
            ] {
                set_json_pointer(document, pointer, serde_json::json!(enabled));
            }
            true
        }
        "settings.display.render_scale" => {
            let Some(scale) = json_setting_f64(value).map(|value| value.clamp(0.5, 1.5)) else {
                return false;
            };
            for pointer in [
                "/startup_settings/display/render_scale",
                "/window/display/render_scale",
                "/plugins/newengine/startup_window/display/render_scale",
                "/plugins/engine.platform.winit/display/render_scale",
            ] {
                set_json_pointer(document, pointer, serde_json::json!(scale));
            }
            true
        }
        "settings.graphics.bloom"
        | "settings.graphics.motion_blur"
        | "settings.graphics.depth_of_field"
        | "settings.graphics.sun_rays"
        | "settings.graphics.shadows" => {
            let Some(enabled) = json_setting_bool(value) else {
                return false;
            };
            let field = match action_id {
                "settings.graphics.bloom" => "bloom_enabled",
                "settings.graphics.motion_blur" => "motion_blur_enabled",
                "settings.graphics.depth_of_field" => "depth_of_field_enabled",
                "settings.graphics.sun_rays" => "sun_rays_enabled",
                "settings.graphics.shadows" => "shadows_enabled",
                _ => unreachable!(),
            };
            set_json_pointer(
                document,
                format!("/startup_settings/graphics/{field}").as_str(),
                serde_json::json!(enabled),
            );
            set_json_pointer(
                document,
                "/startup_settings/graphics/preset",
                serde_json::Value::String("custom".to_owned()),
            );
            true
        }
        _ => false,
    }
}

fn set_json_pointer(document: &mut serde_json::Value, pointer: &str, value: serde_json::Value) {
    if let Some(slot) = document.pointer_mut(pointer) {
        *slot = value;
    }
}

fn json_setting_bool(value: &serde_json::Value) -> Option<bool> {
    value.as_bool().or_else(|| {
        value
            .as_str()
            .and_then(|text| match text.trim().to_ascii_lowercase().as_str() {
                "true" | "1" | "on" | "yes" | "checked" | "selected" => Some(true),
                "false" | "0" | "off" | "no" | "unchecked" | "unselected" => Some(false),
                _ => None,
            })
    })
}

fn json_setting_f64(value: &serde_json::Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str()?.trim().parse::<f64>().ok())
}

fn provider_draw_has_active_animation(draw: &newengine_ui_api::UiDrawList) -> bool {
    draw.paint.commands.iter().any(|command| match command {
        newengine_ui_api::UiPaintCommand::Rect(rect) => {
            rect.node.role == "hover-underline-animated"
        }
        _ => false,
    })
}

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

        let scene_launch_status = self.engine.resources.get::<SceneLaunchStatus>().cloned();
        let presentation_blocks_world_bootstrap = self
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
        let gameplay_hud_refresh_due =
            provider_gameplay_hud && (ui_frame_index <= 4 || ui_frame_index % 4 == 1);
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
            || gameplay_hud_refresh_due
            || ui_frame_index % 120 == 1;
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

fn loading_overlay_requires_immediate_publish(
    previous: Option<&ScreenOverlayStatus>,
    next: &ScreenOverlayStatus,
) -> bool {
    previous.is_none_or(|previous| {
        previous.kind != next.kind
            || previous.reason != next.reason
            || previous.title != next.title
            || previous.terminal != next.terminal
    })
}

#[inline]
fn effective_scene_launch_active(
    status: Option<&SceneLaunchStatus>,
    presentation_blocks_world_bootstrap: bool,
) -> bool {
    status.is_some_and(|status| status.active) && !presentation_blocks_world_bootstrap
}

#[cfg(test)]
mod presentation_loading_lifecycle_tests {
    use super::*;

    #[test]
    fn authored_frontend_suppresses_stale_scene_loading_status() {
        let status = SceneLaunchStatus::loading("Loading", "Preparing", "stale", 0.95);
        assert!(!effective_scene_launch_active(Some(&status), true));
    }

    #[test]
    fn loading_state_reactivates_runtime_preloader_after_start() {
        let status = SceneLaunchStatus::loading("Loading", "Preparing", "active", 0.95);
        assert!(effective_scene_launch_active(Some(&status), false));
    }

    #[test]
    fn inactive_scene_status_never_opens_loading_surface() {
        let status = SceneLaunchStatus::inactive();
        assert!(!effective_scene_launch_active(Some(&status), false));
        assert!(!effective_scene_launch_active(None, false));
    }
}

#[cfg(test)]
mod animation_refresh_tests {
    use super::{
        animate_frontend_keycap_feedback, apply_frontend_setting_value,
        begin_frontend_keycap_feedback, frontend_action_keycap, frontend_keycap_press_amount,
        provider_draw_has_active_animation, ui_dispatch_requests_exit, FrontendKeycapKind,
    };
    use newengine_ui_api::{
        UiDrawList, UiEventDispatchFrame, UiPaintCommand, UiPaintNodeRef, UiRectPaintCommand,
    };
    use std::time::Duration;

    #[test]
    fn authored_exit_action_requests_native_close() {
        let mut frame = UiEventDispatchFrame::default();
        assert!(!ui_dispatch_requests_exit(&frame));
        frame.actions.push(newengine_ui_api::UiActionDispatch {
            action_id: "engine.lifecycle.exit".to_owned(),
            trigger: newengine_ui_api::UiNodeEventTrigger::Click,
            ..newengine_ui_api::UiActionDispatch::default()
        });
        assert!(ui_dispatch_requests_exit(&frame));
    }

    #[test]
    fn frontend_action_maps_to_expected_keycap_feedback() {
        assert_eq!(
            frontend_action_keycap("engine.settings.open"),
            (FrontendKeycapKind::Select, "OPEN")
        );
        assert_eq!(
            frontend_action_keycap("settings.apply"),
            (FrontendKeycapKind::Select, "APPLY")
        );
        assert_eq!(
            frontend_action_keycap("ui.back"),
            (FrontendKeycapKind::Back, "RETURN")
        );
        assert_eq!(
            frontend_action_keycap("engine.lifecycle.exit"),
            (FrontendKeycapKind::Back, "EXITING")
        );
    }

    #[test]
    fn keycap_feedback_changes_only_matching_keycap_commands() {
        let mut draw = newengine_ui_api::UiDrawList::new();
        draw.paint.push(newengine_ui_api::UiPaintCommand::Image(
            newengine_ui_api::UiImagePaintCommand {
                node: newengine_ui_api::UiPaintNodeRef {
                    node_id: "main.hint.select.keycap".to_owned(),
                    ..Default::default()
                },
                rect: [100.0, 100.0, 42.0, 33.0],
                tint_rgba: 0xffff_ffff,
                ..Default::default()
            },
        ));
        draw.paint.push(newengine_ui_api::UiPaintCommand::Image(
            newengine_ui_api::UiImagePaintCommand {
                node: newengine_ui_api::UiPaintNodeRef {
                    node_id: "main.hint.back.keycap".to_owned(),
                    ..Default::default()
                },
                rect: [200.0, 100.0, 69.0, 33.0],
                tint_rgba: 0xffff_ffff,
                ..Default::default()
            },
        ));
        begin_frontend_keycap_feedback(FrontendKeycapKind::Select, "OPEN");
        std::thread::sleep(Duration::from_millis(40));
        animate_frontend_keycap_feedback(&mut draw);
        let select = draw
            .paint
            .commands
            .iter()
            .find_map(|command| match command {
                newengine_ui_api::UiPaintCommand::Image(image)
                    if image.node.node_id == "main.hint.select.keycap" =>
                {
                    Some(image)
                }
                _ => None,
            })
            .expect("transformed select keycap");
        let back = draw
            .paint
            .commands
            .iter()
            .find_map(|command| match command {
                newengine_ui_api::UiPaintCommand::Image(image)
                    if image.node.node_id == "main.hint.back.keycap" =>
                {
                    Some(image)
                }
                _ => None,
            })
            .expect("untouched back keycap");
        assert!(draw.paint.commands.iter().any(|command| matches!(
            command,
            newengine_ui_api::UiPaintCommand::Rect(rect)
                if rect.node.role == "keycap-pressed-well"
        )));
        assert!(
            select.rect[1] > 106.0,
            "keycap must visibly travel downward"
        );
        assert!(select.rect[2] < 39.5, "keycap must compress horizontally");
        assert!(select.rect[3] < 28.5, "keycap must compress vertically");
        assert_eq!(back.rect, [200.0, 100.0, 69.0, 33.0]);
    }

    #[test]
    fn keycap_press_curve_has_attack_hold_and_release() {
        assert_eq!(frontend_keycap_press_amount(Duration::ZERO), 0.0);
        assert!(frontend_keycap_press_amount(Duration::from_millis(25)) > 0.4);
        assert_eq!(frontend_keycap_press_amount(Duration::from_millis(80)), 1.0);
        assert_eq!(
            frontend_keycap_press_amount(Duration::from_millis(180)),
            1.0
        );
        let release = frontend_keycap_press_amount(Duration::from_millis(300));
        assert!(release > 0.0 && release < 1.0);
    }

    #[test]
    fn frontend_setting_values_patch_existing_config_fields() {
        let mut document = serde_json::json!({
            "startup_settings": {
                "display": {"vsync": false, "render_scale": 1.0, "window_mode": "windowed"},
                "graphics": {"preset": "cinematic", "bloom_enabled": true}
            },
            "window": {"display": {"vsync": false, "render_scale": 1.0, "window_mode": "windowed"}},
            "plugins": {
                "newengine": {"startup_window": {"display": {"vsync": false, "render_scale": 1.0, "window_mode": "windowed", "fullscreen": false}}},
                "engine.platform.winit": {"display": {"vsync": false, "render_scale": 1.0, "window_mode": "windowed", "fullscreen": false}}
            }
        });
        assert!(apply_frontend_setting_value(
            &mut document,
            "settings.display.fullscreen",
            &serde_json::json!(true),
        ));
        assert!(apply_frontend_setting_value(
            &mut document,
            "settings.display.vsync",
            &serde_json::json!(true),
        ));
        assert!(apply_frontend_setting_value(
            &mut document,
            "settings.display.render_scale",
            &serde_json::json!(1.25),
        ));
        assert!(apply_frontend_setting_value(
            &mut document,
            "settings.graphics.bloom",
            &serde_json::json!(false),
        ));
        assert_eq!(
            document.pointer("/startup_settings/display/window_mode"),
            Some(&serde_json::json!("exclusive_fullscreen"))
        );
        assert_eq!(
            document.pointer("/startup_settings/display/vsync"),
            Some(&serde_json::json!(true))
        );
        assert_eq!(
            document.pointer("/startup_settings/display/render_scale"),
            Some(&serde_json::json!(1.25))
        );
        assert_eq!(
            document.pointer("/startup_settings/graphics/bloom_enabled"),
            Some(&serde_json::json!(false))
        );
        assert_eq!(
            document.pointer("/startup_settings/graphics/preset"),
            Some(&serde_json::json!("custom"))
        );
    }

    #[test]
    fn provider_draw_detects_animated_hover_underline() {
        let mut draw = UiDrawList::new();
        assert!(!provider_draw_has_active_animation(&draw));
        draw.paint.push(UiPaintCommand::Rect(UiRectPaintCommand {
            node: UiPaintNodeRef {
                role: "hover-underline-animated".to_owned(),
                ..UiPaintNodeRef::default()
            },
            rect: [0.0, 0.0, 20.0, 2.0],
            color: 0xffff_ffff,
            clip_rect: None,
        }));
        assert!(provider_draw_has_active_animation(&draw));
    }

    #[test]
    fn selected_underline_does_not_force_continuous_refresh() {
        let mut draw = UiDrawList::new();
        draw.paint.push(UiPaintCommand::Rect(UiRectPaintCommand {
            node: UiPaintNodeRef {
                role: "selected-underline".to_owned(),
                ..UiPaintNodeRef::default()
            },
            rect: [0.0, 0.0, 20.0, 2.0],
            color: 0xffff_ffff,
            clip_rect: None,
        }));
        assert!(!provider_draw_has_active_animation(&draw));
    }
}
