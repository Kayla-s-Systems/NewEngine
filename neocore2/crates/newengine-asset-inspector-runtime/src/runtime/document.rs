use super::presentation::document_exposes_entries;
use super::*;

impl AssetInspectorRuntimeModule {
    pub(super) fn sync_text_editor_from_document(&mut self, document: &AssetDocument) {
        self.text_editor = TextEditorState::from_document(document);
        self.refresh_syntax_preview();
        if let Some(editor) = self.text_editor.as_ref() {
            newengine_ulog_api::ulog::info!(
                "asset inspector: text editor opened ref='{}' language='{}' lines={} editable={} syntax_preview=true",
                editor.asset_ref,
                editor.language,
                editor.lines.len(),
                editor.editable
            );
        }
    }
    pub(super) fn refresh_syntax_preview(&mut self) {
        let Some(editor) = self.text_editor.as_ref() else {
            self.syntax_preview = None;
            self.syntax_editor = None;
            return;
        };
        let start_line = editor.page.saturating_mul(TEXT_ROWS);
        self.syntax_preview = Some(highlight_preview_page(
            &editor.lines,
            &editor.language,
            start_line,
        ));
        self.syntax_editor = Some(highlight_editor_page(
            &editor.lines,
            &editor.language,
            start_line,
        ));
    }
    pub(super) fn inspect_cached(
        &mut self,
        asset_ref: &str,
    ) -> Result<(AssetDocument, bool), String> {
        if let Some(document) = self.document_cache.get(asset_ref) {
            return Ok((document, true));
        }
        let document = self.facade.inspect(asset_ref)?;
        self.document_cache.insert(&document);
        Ok((document, false))
    }
    pub(super) fn install_document(
        &mut self,
        document: &AssetDocument,
        container_available_hint: bool,
    ) -> AssetPreviewSnapshot {
        self.document_cache.insert(document);
        self.selected_container_available =
            container_available_hint || document_exposes_entries(document);
        self.sync_text_editor_from_document(document);
        let preview = self
            .preview_api
            .request(document, PREVIEW_WIDTH, PREVIEW_HEIGHT);
        self.preview_snapshot = Some(preview.clone());
        self.document = Some(document.clone());
        self.current_preview_cache_valid = true;
        preview
    }

    pub(super) fn reinspect_asset(
        &mut self,
        asset_ref: &str,
        container_available_hint: bool,
    ) -> Result<AssetPreviewSnapshot, String> {
        self.document_cache.invalidate(asset_ref);
        self.preview_api.invalidate(asset_ref);
        let document = self.facade.inspect(asset_ref)?;
        Ok(self.install_document(&document, container_available_hint))
    }

    pub(super) fn edit_text_line(&mut self, row: usize, payload: &serde_json::Value) {
        let mut changed = false;
        {
            let Some(editor) = self.text_editor.as_mut() else {
                return;
            };
            if !editor.editable {
                self.status = "Text asset is read-only".to_owned();
                return;
            }
            let absolute = editor.page * TEXT_ROWS + row;
            let Some(line) = editor.lines.get_mut(absolute) else {
                return;
            };
            let value = payload
                .get("value")
                .and_then(serde_json::Value::as_str)
                .or_else(|| payload.as_str())
                .unwrap_or_default();
            if line != value {
                *line = value.to_owned();
                editor.dirty = editor.compose() != editor.original_text;
                self.status = format!(
                    "Editing {} | line {} | {}",
                    editor.asset_ref,
                    absolute + 1,
                    if editor.dirty { "modified" } else { "clean" }
                );
                changed = true;
            }
        }
        if changed {
            self.refresh_syntax_preview();
        }
    }
    pub(super) fn text_previous_page(&mut self) {
        if let Some(editor) = self.text_editor.as_mut() {
            editor.page = editor.page.saturating_sub(1);
        } else {
            return;
        }
        self.refresh_syntax_preview();
    }
    pub(super) fn text_next_page(&mut self) {
        if let Some(editor) = self.text_editor.as_mut() {
            editor.page = (editor.page + 1).min(editor.total_pages().saturating_sub(1));
        } else {
            return;
        }
        self.refresh_syntax_preview();
    }
    pub(super) fn discard_text_changes(&mut self) {
        let asset_ref = if let Some(editor) = self.text_editor.as_mut() {
            editor.reset();
            editor.asset_ref.clone()
        } else {
            return;
        };
        self.refresh_syntax_preview();
        self.status = format!("Discarded text changes for {asset_ref}");
    }
    pub(super) fn save_text_document(&mut self, frame_index: u64) {
        let Some(editor) = self.text_editor.as_ref() else {
            return;
        };
        if !editor.editable {
            self.status = "Text asset is read-only".to_owned();
            return;
        }
        if !editor.dirty {
            self.status = "Text document has no changes".to_owned();
            return;
        }
        let asset_ref = editor.asset_ref.clone();
        let content = editor.compose();
        self.begin_activity("SAVING", frame_index);
        match self.facade.write_text(&asset_ref, content) {
            Ok(response) if response.ok && response.written => {
                self.status = format!(
                    "Saved {} | {} bytes",
                    response.logical_path, response.bytes_written
                );
                let container_available_hint = self.selected_container_available;
                if let Err(error) = self.reinspect_asset(&asset_ref, container_available_hint) {
                    self.current_preview_cache_valid = false;
                    self.status = format!("Text saved, but re-inspection failed: {error}");
                }
                self.complete_activity(frame_index);
            }
            Ok(response) => {
                self.status = response
                    .diagnostics
                    .last()
                    .cloned()
                    .unwrap_or_else(|| "Text writer rejected the update".to_owned());
                self.complete_activity(frame_index);
            }
            Err(error) => {
                self.status = format!("Text write failed: {error}");
                self.complete_activity(frame_index);
            }
        }
    }
    pub(super) fn edit_field(&mut self, row: usize, payload: &serde_json::Value, frame_index: u64) {
        let Some(document) = self.document.clone() else {
            self.status = "No AssetDocument selected".to_owned();
            return;
        };
        let Some(field) = document_field(&document, row).cloned() else {
            self.status = format!("AssetDocument field row {row} is unavailable");
            return;
        };
        self.begin_activity("APPLYING", frame_index);
        let result = self.facade.apply_field_edit(&document, &field, payload);
        self.status = patch_status("Field edit", &result);
        let reload_required = result.written || result.dirty;
        self.finalize_patch_application(result, reload_required, false, frame_index);
    }
    pub(super) fn dispatch_document_action(&mut self, row: usize, frame_index: u64) {
        let Some(document) = self.document.clone() else {
            self.status = "No AssetDocument selected".to_owned();
            return;
        };
        let Some(action) = available_document_action(&document, row).cloned() else {
            self.status = format!("Provider action row {row} is unavailable");
            return;
        };
        self.begin_activity("APPLYING", frame_index);
        let result = self.facade.dispatch_document_action(&document, &action);
        self.status = patch_status(&action.label, &result);
        let reload_required =
            result.written || result.dirty || action.method.ends_with("discard_staged_json_v1");
        self.finalize_patch_application(result, reload_required, true, frame_index);
    }
    pub(super) fn reload_document(&mut self, frame_index: u64) {
        let Some(asset_ref) = self
            .document
            .as_ref()
            .map(|document| document.asset_ref.clone())
        else {
            return;
        };
        let container_available_hint = self.selected_container_available;
        if let Err(error) = self.reinspect_asset(&asset_ref, container_available_hint) {
            self.current_preview_cache_valid = false;
            self.status = format!("Asset changed, but re-inspection failed: {error}");
        }
        self.last_refresh_frame = Some(frame_index);
    }

    fn finalize_patch_application(
        &mut self,
        result: AssetPatchResult,
        reload_required: bool,
        invalidate_listing_when_outside: bool,
        frame_index: u64,
    ) {
        self.last_patch_result = Some(result);
        if !reload_required {
            self.complete_activity(frame_index);
            return;
        }

        self.reload_document(frame_index);
        if self.inside_container {
            self.refresh(frame_index);
        } else if invalidate_listing_when_outside {
            self.last_refresh_frame = None;
        }
        self.finish_activity_after_preview_request(frame_index);
    }
}

pub(super) fn document_field(document: &AssetDocument, row: usize) -> Option<&AssetDocumentField> {
    document
        .sections
        .iter()
        .flat_map(|section| section.fields.iter())
        .nth(row)
}
pub(super) fn available_document_action(
    document: &AssetDocument,
    row: usize,
) -> Option<&newengine_assets_api::AssetDocumentAction> {
    document
        .actions
        .iter()
        .filter(|action| {
            action.enabled && !action.requires_input && action.patch_template.is_some()
        })
        .nth(row)
}
pub(super) fn patch_status(label: &str, result: &AssetPatchResult) -> String {
    let detail = result
        .diagnostics
        .last()
        .map(|diagnostic| diagnostic.message.as_str())
        .unwrap_or("provider returned no diagnostics");
    format!(
        "{} | accepted={} | written={} | {}",
        label, result.accepted, result.written, detail
    )
}
