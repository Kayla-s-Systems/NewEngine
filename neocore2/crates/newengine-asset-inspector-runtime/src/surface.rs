use newengine_assets_api::{assets_ui_method, ENGINE_ASSETS_UI_SERVICE_ID};
use newengine_assets_ui_runtime::{AssetsUiCompileRequest, AssetsUiCompileResponse};
use newengine_ui_api::{
    UiDocumentSourceKind, UiMountSurfaceRequest, ENGINE_UI_SERVICE_ID,
    UI_SERVICE_METHOD_MOUNT_SURFACE_V1,
};

use crate::ASSET_INSPECTOR_SURFACE_ID;

pub(crate) const ASSET_INSPECTOR_DOCUMENT_REF: &str = "ui/tools/asset_inspector.neui@surface";

pub(crate) fn mount_asset_inspector_surface() -> Result<String, String> {
    let request = AssetsUiCompileRequest {
        document_ref: ASSET_INSPECTOR_DOCUMENT_REF.to_owned(),
        source_kind: UiDocumentSourceKind::Asset,
        mount_runtime: false,
        ..AssetsUiCompileRequest::default()
    };
    let payload = serde_json::to_vec(&request).map_err(|error| error.to_string())?;
    let bytes = newengine_core::call_service_v1_optional(
        ENGINE_ASSETS_UI_SERVICE_ID,
        assets_ui_method::COMPILE_DOCUMENT_V1,
        &payload,
    )?
    .ok_or_else(|| "engine.assets.ui route is unavailable".to_owned())?;
    let response: AssetsUiCompileResponse = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid engine.assets.ui compile response: {error}"))?;
    if !response.ok {
        return Err(format!(
            "engine.assets.ui returned ok=false document='{}'",
            response.document_ref
        ));
    }
    for diagnostic in &response.warnings {
        let informational = diagnostic.contains("resolved ref=")
            || diagnostic.contains("dialect loaded ref=")
            || diagnostic.contains("live root compiled source=");
        if informational {
            newengine_ulog_api::ulog::info!(
                "asset inspector: authored UI compile detail document='{}' detail='{}'",
                response.document_ref,
                diagnostic
            );
        } else {
            newengine_ulog_api::ulog::warn!(
                "asset inspector: authored UI compile warning document='{}' warning='{}'",
                response.document_ref,
                diagnostic
            );
        }
    }
    let surface_id = if response.compiled_document.surface_id.trim().is_empty() {
        response.surface_id.clone()
    } else {
        response.compiled_document.surface_id.clone()
    };
    if surface_id != ASSET_INSPECTOR_SURFACE_ID {
        return Err(format!(
            "asset inspector document declared unexpected surface '{}' expected='{}'",
            surface_id, ASSET_INSPECTOR_SURFACE_ID
        ));
    }
    let mount = UiMountSurfaceRequest {
        surface_id: surface_id.clone(),
        document: response.compiled_document,
        visible: true,
    };
    let payload = serde_json::to_vec(&mount)
        .map_err(|error| format!("ui.mount_surface_v1 encode failed: {error}"))?;
    newengine_core::call_service_v1_optional(
        ENGINE_UI_SERVICE_ID,
        UI_SERVICE_METHOD_MOUNT_SURFACE_V1,
        &payload,
    )?
    .ok_or_else(|| "engine.ui route is unavailable".to_owned())?;
    newengine_ulog_api::ulog::info!(
        "asset inspector: standalone authored surface mounted document='{}' surface='{}'",
        ASSET_INSPECTOR_DOCUMENT_REF,
        surface_id
    );
    Ok(surface_id)
}
