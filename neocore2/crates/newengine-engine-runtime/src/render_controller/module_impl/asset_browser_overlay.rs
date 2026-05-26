#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_input_actions_api::engine_action;
use newengine_assets_api::{
    asset_browser_method, AssetBrowserListResponse, AssetBrowserNode,
    AssetBrowserSnapshotResponse, ENGINE_ASSETS_BROWSER_SERVICE_ID,
};
use newengine_core::ModuleCtx;
use newengine_input_actions_api::InputActionFrame;
use newengine_ui_api::{
    UiComponentNode, UiFontStyle, UiSurfaceAnchor, UiSurfaceNode, UiSurfaceStyle,
    UI_COMPONENT_PANEL, UI_THEME_NORTHSTAR_DEFAULT, UI_SURFACE_EDITOR_ASSET_BROWSER,
};
use serde_json::Value;
use std::collections::BTreeMap;

use super::super::controller::RuntimeRenderController;

const ACTION_ASSET_BROWSER_TOGGLE: &str = engine_action::ASSET_BROWSER_TOGGLE;
const REFRESH_PERIOD_FRAMES: u64 = 30;
const MAX_LINES: usize = 18;
const ASSET_BROWSER_SOURCE: &str = "engine.assets.browser";

impl RuntimeRenderController {
    pub(super) fn update_asset_browser_overlay<E: Send + 'static>(
        &mut self,
        _ctx: &mut ModuleCtx<'_, E>,
        actions: &InputActionFrame,
        surface_size_px: [u32; 2],
    ) -> bool {
        if actions.actions.iter().any(|action| action == ACTION_ASSET_BROWSER_TOGGLE) {
            // The input binding layer should emit pressed-edge semantic actions. Keep only a
            // same-frame guard here so render runtime does not own user-configurable UI debounce.
            if self.editor.asset_browser_last_toggle_frame == self.frame.frame_index {
                log::debug!(
                    "asset browser surface: duplicate same-frame toggle ignored action='{}' frame={} surface='{}'",
                    ACTION_ASSET_BROWSER_TOGGLE,
                    self.frame.frame_index,
                    UI_SURFACE_EDITOR_ASSET_BROWSER,
                );
            } else {
                self.editor.asset_browser_open = !self.editor.asset_browser_open;
                self.editor.asset_browser_last_refresh_frame = 0;
                self.editor.asset_browser_last_toggle_frame = self.frame.frame_index;
                log::info!(
                    "asset browser surface: {} action='{}' surface='{}' route='engine.ui' node='UiSurfaceNode'",
                    if self.editor.asset_browser_open { "opened" } else { "closed" },
                    ACTION_ASSET_BROWSER_TOGGLE,
                    UI_SURFACE_EDITOR_ASSET_BROWSER,
                );
            }
        }

        if !self.editor.asset_browser_open {
            if self.editor.asset_browser_node.is_some() {
                crate::ui_gateway::publish_surface_node(&UiSurfaceNode::hidden(
                    UI_SURFACE_EDITOR_ASSET_BROWSER,
                    ASSET_BROWSER_SOURCE,
                ));
                self.editor.asset_browser_node = None;
            }
            return false;
        }

        let should_refresh = self.editor.asset_browser_node.is_none()
            || self.editor.asset_browser_last_refresh_frame == 0
            || self
                .frame
                .frame_index
                .saturating_sub(self.editor.asset_browser_last_refresh_frame)
                >= REFRESH_PERIOD_FRAMES;
        if should_refresh {
            self.editor.asset_browser_last_refresh_frame = self.frame.frame_index;
            self.editor.asset_browser_node = Some(asset_browser_surface_node(
                self.frame.frame_index,
                surface_size_px,
            ));
        }

        if let Some(node) = self.editor.asset_browser_node.as_ref() {
            // Publish retained state every visible frame. The payload is small and makes the
            // surface robust against provider reloads, route handoff, and late engine.ui startup.
            crate::ui_gateway::publish_surface_node(node);
        }
        true
    }
}

fn asset_browser_surface_node(frame_index: u64, surface_size_px: [u32; 2]) -> UiSurfaceNode {
    match fetch_snapshot() {
        Ok(snapshot) => node_from_snapshot(frame_index, surface_size_px, snapshot),
        Err(err) => node_error(frame_index, surface_size_px, err),
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

fn node_from_snapshot(
    frame_index: u64,
    surface_size_px: [u32; 2],
    snapshot: AssetBrowserSnapshotResponse,
) -> UiSurfaceNode {
    let mut lines = Vec::new();
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
    metrics.insert("frame_index".to_owned(), serde_json::json!(frame_index));
    metrics.insert("surface_size_px".to_owned(), serde_json::json!(surface_size_px));
    metrics.insert("root".to_owned(), root_metrics(&snapshot.root));
    metrics.insert("file_type_manifest".to_owned(), snapshot.file_type_manifest);
    metrics.insert("formats".to_owned(), snapshot.formats);

    let components = component_lines("asset", &lines);
    UiSurfaceNode {
        version: 1,
        surface_id: UI_SURFACE_EDITOR_ASSET_BROWSER.to_owned(),
        source: ASSET_BROWSER_SOURCE.to_owned(),
        visible: true,
        modal: true,
        z_order: 970,
        title: "ASSET BROWSER".to_owned(),
        subtitle: "engine.assets.browser node attached through engine.ui".to_owned(),
        body_lines: lines,
        footer_lines: vec![
            "F1 closes".to_owned(),
            "ESC toggles primary UI".to_owned(),
            "source: asset_browser.snapshot_v1".to_owned(),
        ],
        style_tags: vec!["retained".to_owned()],
        theme_id: UI_THEME_NORTHSTAR_DEFAULT.to_owned(),
        component_id: UI_COMPONENT_PANEL.to_owned(),
        components,
        message: None,
        style: asset_browser_style(),
        metrics,
    }
}

fn node_error(
    frame_index: u64,
    surface_size_px: [u32; 2],
    err: String,
) -> UiSurfaceNode {
    UiSurfaceNode {
        version: 1,
        surface_id: UI_SURFACE_EDITOR_ASSET_BROWSER.to_owned(),
        source: ASSET_BROWSER_SOURCE.to_owned(),
        visible: true,
        modal: true,
        z_order: 970,
        title: "ASSET BROWSER".to_owned(),
        subtitle: "surface source is unavailable".to_owned(),
        body_lines: vec![err.clone()],
        footer_lines: vec!["F1 closes".to_owned(), "engine.ui keeps the node retained".to_owned()],
        style_tags: vec!["error".to_owned(), "retained".to_owned()],
        theme_id: UI_THEME_NORTHSTAR_DEFAULT.to_owned(),
        component_id: UI_COMPONENT_PANEL.to_owned(),
        components: vec![UiComponentNode::text("asset.error", err.clone()).tagged("error")],
        message: None,
        style: asset_browser_style(),
        metrics: BTreeMap::from([
            ("frame_index".to_owned(), serde_json::json!(frame_index)),
            ("surface_size_px".to_owned(), serde_json::json!(surface_size_px)),
            ("error".to_owned(), Value::String(err)),
        ]),
    }
}


fn asset_browser_style() -> UiSurfaceStyle {
    UiSurfaceStyle {
        theme_id: UI_THEME_NORTHSTAR_DEFAULT.to_owned(),
        anchor: UiSurfaceAnchor::TopRight,
        min_size_px: [520.0, 420.0],
        max_size_px: [820.0, 720.0],
        margin_px: [28.0, 24.0],
        row_pitch_px: 26.0,
        font: UiFontStyle {
            body_px: 18.0,
            title_px: 30.0,
            secondary_px: 15.0,
            line_height_px: 22.0,
            pixel_snap: true,
            ..UiFontStyle::default()
        },
        ..UiSurfaceStyle::default()
    }
}

fn component_lines(prefix: &str, lines: &[String]) -> Vec<UiComponentNode> {
    lines
        .iter()
        .enumerate()
        .map(|(index, line)| UiComponentNode::row(format!("{prefix}.line.{index}"), line.clone()))
        .collect()
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
