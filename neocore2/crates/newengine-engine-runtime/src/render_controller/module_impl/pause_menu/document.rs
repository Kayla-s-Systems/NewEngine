#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_assets_api::{assets_ui_method, ENGINE_ASSETS_UI_SERVICE_ID};
use newengine_ui_api::{UiCompiledDocument, UiMountSurfaceRequest, UI_SERVICE_METHOD_MOUNT_SURFACE_V1};
use newengine_ui_menu_runtime::MenuRuntime;
use newengine_ui_navigation_api::{MenuDocument, ENGINE_PAUSE_MENU_SURFACE_REF};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(default)]
struct AssetsUiCompileResponse {
    ok: bool,
    document_ref: String,
    surface_id: String,
    compiled_document: UiCompiledDocument,
    menu_document: Option<MenuDocument>,
    warnings: Vec<String>,
}

impl Default for AssetsUiCompileResponse {
    fn default() -> Self {
        Self {
            ok: false,
            document_ref: String::new(),
            surface_id: String::new(),
            compiled_document: UiCompiledDocument::default(),
            menu_document: None,
            warnings: Vec::new(),
        }
    }
}

/// Load the pause menu through the canonical UI asset pipeline.
///
/// Boundary rule:
///
/// ```text
/// engine.pause_menu -> engine.assets.ui::compile_document_v1(ref) -> response DTO
/// engine.pause_menu -> engine.ui::mount_surface_v1(compiled DTO)       -> best-effort live mount
/// ```
///
/// The pause runtime does not parse `.neui`, NEF8, deflate, XMLcentral, VFS paths
/// or old JSON menu assets. It only performs request/response calls.
pub(super) fn try_load_pause_menu_document() -> Result<MenuRuntime, String> {
    let response = compile_pause_surface()?;
    if !response.ok {
        return Err(format!(
            "engine.assets.ui returned ok=false for '{}' surface='{}'",
            response.document_ref,
            response.surface_id
        ));
    }
    for warning in &response.warnings {
        log::warn!("engine.pause_menu: .neui compile warning ref='{}' warning='{}'", response.document_ref, warning);
    }

    mount_pause_surface_best_effort(&response.compiled_document);

    let document = response.menu_document.ok_or_else(|| {
        format!(
            "engine.assets.ui compiled '{}' but response did not include a MenuDocument DTO",
            response.document_ref
        )
    })?;
    MenuRuntime::new(document)
}

fn compile_pause_surface() -> Result<AssetsUiCompileResponse, String> {
    let payload = serde_json::to_vec(&serde_json::json!({
        "document_ref": ENGINE_PAUSE_MENU_SURFACE_REF,
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
                ENGINE_PAUSE_MENU_SURFACE_REF
            ));
        }
    };

    serde_json::from_slice::<AssetsUiCompileResponse>(&bytes).map_err(|e| {
        format!(
            "engine.assets.ui returned non-compile response for '{}': {}",
            ENGINE_PAUSE_MENU_SURFACE_REF, e
        )
    })
}

fn mount_pause_surface_best_effort(compiled_document: &UiCompiledDocument) {
    if compiled_document.surface_id.trim().is_empty() {
        return;
    }
    let service_supports_mount = newengine_core::describe_service(newengine_ui_api::ENGINE_UI_SERVICE_ID)
        .map(|description| description.contains(UI_SERVICE_METHOD_MOUNT_SURFACE_V1))
        .unwrap_or(false);
    if !service_supports_mount {
        return;
    }

    let request = UiMountSurfaceRequest {
        surface_id: compiled_document.surface_id.clone(),
        document: compiled_document.clone(),
        visible: true,
    };
    let payload = match serde_json::to_vec(&request) {
        Ok(payload) => payload,
        Err(e) => {
            log::warn!("engine.pause_menu: failed to encode ui.mount_surface_v1 request: {e}");
            return;
        }
    };
    match newengine_core::call_service_v1_optional(
        newengine_ui_api::ENGINE_UI_SERVICE_ID,
        UI_SERVICE_METHOD_MOUNT_SURFACE_V1,
        &payload,
    ) {
        Ok(Some(_)) => {}
        Ok(None) => {
            log::warn!(
                "engine.pause_menu: engine.ui service is not registered; compiled surface '{}' remains available as DTO only",
                compiled_document.surface_id
            );
        }
        Err(e) => {
            log::warn!(
                "engine.pause_menu: ui.mount_surface_v1 failed surface='{}' err='{}'",
                compiled_document.surface_id,
                e
            );
        }
    }
}
