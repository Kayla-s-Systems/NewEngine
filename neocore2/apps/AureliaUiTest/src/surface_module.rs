use std::collections::BTreeSet;

use newengine_assets_api::{ASSETS_UI_BACKEND_CAPABILITY_ID, ENGINE_ASSETS_UI_SERVICE_ID};
use newengine_core::{EngineResult, Module, ModuleCtx};
use newengine_ui_api::{
    UiEventDispatchFrame, UiNodeEventTrigger, UiStatePatch, ENGINE_UI_SERVICE_ID,
    UI_BACKEND_CAPABILITY_ID, UI_SERVICE_METHOD_APPLY_NODE_REQUEST_V1,
    UI_SERVICE_METHOD_APPLY_STATE_PATCH_V1,
};

use crate::options::SURFACE_ID;
use crate::ui_document::{build_aurelia_ui_test_request, AureliaUiTestRouteStatus};

pub struct AureliaUiTestSurfaceModule {
    published_once: bool,
    last_publish_frame: u64,
    last_state_patch_frame: u64,
    last_consumed_dispatch_frame: u64,
    state_patch_warning_logged: bool,
    click_count: u64,
    input_text: String,
    checkbox_checked: bool,
    toggle_on: bool,
    select_value: String,
    slider_value: f32,
}

impl Default for AureliaUiTestSurfaceModule {
    fn default() -> Self {
        Self {
            published_once: false,
            last_publish_frame: 0,
            last_state_patch_frame: 0,
            last_consumed_dispatch_frame: 0,
            state_patch_warning_logged: false,
            click_count: 0,
            input_text: "engine.ui ? ui ? demo".to_owned(),
            checkbox_checked: false,
            toggle_on: false,
            select_value: "Option Beta".to_owned(),
            slider_value: 0.40,
        }
    }
}

impl Module<()> for AureliaUiTestSurfaceModule {
    #[inline]
    fn id(&self) -> &'static str {
        "apps.aurelia_ui_test.surface"
    }

    fn start(&mut self, _ctx: &mut ModuleCtx<'_, ()>) -> EngineResult<()> {
        Ok(())
    }

    fn update(&mut self, ctx: &mut ModuleCtx<'_, ()>) -> EngineResult<()> {
        let frame_index = ctx.frame().map(|frame| frame.frame_index).unwrap_or(0);
        self.consume_actions(ctx);

        // Publish the authored tree once. Widget-local state (text input, checkbox,
        // slider drag) is owned by the UI provider through state patches; periodic
        // full-tree republishes reset controls and make inputs feel broken.
        let needs_publish = !self.published_once;
        if needs_publish && self.publish_surface(frame_index) {
            self.published_once = true;
            self.last_publish_frame = frame_index;
        }

        if self.published_once {
            self.publish_live_state_patch(frame_index);
        }

        Ok(())
    }
}

impl AureliaUiTestSurfaceModule {
    fn consume_actions(&mut self, ctx: &mut ModuleCtx<'_, ()>) {
        let Some(dispatch) = ctx.resources().get::<UiEventDispatchFrame>() else {
            return;
        };
        if dispatch.frame_index <= self.last_consumed_dispatch_frame {
            return;
        }
        self.last_consumed_dispatch_frame = dispatch.frame_index;

        let mut seen_actions = BTreeSet::new();
        let mut request_exit = false;
        for action in &dispatch.actions {
            let dedupe_key = (
                action.action_id.as_str(),
                action.surface_id.as_str(),
                action.node_id.as_str(),
                action.trigger.as_str(),
                action_has_value_payload(action),
            );
            if !seen_actions.insert(dedupe_key) {
                newengine_ulog_api::ulog::warn!(
                    "AureliaUiTest: duplicate UI action suppressed action='{}' surface='{}' node='{}' dispatch_frame={}",
                    action.action_id,
                    action.surface_id,
                    action.node_id,
                    dispatch.frame_index
                );
                continue;
            }

            match action.action_id.as_str() {
                "aurelia_ui_test.click" => {
                    if action.trigger == UiNodeEventTrigger::Click {
                        self.click_count = self.click_count.saturating_add(1);
                        let click_debug = ui_click_debug_suffix(action);
                        newengine_ulog_api::ulog::info!(
                            "AureliaUiTest: action '{}' from surface='{}' node='{}' trigger={} dispatch_frame={} clicks={}{}",
                            action.action_id,
                            action.surface_id,
                            action.node_id,
                            action.trigger.as_str(),
                            dispatch.frame_index,
                            self.click_count,
                            click_debug
                        );
                    } else {
                        newengine_ulog_api::ulog::info!(
                            "AureliaUiTest: ignored non-click button action action='{}' surface='{}' node='{}' trigger={} dispatch_frame={}",
                            action.action_id,
                            action.surface_id,
                            action.node_id,
                            action.trigger.as_str(),
                            dispatch.frame_index,
                        );
                    }
                }
                "aurelia_ui_test.input" => {
                    if let Some(value) = action_payload_string(action) {
                        self.input_text = value;
                        self.publish_control_state_patch(dispatch.frame_index);
                    } else {
                        log_ignored_value_action("input", action, dispatch.frame_index);
                    }
                }
                "aurelia_ui_test.checkbox" => {
                    if let Some(value) = action_payload_bool(action) {
                        self.checkbox_checked = value;
                        self.publish_control_state_patch(dispatch.frame_index);
                    } else {
                        log_ignored_value_action("checkbox", action, dispatch.frame_index);
                    }
                }
                "aurelia_ui_test.toggle" => {
                    if let Some(value) = action_payload_bool(action) {
                        self.toggle_on = value;
                        self.publish_control_state_patch(dispatch.frame_index);
                    } else {
                        log_ignored_value_action("toggle", action, dispatch.frame_index);
                    }
                }
                "aurelia_ui_test.select" => {
                    if let Some(value) = action_payload_string(action) {
                        self.select_value = value;
                        self.publish_control_state_patch(dispatch.frame_index);
                    } else {
                        log_ignored_value_action("select", action, dispatch.frame_index);
                    }
                }
                "aurelia_ui_test.slider" => {
                    if let Some(value) = action_payload_f32(action) {
                        self.slider_value = value.clamp(0.0, 1.0);
                        self.publish_control_state_patch(dispatch.frame_index);
                    } else {
                        log_ignored_value_action("slider", action, dispatch.frame_index);
                    }
                }
                "aurelia_ui_test.quit" => {
                    newengine_ulog_api::ulog::info!(
                        "AureliaUiTest: quit requested from UI gateway action dispatch_frame={}",
                        dispatch.frame_index
                    );
                    request_exit = true;
                }
                _ => {}
            }
        }

        if request_exit {
            ctx.request_exit();
        }
    }

    fn publish_surface(&self, frame_index: u64) -> bool {
        let route_status = self.route_status();
        if !route_status.assets_ui_available {
            if !self.published_once || frame_index % 120 == 0 {
                newengine_ulog_api::ulog::warn!(
                    "AureliaUiTest: engine.assets.ui is not registered yet; waiting before compiling Showcase .neui"
                );
            }
            return false;
        }

        if !route_status.route_available {
            if !self.published_once {
                newengine_ulog_api::ulog::warn!(
                    "AureliaUiTest: engine.ui route is not available yet; waiting for active UI provider"
                );
            }
            return false;
        }

        if !route_status.ui_backend_capability {
            if !self.published_once {
                newengine_ulog_api::ulog::warn!(
                    "AureliaUiTest: engine.ui route is active but required capability '{}' is missing",
                    UI_BACKEND_CAPABILITY_ID
                );
            }
            return false;
        }

        let request = build_aurelia_ui_test_request(frame_index, self.click_count, route_status);
        let payload = match serde_json::to_vec(&request) {
            Ok(payload) => payload,
            Err(err) => {
                newengine_ulog_api::ulog::error!(
                    "AureliaUiTest: failed to encode UI node request: {err}"
                );
                return false;
            }
        };

        match newengine_core::call_service_v1_optional(
            ENGINE_UI_SERVICE_ID,
            UI_SERVICE_METHOD_APPLY_NODE_REQUEST_V1,
            &payload,
        ) {
            Ok(Some(bytes)) => {
                if frame_index == 0 || frame_index % 240 == 1 {
                    newengine_ulog_api::ulog::info!(
                        "AureliaUiTest: published provider-neutral UI node tree to engine.ui bytes={} response_bytes={}",
                        payload.len(),
                        bytes.len()
                    );
                }
                true
            }
            Ok(None) => {
                newengine_ulog_api::ulog::warn!(
                    "AureliaUiTest: engine.ui route unavailable during publish"
                );
                false
            }
            Err(err) => {
                newengine_ulog_api::ulog::warn!("AureliaUiTest: engine.ui publish failed: {err}");
                false
            }
        }
    }

    fn publish_live_state_patch(&mut self, frame_index: u64) {
        if frame_index == self.last_state_patch_frame {
            return;
        }
        if !newengine_core::has_engine_gateway_route(ENGINE_UI_SERVICE_ID) {
            return;
        }

        let route_status = self.route_status();
        let pulse = ((frame_index % 180) as f32) / 179.0;
        let frame_text = frame_index.to_string();
        let clicks_text = self.click_count.to_string();
        let patch = UiStatePatch::new(frame_index, SURFACE_ID)
            .with_change(
                "status.route.value",
                "text",
                serde_json::json!(if route_status.route_available {
                    "engine.ui active"
                } else {
                    "waiting"
                }),
            )
            .with_change(
                "status.capability.value",
                "text",
                serde_json::json!(if route_status.ui_backend_capability {
                    "ui.backend yes"
                } else {
                    "missing"
                }),
            )
            .with_change(
                "status.transport.value",
                "text",
                serde_json::json!(route_status.frame_mode_text()),
            )
            .with_change("status.frame.value", "text", serde_json::json!(frame_text))
            .with_change("status.frame.value", "value", serde_json::json!(frame_text))
            .with_change(
                "status.clicks.value",
                "text",
                serde_json::json!(clicks_text),
            )
            .with_change(
                "status.clicks.value",
                "value",
                serde_json::json!(clicks_text),
            )
            .with_change(
                "showcase.progress",
                "value",
                serde_json::json!(format!("{:.0}%", pulse * 100.0)),
            )
            .with_change(
                "diag.4",
                "text",
                serde_json::json!(format!(
                    "✓ live frame patch: {}",
                    route_status.frame_mode_text()
                )),
            );

        let payload = match serde_json::to_vec(&patch) {
            Ok(payload) => payload,
            Err(err) => {
                newengine_ulog_api::ulog::warn!(
                    "AureliaUiTest: failed to encode live UI state patch: {err}"
                );
                return;
            }
        };

        match newengine_core::call_service_v1_optional(
            ENGINE_UI_SERVICE_ID,
            UI_SERVICE_METHOD_APPLY_STATE_PATCH_V1,
            &payload,
        ) {
            Ok(Some(_)) => {
                self.last_state_patch_frame = frame_index;
            }
            Ok(None) if !self.state_patch_warning_logged || frame_index % 240 == 1 => {
                self.state_patch_warning_logged = true;
                newengine_ulog_api::ulog::warn!(
                    "AureliaUiTest: engine.ui route did not accept live state patch"
                );
            }
            Ok(None) => {}
            Err(err) if !self.state_patch_warning_logged || frame_index % 240 == 1 => {
                self.state_patch_warning_logged = true;
                newengine_ulog_api::ulog::warn!("AureliaUiTest: live UI state patch failed: {err}");
            }
            Err(_) => {}
        }
    }

    fn publish_control_state_patch(&mut self, frame_index: u64) {
        if !newengine_core::has_engine_gateway_route(ENGINE_UI_SERVICE_ID) {
            return;
        }

        let patch = UiStatePatch::new(frame_index, SURFACE_ID)
            .with_change(
                "framework.value",
                "nodes.input.text/value",
                serde_json::json!(self.input_text),
            )
            .with_change(
                "framework.value",
                "nodes.input.checkbox/value",
                serde_json::json!(self.checkbox_checked),
            )
            .with_change(
                "framework.value",
                "nodes.input.toggle/value",
                serde_json::json!(self.toggle_on),
            )
            .with_change(
                "framework.value",
                "nodes.select.mode/value",
                serde_json::json!(self.select_value),
            )
            .with_change(
                "framework.value",
                "nodes.showcase.slider/value",
                serde_json::json!(self.slider_value),
            )
            .with_change(
                "framework.value",
                "nodes.showcase.progress/value",
                serde_json::json!(self.slider_value),
            );

        let payload = match serde_json::to_vec(&patch) {
            Ok(payload) => payload,
            Err(err) => {
                newengine_ulog_api::ulog::warn!(
                    "AureliaUiTest: failed to encode control state patch: {err}"
                );
                return;
            }
        };

        match newengine_core::call_service_v1_optional(
            ENGINE_UI_SERVICE_ID,
            UI_SERVICE_METHOD_APPLY_STATE_PATCH_V1,
            &payload,
        ) {
            Ok(Some(_)) => {
                self.last_state_patch_frame = frame_index;
                newengine_ulog_api::ulog::info!(
                    "AureliaUiTest: control state patch applied frame={} input='{}' checkbox={} toggle={} select='{}' slider={:.2}",
                    frame_index,
                    self.input_text,
                    self.checkbox_checked,
                    self.toggle_on,
                    self.select_value,
                    self.slider_value
                );
            }
            Ok(None) => {
                newengine_ulog_api::ulog::warn!(
                    "AureliaUiTest: engine.ui route did not accept control state patch"
                );
            }
            Err(err) => {
                newengine_ulog_api::ulog::warn!("AureliaUiTest: control state patch failed: {err}");
            }
        }
    }

    fn route_status(&self) -> AureliaUiTestRouteStatus {
        let route_available = newengine_core::has_engine_gateway_route(ENGINE_UI_SERVICE_ID);
        let ui_backend_capability = newengine_core::engine_gateway_has_capability(
            ENGINE_UI_SERVICE_ID,
            UI_BACKEND_CAPABILITY_ID,
        );
        let assets_ui_available =
            newengine_core::has_engine_gateway_route(ENGINE_ASSETS_UI_SERVICE_ID)
                || newengine_core::engine_gateway_has_capability(
                    ENGINE_ASSETS_UI_SERVICE_ID,
                    ASSETS_UI_BACKEND_CAPABILITY_ID,
                );
        AureliaUiTestRouteStatus::new(
            route_available,
            ui_backend_capability,
            true,
            assets_ui_available,
        )
    }
}

fn action_has_value_payload(action: &newengine_ui_api::UiActionDispatch) -> bool {
    action_payload_string(action).is_some()
        || action_payload_bool(action).is_some()
        || action_payload_f32(action).is_some()
}

fn log_ignored_value_action(
    control: &str,
    action: &newengine_ui_api::UiActionDispatch,
    dispatch_frame: u64,
) {
    newengine_ulog_api::ulog::info!(
        "AureliaUiTest: ignored {} action without value action='{}' surface='{}' node='{}' trigger={} dispatch_frame={}",
        control,
        action.action_id,
        action.surface_id,
        action.node_id,
        action.trigger.as_str(),
        dispatch_frame,
    );
}

fn action_payload_string(action: &newengine_ui_api::UiActionDispatch) -> Option<String> {
    for key in ["value", "text", "selected", "option"] {
        if let Some(value) = action.payload.get(key).and_then(|value| value.as_str()) {
            return Some(value.to_owned());
        }
    }
    None
}

fn action_payload_bool(action: &newengine_ui_api::UiActionDispatch) -> Option<bool> {
    for key in ["value", "checked", "active", "on"] {
        let Some(value) = action.payload.get(key) else {
            continue;
        };
        if let Some(value) = value.as_bool() {
            return Some(value);
        }
        if let Some(text) = value.as_str() {
            match text.trim().to_ascii_lowercase().as_str() {
                "true" | "1" | "yes" | "on" | "checked" => return Some(true),
                "false" | "0" | "no" | "off" | "unchecked" => return Some(false),
                _ => {}
            }
        }
    }
    None
}

fn action_payload_f32(action: &newengine_ui_api::UiActionDispatch) -> Option<f32> {
    for key in ["value", "progress_01", "progress", "x"] {
        if let Some(value) = action.payload.get(key).and_then(json_f32) {
            return Some(value);
        }
    }
    None
}

fn ui_click_debug_suffix(action: &newengine_ui_api::UiActionDispatch) -> String {
    let bounds = action
        .payload
        .get("global_rect")
        .and_then(json_rect4)
        .map(|rect| {
            format!(
                " button_bounds=[x:{:.1},y:{:.1},w:{:.1},h:{:.1}]",
                rect[0], rect[1], rect[2], rect[3]
            )
        })
        .unwrap_or_else(|| " button_bounds=<missing>".to_owned());
    let local = action
        .payload
        .get("local_pos")
        .and_then(json_vec2)
        .map(|pos| format!(" local_click=[x:{:.1},y:{:.1}]", pos[0], pos[1]))
        .unwrap_or_else(|| " local_click=<missing>".to_owned());
    let global = match (
        action.payload.get("global_rect").and_then(json_rect4),
        action.payload.get("local_pos").and_then(json_vec2),
    ) {
        (Some(rect), Some(local)) => format!(
            " global_click=[x:{:.1},y:{:.1}]",
            rect[0] + local[0],
            rect[1] + local[1]
        ),
        _ => " global_click=<missing>".to_owned(),
    };
    format!("{}{}{}", bounds, local, global)
}

fn json_rect4(value: &serde_json::Value) -> Option<[f32; 4]> {
    let arr = value.as_array()?;
    if arr.len() != 4 {
        return None;
    }
    Some([
        json_f32(&arr[0])?,
        json_f32(&arr[1])?,
        json_f32(&arr[2])?,
        json_f32(&arr[3])?,
    ])
}

fn json_vec2(value: &serde_json::Value) -> Option<[f32; 2]> {
    let arr = value.as_array()?;
    if arr.len() != 2 {
        return None;
    }
    Some([json_f32(&arr[0])?, json_f32(&arr[1])?])
}

fn json_f32(value: &serde_json::Value) -> Option<f32> {
    match value {
        serde_json::Value::Number(number) => number.as_f64().map(|value| value as f32),
        serde_json::Value::String(text) => text.trim().parse::<f32>().ok(),
        _ => None,
    }
}
