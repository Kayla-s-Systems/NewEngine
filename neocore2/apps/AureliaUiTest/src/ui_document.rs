use newengine_assets_api::{assets_ui_method, ENGINE_ASSETS_UI_SERVICE_ID};
use newengine_ui_api::{
    UiCompiledDocument, UiNodeRequest, UiNodeRequestSourceKind, UiNodeTreeRequest,
    UiRuntimeNodeKind, UiSurfaceAdmissionPolicy, UiSurfaceAnchor, UiSurfaceStyle,
    UI_FONT_ASSET_BRAND, UI_FONT_ASSET_EDITOR_DISPLAY, UI_FONT_ASSET_EDITOR_SANS,
    UI_THEME_NORTHSTAR_DEFAULT,
};
use serde::Deserialize;

use crate::options::SURFACE_ID;

const SHOWCASE_DOCUMENT_REF: &str = "assets/ui/showcase.neui@surface";

#[derive(Clone, Debug)]
pub struct AureliaUiTestRouteStatus {
    pub route_available: bool,
    pub ui_backend_capability: bool,
    pub binary_frame_required: bool,
    pub assets_ui_available: bool,
}

impl AureliaUiTestRouteStatus {
    #[inline]
    pub fn new(
        route_available: bool,
        ui_backend_capability: bool,
        binary_frame_required: bool,
        assets_ui_available: bool,
    ) -> Self {
        Self {
            route_available,
            ui_backend_capability,
            binary_frame_required,
            assets_ui_available,
        }
    }

    #[inline]
    pub fn frame_mode_text(&self) -> &'static str {
        if self.binary_frame_required {
            "binary strict"
        } else {
            "binary + fallback"
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
struct AssetsUiCompileResponse {
    ok: bool,
    document_ref: String,
    surface_id: String,
    compiled_document: UiCompiledDocument,
    warnings: Vec<String>,
}

impl Default for AssetsUiCompileResponse {
    fn default() -> Self {
        Self {
            ok: false,
            document_ref: String::new(),
            surface_id: String::new(),
            compiled_document: UiCompiledDocument::default(),
            warnings: Vec::new(),
        }
    }
}

pub fn build_aurelia_ui_test_request(
    frame_index: u64,
    click_count: u64,
    route_status: AureliaUiTestRouteStatus,
) -> UiNodeTreeRequest {
    let mut request = match compile_showcase_document_to_request() {
        Ok(request) => request,
        Err(err) => fallback_error_ui(err),
    };
    patch_runtime_values(&mut request.root, frame_index, click_count, &route_status);
    request.request_id = format!("aurelia-ui-showcase.frame.{frame_index}");
    request.diagnostics.push(format!(
        ".neui authored showcase mounted document_ref={} assets.ui={} route={} ui.backend={} frame_mode={}",
        SHOWCASE_DOCUMENT_REF,
        route_status.assets_ui_available,
        route_status.route_available,
        route_status.ui_backend_capability,
        route_status.frame_mode_text(),
    ));
    request.diagnostics.push(format!(
        "font.requested={} font.stack=[{},{},Inter,Segoe UI] fallback_expected=false",
        UI_FONT_ASSET_EDITOR_SANS, UI_FONT_ASSET_EDITOR_DISPLAY, UI_FONT_ASSET_BRAND,
    ));
    request
}

fn compile_showcase_document_to_request() -> Result<UiNodeTreeRequest, String> {
    let response = compile_showcase_document()?;
    if !response.ok {
        return Err(format!(
            "engine.assets.ui returned ok=false for '{}' surface='{}'",
            response.document_ref, response.surface_id
        ));
    }
    let mut root = response.compiled_document.root.clone().ok_or_else(|| {
        format!(
            "engine.assets.ui compiled '{}' without root node",
            response.document_ref
        )
    })?;
    if root.id.trim().is_empty() {
        root.id = SURFACE_ID.to_owned();
    }
    let mut diagnostics = vec![format!(
        "Aurelia UI Showcase authored by packed {}",
        SHOWCASE_DOCUMENT_REF
    )];
    diagnostics.extend(
        response
            .warnings
            .into_iter()
            .map(|warning| format!("engine.assets.ui warning: {warning}")),
    );
    let theme_id = response
        .compiled_document
        .theme_ref
        .clone()
        .unwrap_or_else(|| UI_THEME_NORTHSTAR_DEFAULT.to_owned());
    Ok(UiNodeTreeRequest {
        request_id: "aurelia-ui-showcase.frame.0".to_owned(),
        surface_id: SURFACE_ID.to_owned(),
        source: SHOWCASE_DOCUMENT_REF.to_owned(),
        source_kind: UiNodeRequestSourceKind::AuthoredAsset,
        theme_id,
        visible: true,
        modal: false,
        z_order: 100,
        root,
        surface_style: Some(showcase_surface_style()),
        admission_policy: Some(UiSurfaceAdmissionPolicy::AcceptAll),
        diagnostics,
        ..UiNodeTreeRequest::default()
    })
}

fn compile_showcase_document() -> Result<AssetsUiCompileResponse, String> {
    let payload = serde_json::to_vec(&serde_json::json!({
        "document_ref": SHOWCASE_DOCUMENT_REF,
        "source_kind": "asset",
        "mount_runtime": false
    }))
    .map_err(|e| e.to_string())?;

    let bytes = match newengine_core::call_service_v1_optional(
        ENGINE_ASSETS_UI_SERVICE_ID,
        assets_ui_method::COMPILE_DOCUMENT_V1,
        &payload,
    )? {
        Some(bytes) => bytes,
        None => {
            return Err(format!(
                "engine.assets.ui service is not registered; cannot compile '{}'",
                SHOWCASE_DOCUMENT_REF
            ));
        }
    };

    serde_json::from_slice::<AssetsUiCompileResponse>(&bytes).map_err(|e| {
        format!(
            "engine.assets.ui returned non-compile response for '{}': {}",
            SHOWCASE_DOCUMENT_REF, e
        )
    })
}

fn patch_runtime_values(
    root: &mut UiNodeRequest,
    frame_index: u64,
    click_count: u64,
    route_status: &AureliaUiTestRouteStatus,
) {
    patch_text(
        root,
        "status.route.value",
        if route_status.route_available {
            "engine.ui active"
        } else {
            "waiting"
        },
    );
    patch_text(
        root,
        "status.capability.value",
        if route_status.ui_backend_capability {
            "ui.backend yes"
        } else {
            "missing"
        },
    );
    patch_text(
        root,
        "status.transport.value",
        route_status.frame_mode_text(),
    );
    patch_text(root, "status.frame.value", &frame_index.to_string());
    patch_text(root, "status.clicks.value", &click_count.to_string());
    patch_text(
        root,
        "diag.4",
        &format!("✓ frame transport: {}", route_status.frame_mode_text()),
    );
    let pulse = ((frame_index % 180) as f32) / 179.0;
    patch_value(root, "showcase.slider", &format!("{pulse:.2}"));
    patch_value(root, "showcase.progress", &format!("{:.0}%", pulse * 100.0));
}

fn patch_text(node: &mut UiNodeRequest, id: &str, text: &str) {
    if node.id == id {
        node.text = text.to_owned();
        return;
    }
    for child in &mut node.children {
        patch_text(child, id, text);
    }
}

fn patch_value(node: &mut UiNodeRequest, id: &str, value: &str) {
    if node.id == id {
        node.value = Some(value.to_owned());
        node.props
            .insert("value".to_owned(), serde_json::json!(value));
        return;
    }
    for child in &mut node.children {
        patch_value(child, id, value);
    }
}

fn showcase_surface_style() -> UiSurfaceStyle {
    let mut style = UiSurfaceStyle::default();
    style.theme_id = UI_THEME_NORTHSTAR_DEFAULT.to_owned();
    style.anchor = UiSurfaceAnchor::TopLeft;
    style.min_size_px = [1280.0, 720.0];
    style.max_size_px = [1280.0, 720.0];
    style.margin_px = [0.0, 0.0];
    style.padding_px = [24.0, 48.0, 24.0, 24.0];
    style.row_pitch_px = 32.0;
    style.panel_rgba = [248, 250, 253, 255];
    style.panel_header_rgba = [255, 255, 255, 255];
    style.accent_rgba = [0, 113, 206, 255];
    style.text_rgba = [23, 32, 54, 255];
    style.text_muted_rgba = [91, 104, 126, 255];
    style.border_rgba = [218, 225, 235, 255];
    style.backdrop_rgba = [255, 255, 255, 255];
    style.corner_radius_px = 8.0;
    style.border_px = 1.0;
    style.shadow_alpha = 0;
    style.row_even_alpha = 0;
    style.row_odd_alpha = 0;
    style.font.stack = vec![
        UI_FONT_ASSET_EDITOR_SANS.to_owned(),
        UI_FONT_ASSET_EDITOR_DISPLAY.to_owned(),
        UI_FONT_ASSET_BRAND.to_owned(),
        "Segoe UI".to_owned(),
    ];
    style.font.title_px = 30.0;
    style.font.body_px = 13.0;
    style.font.secondary_px = 11.5;
    style.font.line_height_px = 17.0;
    style.font.pixel_snap = false;
    style.normalized()
}

fn fallback_error_ui(error: String) -> UiNodeTreeRequest {
    UiNodeTreeRequest {
        request_id: "aurelia-ui-showcase.error".to_owned(),
        surface_id: SURFACE_ID.to_owned(),
        source: "apps.AureliaUiTest.error".to_owned(),
        source_kind: UiNodeRequestSourceKind::Generated,
        theme_id: UI_THEME_NORTHSTAR_DEFAULT.to_owned(),
        visible: true,
        z_order: 100,
        root: UiNodeRequest::new("showcase.error", UiRuntimeNodeKind::Surface).with_text(error),
        surface_style: Some(showcase_surface_style()),
        admission_policy: Some(UiSurfaceAdmissionPolicy::AcceptAll),
        diagnostics: vec!["failed to compile Showcase through engine.assets.ui".to_owned()],
        ..UiNodeTreeRequest::default()
    }
}
