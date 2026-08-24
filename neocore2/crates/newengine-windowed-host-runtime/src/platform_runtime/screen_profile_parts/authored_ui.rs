use super::*;
use newengine_ui_api::{
    UiCompiledDocument, UiMountSurfaceRequest, UI_SERVICE_METHOD_MOUNT_SURFACE_V1,
};

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct ScreenProfileAssetsUiCompileResponse {
    ok: bool,
    document_ref: String,
    surface_id: String,
    compiled_document: UiCompiledDocument,
    warnings: Vec<String>,
}

fn authored_ui_compile_message_is_info(message: &str) -> bool {
    [
        ".neui dialect loaded ",
        ".neui theme library resolved ",
        ".neui component library resolved ",
        ".neui live root compiled ",
    ]
    .iter()
    .any(|prefix| message.starts_with(prefix))
}

impl ScreenProfileRuntimeState {
    pub(super) fn mount_authored_ui_document(
        &mut self,
        document_ref: &str,
    ) -> Result<String, String> {
        // Screen profile initialization can run before scene/content bootstrap. Mount
        // canonical runtime roots here as a synchronous prerequisite so authored HUD
        // compilation never depends on a later world tick or the process CWD.
        let assets =
            newengine_assets::AssetServiceClient::new(newengine_plugin_host::default_host_api());
        let roots =
            newengine_asset_bootstrap_runtime::collect_app_asset_roots("", "NEWENGINE_APP_ASSETS");
        newengine_asset_bootstrap_runtime::mount_asset_roots_best_effort(&assets, &roots);

        let payload = serde_json::to_vec(&serde_json::json!({
            "document_ref": document_ref,
            "source_kind": "asset",
            "mount_runtime": false
        }))
        .map_err(|e| e.to_string())?;
        let bytes = newengine_core::call_service_v1_optional(
            ENGINE_ASSETS_UI_SERVICE_ID,
            assets_ui_method::COMPILE_DOCUMENT_V1,
            &payload,
        )?
        .ok_or_else(|| {
            format!(
                "engine.assets.ui service is not registered; cannot compile '{}'",
                document_ref
            )
        })?;
        let response: ScreenProfileAssetsUiCompileResponse = serde_json::from_slice(&bytes)
            .map_err(|e| format!("engine.assets.ui returned invalid compile response: {e}"))?;
        if !response.ok {
            return Err(format!(
                "engine.assets.ui returned ok=false for '{}' surface='{}'",
                response.document_ref, response.surface_id
            ));
        }
        for diagnostic in &response.warnings {
            if authored_ui_compile_message_is_info(diagnostic) {
                newengine_ulog_api::ulog::info!(
                    "screen profile: authored game .neui compile info ref='{}' diagnostic='{}'",
                    response.document_ref,
                    diagnostic
                );
            } else {
                newengine_ulog_api::ulog::warn!(
                    "screen profile: authored game .neui compile warning ref='{}' warning='{}'",
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
        if surface_id.trim().is_empty() {
            return Err(format!(
                "compiled game .neui '{}' did not declare a surface id",
                document_ref
            ));
        }
        let request = UiMountSurfaceRequest {
            surface_id: surface_id.clone(),
            document: response.compiled_document,
            visible: true,
        };
        let payload = serde_json::to_vec(&request)
            .map_err(|e| format!("failed to encode ui.mount_surface_v1 request: {e}"))?;
        newengine_core::call_service_v1_optional(
            newengine_ui_api::ENGINE_UI_SERVICE_ID,
            UI_SERVICE_METHOD_MOUNT_SURFACE_V1,
            &payload,
        )?
        .ok_or_else(|| {
            format!(
                "engine.ui service is not registered; cannot mount authored game UI '{}'",
                document_ref
            )
        })?;
        newengine_ulog_api::ulog::info!(
            "screen profile: authored game .neui mounted ref='{}' surface='{}' policy='no generated gameplay HUD fallback'",
            document_ref,
            surface_id
        );
        Ok(surface_id)
    }
}

#[cfg(test)]
mod authored_ui_diagnostic_tests {
    use super::authored_ui_compile_message_is_info;

    #[test]
    fn successful_compiler_diagnostics_are_info() {
        for message in [
            ".neui dialect loaded ref='ui/dialects/runtime.neui@dialect'",
            ".neui theme library resolved ref='ui/themes/default.neui@theme'",
            ".neui component library resolved ref='ui/components/common.neui@library'",
            ".neui live root compiled source='ui/engine/main_menu.neui@surface'",
        ] {
            assert!(authored_ui_compile_message_is_info(message), "{message}");
        }
    }

    #[test]
    fn degraded_compiler_diagnostics_remain_warnings() {
        for message in [
            ".neui dialect fallback ref='ui/dialects/runtime.neui@dialect'",
            ".neui theme library unresolved ref='ui/themes/missing.neui@theme'",
            ".neui theme library contains no Theme entry ref='ui/themes/empty.neui@theme'",
            ".neui component library unresolved ref='ui/components/missing.neui@library'",
        ] {
            assert!(!authored_ui_compile_message_is_info(message), "{message}");
        }
    }
}
