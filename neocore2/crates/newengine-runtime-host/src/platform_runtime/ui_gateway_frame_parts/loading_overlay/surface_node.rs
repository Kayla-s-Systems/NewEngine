use super::*;
use super::components::{error_overlay_components, loading_overlay_components};

pub(super) fn build_overlay_surface_node(
    status: &ScreenOverlayStatus,
    provider: UiProviderBinding,
    frame_index: u64,
) -> UiSurfaceNode {
    let projection = loading_surface_projection(status, provider);
    let spec = OverlaySurfaceSpec::from_status(status, projection.surface_id());
    let progress = OverlayProgress::from_status(status);
    let body_lines = spec.body_lines(status, progress.percent);
    let footer_lines = spec.footer_lines();
    let style_tags = spec.style_tags();
    let components = spec.components(status, progress, frame_index);
    let style = spec.style();
    let title = if spec.is_error() { status.title.clone() } else { String::new() };
    let subtitle = if spec.is_error() { status.status.clone() } else { String::new() };
    let mut metrics = overlay_metrics(status, &projection, frame_index);
    metrics.insert("surface_id".to_owned(), serde_json::json!(spec.surface_id.as_str()));

    UiSurfaceNode {
        version: 1,
        surface_id: spec.surface_id,
        source: spec.source,
        visible: true,
        modal: spec.modal,
        z_order: spec.z_order,
        title,
        subtitle,
        body_lines,
        footer_lines,
        style_tags,
        theme_id: UI_THEME_NORTHSTAR_DEFAULT.to_owned(),
        style_ref: None,
        component_id: UI_COMPONENT_PANEL.to_owned(),
        components,
        message: None,
        style,
        admission_policy: Default::default(),
        metrics,
    }
}

pub(super) fn hidden_loading_overlay_node(frame_index: u64) -> UiSurfaceNode {
    let mut metrics = BTreeMap::new();
    metrics.insert("frame_index".to_owned(), serde_json::json!(frame_index));
    metrics.insert("reason".to_owned(), serde_json::json!("scene-launch-complete"));

    UiSurfaceNode {
        version: 1,
        surface_id: UI_SURFACE_ENGINE_LOADING.to_owned(),
        source: "engine.ui.loading".to_owned(),
        visible: false,
        modal: false,
        z_order: 900,
        title: String::new(),
        subtitle: String::new(),
        body_lines: Vec::new(),
        footer_lines: Vec::new(),
        style_tags: vec!["retained".to_owned(), "hidden".to_owned()],
        theme_id: UI_THEME_NORTHSTAR_DEFAULT.to_owned(),
        style_ref: None,
        component_id: UI_COMPONENT_PANEL.to_owned(),
        components: Vec::new(),
        message: None,
        style: UiSurfaceStyle::default(),
        admission_policy: Default::default(),
        metrics,
    }
}

#[derive(Clone, Copy)]
struct OverlayProgress {
    value: f32,
    percent: f32,
}

impl OverlayProgress {
    fn from_status(status: &ScreenOverlayStatus) -> Self {
        Self {
            value: status.progress_01().clamp(0.0, 0.995),
            percent: (status.progress_01() * 100.0).clamp(0.0, 99.0),
        }
    }
}

struct OverlaySurfaceSpec {
    surface_id: String,
    source: String,
    modal: bool,
    z_order: i32,
}

impl OverlaySurfaceSpec {
    fn from_status(status: &ScreenOverlayStatus, projected_surface_id: &str) -> Self {
        let is_error = status.kind == ScreenOverlayStatusKind::Error;
        let fallback_surface = if is_error {
            UI_SURFACE_ENGINE_ERROR_MODAL
        } else {
            UI_SURFACE_ENGINE_LOADING
        };

        Self {
            surface_id: if projected_surface_id.trim().is_empty() {
                fallback_surface.to_owned()
            } else {
                projected_surface_id.to_owned()
            },
            source: if is_error { "engine.ui.error" } else { "engine.ui.loading" }.to_owned(),
            modal: is_error,
            z_order: if is_error { 1000 } else { 900 },
        }
    }

    fn is_error(&self) -> bool {
        self.modal
    }

    fn body_lines(&self, status: &ScreenOverlayStatus, _progress_percent: f32) -> Vec<String> {
        if self.is_error() {
            return vec![
                status.title.clone(),
                status.status.clone(),
                status.detail.clone(),
                "NorthStar diagnostics captured this failure; the window remains alive for inspection.".to_owned(),
            ];
        }

        Vec::new()
    }

    fn footer_lines(&self) -> Vec<String> {
        if self.is_error() {
            vec!["Press close/quit from the host shell after copying diagnostics.".to_owned()]
        } else {
            Vec::new()
        }
    }

    fn style_tags(&self) -> Vec<String> {
        if self.is_error() {
            return vec![
                "retained".to_owned(),
                "centered-error".to_owned(),
                "northstar-error-modal".to_owned(),
                "diagnostic-card".to_owned(),
            ];
        }

        vec!["retained".to_owned(), "startup-loading".to_owned(), "bg-sprite-progress".to_owned()]
    }

    fn components(&self, status: &ScreenOverlayStatus, progress: OverlayProgress, frame_index: u64) -> Vec<UiComponentNode> {
        if self.is_error() {
            error_overlay_components(status)
        } else {
            loading_overlay_components(
                progress.value,
                progress.percent,
                frame_index,
            )
        }
    }

    fn style(&self) -> UiSurfaceStyle {
        if self.is_error() {
            return UiSurfaceStyle {
                anchor: UiSurfaceAnchor::Center,
                min_size_px: [700.0, 360.0],
                max_size_px: [980.0, 620.0],
                row_pitch_px: 30.0,
                ..UiSurfaceStyle::default()
            };
        }

        UiSurfaceStyle {
            anchor: UiSurfaceAnchor::TopLeft,
            min_size_px: [0.0, 0.0],
            max_size_px: [100_000.0, 100_000.0],
            row_pitch_px: 0.0,
            ..UiSurfaceStyle::default()
        }
    }
}

fn overlay_metrics(
    status: &ScreenOverlayStatus,
    projection: &impl serde::Serialize,
    frame_index: u64,
) -> BTreeMap<String, serde_json::Value> {
    let mut metrics = BTreeMap::new();
    metrics.insert(
        "surface_projection".to_owned(),
        serde_json::to_value(projection).unwrap_or(serde_json::Value::Null),
    );
    metrics.insert("frame_index".to_owned(), serde_json::json!(frame_index));
    metrics.insert("overlay_kind".to_owned(), serde_json::json!(format!("{:?}", status.kind)));
    metrics.insert("overlay_reason".to_owned(), serde_json::json!(format!("{:?}", status.reason)));
    metrics
}
