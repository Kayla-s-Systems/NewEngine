use std::collections::BTreeSet;

use newengine_assets_api::{ASSETS_UI_BACKEND_CAPABILITY_ID, ENGINE_ASSETS_UI_SERVICE_ID};
use newengine_core::{EngineResult, Module, ModuleCtx};
use newengine_ui_api::{
    UiEventDispatchFrame, UiStatePatch, ENGINE_UI_SERVICE_ID, UI_BACKEND_CAPABILITY_ID,
    UI_SERVICE_METHOD_APPLY_NODE_REQUEST_V1, UI_SERVICE_METHOD_APPLY_STATE_PATCH_V1,
};

use crate::options::SURFACE_ID;
use crate::ui_document::{build_aurelia_ui_test_request, AureliaUiTestRouteStatus};

#[derive(Default)]
pub struct AureliaUiTestSurfaceModule {
    published_once: bool,
    last_publish_frame: u64,
    last_state_patch_frame: u64,
    last_published_click_count: u64,
    last_consumed_dispatch_frame: u64,
    state_patch_warning_logged: bool,
    click_count: u64,
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

        let needs_publish = !self.published_once
            || self.click_count != self.last_published_click_count
            || frame_index.saturating_sub(self.last_publish_frame) >= 240;
        if needs_publish && self.publish_surface(frame_index) {
            self.published_once = true;
            self.last_publish_frame = frame_index;
            self.last_published_click_count = self.click_count;
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
                    self.click_count = self.click_count.saturating_add(1);
                    newengine_ulog_api::ulog::info!(
                        "AureliaUiTest: action '{}' from surface='{}' node='{}' dispatch_frame={} clicks={}",
                        action.action_id,
                        action.surface_id,
                        action.node_id,
                        dispatch.frame_index,
                        self.click_count
                    );
                }
                "aurelia_ui_test.quit" => {
                    newengine_ulog_api::ulog::info!("AureliaUiTest: quit requested from UI gateway action dispatch_frame={}", dispatch.frame_index);
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
                newengine_ulog_api::ulog::error!("AureliaUiTest: failed to encode UI node request: {err}");
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
                newengine_ulog_api::ulog::warn!("AureliaUiTest: engine.ui route unavailable during publish");
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
                serde_json::json!(if route_status.route_available { "engine.ui active" } else { "waiting" }),
            )
            .with_change(
                "status.capability.value",
                "text",
                serde_json::json!(if route_status.ui_backend_capability { "ui.backend yes" } else { "missing" }),
            )
            .with_change(
                "status.transport.value",
                "text",
                serde_json::json!(route_status.frame_mode_text()),
            )
            .with_change("status.frame.value", "text", serde_json::json!(frame_text))
            .with_change("status.frame.value", "value", serde_json::json!(frame_text))
            .with_change("status.clicks.value", "text", serde_json::json!(clicks_text))
            .with_change("status.clicks.value", "value", serde_json::json!(clicks_text))
            .with_change("showcase.slider", "value", serde_json::json!(format!("{pulse:.2}")))
            .with_change("showcase.progress", "value", serde_json::json!(format!("{:.0}%", pulse * 100.0)))
            .with_change(
                "diag.4",
                "text",
                serde_json::json!(format!("✓ live frame patch: {}", route_status.frame_mode_text())),
            );

        let payload = match serde_json::to_vec(&patch) {
            Ok(payload) => payload,
            Err(err) => {
                newengine_ulog_api::ulog::warn!("AureliaUiTest: failed to encode live UI state patch: {err}");
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
                newengine_ulog_api::ulog::warn!("AureliaUiTest: engine.ui route did not accept live state patch");
            }
            Ok(None) => {}
            Err(err) if !self.state_patch_warning_logged || frame_index % 240 == 1 => {
                self.state_patch_warning_logged = true;
                newengine_ulog_api::ulog::warn!("AureliaUiTest: live UI state patch failed: {err}");
            }
            Err(_) => {}
        }
    }

    fn route_status(&self) -> AureliaUiTestRouteStatus {
        let route_available = newengine_core::has_engine_gateway_route(ENGINE_UI_SERVICE_ID);
        let ui_backend_capability = newengine_core::engine_gateway_has_capability(
            ENGINE_UI_SERVICE_ID,
            UI_BACKEND_CAPABILITY_ID,
        );
        let assets_ui_available = newengine_core::has_engine_gateway_route(ENGINE_ASSETS_UI_SERVICE_ID)
            || newengine_core::engine_gateway_has_capability(
                ENGINE_ASSETS_UI_SERVICE_ID,
                ASSETS_UI_BACKEND_CAPABILITY_ID,
            );
        AureliaUiTestRouteStatus::new(route_available, ui_backend_capability, true, assets_ui_available)
    }
}
