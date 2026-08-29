use super::*;

impl AssetsCatalogUiRuntimeModule {
    pub(crate) fn publish_selected_asset_context(&self, resources: &mut Resources) {
        let Some(snapshot) = self.cached_snapshot.as_ref() else {
            return;
        };
        let Some(entry) = snapshot
            .entries
            .get(self.selected_index)
            .or_else(|| snapshot.entries.iter().find(|entry| !entry.is_directory()))
        else {
            return;
        };
        if entry.is_directory() {
            return;
        }
        let selection = EditorSelectionContext::asset(
            entry.logical_path.clone(),
            entry.name.clone(),
            ASSETS_CATALOG_SURFACE_ID,
            entry.semantic_gateway.clone(),
        );
        resources.insert(selection);
    }

    pub(crate) fn document_actions_for_snapshot(
        &mut self,
        snapshot: &AssetsCatalogSnapshot,
    ) -> Vec<AssetDocumentAction> {
        let Some(entry) = snapshot
            .entries
            .get(self.selected_index)
            .or_else(|| snapshot.entries.iter().find(|entry| !entry.is_directory()))
        else {
            return Vec::new();
        };
        if entry.is_directory() {
            return Vec::new();
        }

        if self.cached_document_action_ref == entry.logical_path {
            return self.cached_document_actions.clone();
        }

        self.cached_document_action_ref = entry.logical_path.clone();
        match self.state.client.inspect_document_json_v1(
            newengine_assets_api::AssetDocumentRequest {
                asset_ref: entry.logical_path.clone(),
                requester: ASSETS_CATALOG_UI_OWNER.to_owned(),
                ..newengine_assets_api::AssetDocumentRequest::default()
            },
        ) {
            Ok(document) => {
                self.cached_document_action_error = None;
                self.cached_document_actions = document.actions;
                self.cached_document_actions.clone()
            }
            Err(error) => {
                let should_log =
                    self.cached_document_action_error.as_deref() != Some(error.as_str());
                if should_log {
                    newengine_ulog_api::ulog::warn!(
                        "asset browser UI: asset document actions unavailable path='{}' err='{}'",
                        entry.logical_path,
                        error,
                    );
                }
                self.cached_document_action_error = Some(error);
                self.cached_document_actions.clear();
                Vec::new()
            }
        }
    }

    pub(crate) fn dispatch_asset_document_action(
        &mut self,
        action_id: &str,
        frame_index: u64,
        surface_size_px: [u32; 2],
    ) {
        let Some(action) = self
            .cached_document_actions
            .iter()
            .find(|action| action.id == action_id)
            .cloned()
        else {
            newengine_ulog_api::ulog::warn!(
                "asset browser UI: unknown document action id='{}'",
                action_id
            );
            return;
        };

        if !action.enabled {
            self.last_action_result = Some(AssetPatchResult {
                accepted: false,
                written: false,
                dirty: false,
                diagnostics: vec![newengine_assets_api::AssetDocumentDiagnostic::warn(
                    "asset.action.disabled",
                    if action.disabled_reason.trim().is_empty() {
                        format!("action '{}' is disabled by provider policy", action.label)
                    } else {
                        action.disabled_reason.clone()
                    },
                )],
                ..AssetPatchResult::default()
            });
            self.invalidate_node();
            return;
        }

        if action.requires_input {
            self.last_action_result = Some(AssetPatchResult {
                accepted: false,
                written: false,
                dirty: false,
                diagnostics: vec![newengine_assets_api::AssetDocumentDiagnostic::info(
                    "asset.action.requires_input",
                    format!(
                        "action '{}' needs a schema/dialog payload before emitting AssetPatch",
                        action.label
                    ),
                )],
                ..AssetPatchResult::default()
            });
            self.invalidate_node();
            return;
        }

        let Some(patch) = action.patch_template.clone() else {
            self.last_action_result = Some(AssetPatchResult {
                accepted: false,
                written: false,
                dirty: false,
                diagnostics: vec![newengine_assets_api::AssetDocumentDiagnostic::warn(
                    "asset.action.no_patch",
                    format!(
                        "action '{}' has no provider-declared AssetPatch template",
                        action.label
                    ),
                )],
                ..AssetPatchResult::default()
            });
            self.invalidate_node();
            return;
        };

        match self.state.client.apply_patch_json_v1(patch) {
            Ok(result) => {
                newengine_ulog_api::ulog::info!(
                    "asset browser UI: document action dispatched id='{}' accepted={} written={}",
                    action.id,
                    result.accepted,
                    result.written,
                );
                self.last_action_result = Some(result);
                self.cached_snapshot = None;
                self.invalidate_node();
                self.refresh_cache(frame_index, surface_size_px);
            }
            Err(error) => {
                self.last_action_result = Some(AssetPatchResult {
                    accepted: false,
                    written: false,
                    dirty: true,
                    diagnostics: vec![newengine_assets_api::AssetDocumentDiagnostic::error(
                        "asset.action.dispatch_failed",
                        error,
                    )],
                    ..AssetPatchResult::default()
                });
                self.invalidate_node();
            }
        }
    }
}
