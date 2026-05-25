#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_input_actions_api::engine_action;
use newengine_assets_api::{
    asset_browser_method, AssetBrowserListResponse, AssetBrowserNode,
    AssetBrowserSnapshotResponse, ENGINE_ASSETS_BROWSER_SERVICE_ID,
};
use newengine_core::ModuleCtx;
use newengine_input_actions_api::InputActionFrame;
use newengine_ui_api::{UiRuntimeDebugOverlayTelemetry, UI_SURFACE_EDITOR_ASSET_BROWSER};
use serde_json::Value;
use std::collections::BTreeMap;

use super::super::controller::RuntimeRenderController;

const ACTION_ASSET_BROWSER_TOGGLE: &str = engine_action::ASSET_BROWSER_TOGGLE;
const REFRESH_PERIOD_FRAMES: u64 = 30;
const MAX_LINES: usize = 18;
const DEFAULT_TOGGLE_DEBOUNCE_FRAMES: u64 = 12;

#[inline]
fn asset_browser_toggle_debounce_frames() -> u64 {
    crate::env_config::var_u64(
        "NEWENGINE_ASSET_BROWSER_TOGGLE_DEBOUNCE_FRAMES",
        DEFAULT_TOGGLE_DEBOUNCE_FRAMES,
        0,
        240,
    )
}

impl RuntimeRenderController {
    pub(super) fn update_asset_browser_overlay<E: Send + 'static>(
        &mut self,
        ctx: &mut ModuleCtx<'_, E>,
        actions: &InputActionFrame,
        surface_size_px: [u32; 2],
    ) -> bool {
        if actions.actions.iter().any(|action| action == ACTION_ASSET_BROWSER_TOGGLE) {
            let debounce_frames = asset_browser_toggle_debounce_frames();
            let since_last_toggle = self
                .frame
                .frame_index
                .saturating_sub(self.editor.asset_browser_last_toggle_frame);
            let debounced = self.editor.asset_browser_last_toggle_frame != 0
                && since_last_toggle < debounce_frames;
            if debounced {
                log::debug!(
                    "asset browser overlay: toggle ignored action='{}' frame={} since_last_toggle={} debounce_frames={} surface='{}'",
                    ACTION_ASSET_BROWSER_TOGGLE,
                    self.frame.frame_index,
                    since_last_toggle,
                    debounce_frames,
                    UI_SURFACE_EDITOR_ASSET_BROWSER,
                );
            } else {
                self.editor.asset_browser_open = !self.editor.asset_browser_open;
                self.editor.asset_browser_last_refresh_frame = 0;
                self.editor.asset_browser_last_toggle_frame = self.frame.frame_index;
                log::info!(
                    "asset browser overlay: {} action='{}' surface='{}' route='engine.ui'",
                    if self.editor.asset_browser_open { "opened" } else { "closed" },
                    ACTION_ASSET_BROWSER_TOGGLE,
                    UI_SURFACE_EDITOR_ASSET_BROWSER,
                );
            }
        }

        if !self.editor.asset_browser_open {
            remove_owned_debug_overlay(ctx);
            return false;
        }

        let should_refresh = self.editor.asset_browser_last_refresh_frame == 0
            || self
                .frame
                .frame_index
                .saturating_sub(self.editor.asset_browser_last_refresh_frame)
                >= REFRESH_PERIOD_FRAMES;
        if should_refresh {
            self.editor.asset_browser_last_refresh_frame = self.frame.frame_index;
            let telemetry = asset_browser_debug_overlay(self.frame.frame_index, surface_size_px);
            ctx.resources_mut().insert::<UiRuntimeDebugOverlayTelemetry>(telemetry);
        }
        true
    }
}

fn remove_owned_debug_overlay<E: Send + 'static>(ctx: &mut ModuleCtx<'_, E>) {
    let remove = ctx
        .resources()
        .get::<UiRuntimeDebugOverlayTelemetry>()
        .map(|telemetry| telemetry.source == "engine.assets.browser")
        .unwrap_or(false);
    if remove {
        let _ = ctx.resources_mut().remove::<UiRuntimeDebugOverlayTelemetry>();
    }
}

fn asset_browser_debug_overlay(frame_index: u64, surface_size_px: [u32; 2]) -> UiRuntimeDebugOverlayTelemetry {
    match fetch_snapshot() {
        Ok(snapshot) => telemetry_from_snapshot(frame_index, surface_size_px, snapshot),
        Err(err) => telemetry_error(frame_index, surface_size_px, err),
    }
}

fn fetch_snapshot() -> Result<AssetBrowserSnapshotResponse, String> {
    let Some(bytes) = newengine_core::call_service_v1_optional(
        ENGINE_ASSETS_BROWSER_SERVICE_ID,
        asset_browser_method::SNAPSHOT_V1,
        &[],
    )
    .map_err(|e| e.to_string())? else {
        return Err(format!(
            "service '{}' is not registered",
            ENGINE_ASSETS_BROWSER_SERVICE_ID
        ));
    };
    serde_json::from_slice::<AssetBrowserSnapshotResponse>(&bytes).map_err(|e| {
        format!(
            "service '{}' returned invalid snapshot json: {}",
            ENGINE_ASSETS_BROWSER_SERVICE_ID, e
        )
    })
}

fn telemetry_from_snapshot(
    frame_index: u64,
    surface_size_px: [u32; 2],
    snapshot: AssetBrowserSnapshotResponse,
) -> UiRuntimeDebugOverlayTelemetry {
    let mut lines = Vec::new();
    lines.push("ASSET BROWSER // VFS ROOT".to_owned());
    lines.push("F1 toggles Asset Browser through engine.ui  |  ESC opens pause menu  |  @entry opens ListFile dictionaries".to_owned());
    lines.push(format!(
        "root ok={} folders={} assets={} entries={} sources={} warnings={}",
        snapshot.root.ok,
        snapshot.root.folders.len(),
        snapshot.root.assets.len(),
        snapshot.root.entries.len(),
        snapshot.sources.len().max(snapshot.root.sources.len()),
        snapshot.warnings.len() + snapshot.root.warnings.len(),
    ));
    append_nodes(&mut lines, "DIR", &snapshot.root.folders);
    append_nodes(&mut lines, "ASSET", &snapshot.root.assets);
    append_nodes(&mut lines, "ENTRY", &snapshot.root.entries);
    if lines.len() > MAX_LINES {
        lines.truncate(MAX_LINES);
        lines.push("...".to_owned());
    }
    for warning in snapshot.warnings.iter().chain(snapshot.root.warnings.iter()).take(4) {
        lines.push(format!("WARN {warning}"));
    }

    let mut metrics = BTreeMap::new();
    metrics.insert("surface_size_px".to_owned(), serde_json::json!(surface_size_px));
    metrics.insert("root".to_owned(), root_metrics(&snapshot.root));
    metrics.insert("file_type_manifest".to_owned(), snapshot.file_type_manifest);
    metrics.insert("formats".to_owned(), snapshot.formats);

    UiRuntimeDebugOverlayTelemetry {
        version: 1,
        surface_id: UI_SURFACE_EDITOR_ASSET_BROWSER.to_owned(),
        source: "engine.assets.browser".to_owned(),
        frame_index,
        text: lines.join("\n"),
        lines,
        metrics,
    }
}

fn telemetry_error(
    frame_index: u64,
    surface_size_px: [u32; 2],
    err: String,
) -> UiRuntimeDebugOverlayTelemetry {
    let lines = vec![
        "ASSET BROWSER // UNAVAILABLE".to_owned(),
        "F1 toggles Asset Browser through engine.ui  |  ESC opens pause menu".to_owned(),
        err.clone(),
    ];
    let mut metrics = BTreeMap::new();
    metrics.insert("surface_size_px".to_owned(), serde_json::json!(surface_size_px));
    metrics.insert("error".to_owned(), Value::String(err));
    UiRuntimeDebugOverlayTelemetry {
        version: 1,
        surface_id: UI_SURFACE_EDITOR_ASSET_BROWSER.to_owned(),
        source: "engine.assets.browser".to_owned(),
        frame_index,
        text: lines.join("\n"),
        lines,
        metrics,
    }
}

fn append_nodes(lines: &mut Vec<String>, prefix: &str, nodes: &[AssetBrowserNode]) {
    for node in nodes.iter().take(8) {
        let mut label = format!("{prefix} {}", display_node_ref(node));
        if !node.asset_kind.is_empty() {
            label.push_str("  kind=");
            label.push_str(node.asset_kind.as_str());
        }
        if node.has_children {
            label.push_str("  [+]");
        }
        if node.can_update || node.can_delete || node.can_rebuild {
            label.push_str("  write");
        }
        lines.push(label);
    }
}

fn display_node_ref(node: &AssetBrowserNode) -> String {
    node.entry_ref
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            if node.logical_path.trim().is_empty() {
                node.name.clone()
            } else {
                node.logical_path.clone()
            }
        })
}

fn root_metrics(root: &AssetBrowserListResponse) -> Value {
    serde_json::json!({
        "ok": root.ok,
        "location": root.location.canonical_ref(),
        "folders": root.folders.len(),
        "assets": root.assets.len(),
        "entries": root.entries.len(),
        "sources": root.sources.len(),
        "warnings": root.warnings,
    })
}
