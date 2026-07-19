use super::super::presentation::opened_asset_status;
use super::super::*;
use super::{activation_is_deferred, log_preview_open_timing};

impl AssetInspectorRuntimeModule {
    pub(in crate::runtime) fn execute_pending_entry_activation(&mut self, frame_index: u64) {
        let Some(pending) = self.pending_entry_activation.take() else {
            return;
        };
        if activation_is_deferred(pending.requested_frame, frame_index) {
            self.pending_entry_activation = Some(pending);
            return;
        }
        self.selected_index = Some(pending.absolute_index);
        self.last_patch_result = None;
        if self
            .activity
            .as_ref()
            .is_some_and(|activity| activity.completed_frame.is_some())
        {
            self.begin_activity("OPENING", pending.requested_frame);
        }
        let opened_at = Instant::now();
        match self.inspect_cached(&pending.entry.logical_path) {
            Ok((document, document_cache_hit)) => {
                let inspect_ms = opened_at.elapsed().as_secs_f64() * 1000.0;
                self.selected_container_entry_count = 0;
                let preview_started = Instant::now();
                let preview = self.install_document(&document, pending.entry.is_container);
                let preview_ms = preview_started.elapsed().as_secs_f64() * 1000.0;
                self.status = opened_asset_status(
                    &document,
                    &preview,
                    self.selected_container_available,
                    self.selected_container_entry_count,
                );
                log_preview_open_timing(
                    "asset",
                    &pending.entry.logical_path,
                    inspect_ms,
                    preview_ms,
                    document_cache_hit,
                    self.preview_api.last_request_cache_hit(),
                );
                self.prepare_preview_entries(&document, frame_index);
                self.finish_activity_after_preview_request(frame_index);
            }
            Err(error) => {
                self.current_preview_cache_valid = false;
                self.complete_activity(frame_index);
                self.clear_selection();
                self.status = format!("engine.assets.inspect failed: {error}");
            }
        }
    }
}
