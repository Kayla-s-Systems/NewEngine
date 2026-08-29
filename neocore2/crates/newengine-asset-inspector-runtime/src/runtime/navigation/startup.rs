use super::super::presentation::provider_label;
use super::super::*;

impl AssetInspectorRuntimeModule {
    pub(in crate::runtime) fn try_open_startup_asset(&mut self, frame_index: u64) {
        if self.startup_asset_opened {
            return;
        }
        let Some(asset_ref) = self.startup_asset_ref.clone() else {
            self.startup_asset_opened = true;
            return;
        };
        if self
            .last_startup_asset_attempt_frame
            .is_some_and(|last| frame_index.saturating_sub(last) < 30)
        {
            return;
        }
        self.last_startup_asset_attempt_frame = Some(frame_index);
        self.startup_asset_attempts = self.startup_asset_attempts.saturating_add(1);
        self.begin_activity("OPENING", frame_index);
        match self.inspect_cached(&asset_ref) {
            Ok((document, _document_cache_hit)) => {
                let source_path = asset_ref.split('@').next().unwrap_or(&asset_ref);
                self.current_path = source_path
                    .rsplit_once('/')
                    .map(|(parent, _)| parent.to_owned())
                    .unwrap_or_default();
                self.inside_container = false;
                self.selected_index = None;
                self.selected_container_entry_count = 0;
                let status = format!(
                    "Opened {} from {} | provider={}",
                    document.title,
                    STARTUP_ASSET_ENV,
                    provider_label(&document)
                );
                self.install_document(&document, false);
                self.finish_activity_after_preview_request(frame_index);
                self.prepare_preview_entries(&document, frame_index);
                self.status = status;
                self.startup_asset_opened = true;
                self.last_refresh_frame = None;
                self.dirty = true;
                newengine_ulog_api::ulog::info!(
                    "asset inspector: startup asset opened ref='{}' preview_requested=true",
                    asset_ref
                );
            }
            Err(error) => {
                self.complete_activity(frame_index);
                if self.startup_asset_attempts == 1 || self.startup_asset_attempts.is_multiple_of(4)
                {
                    newengine_ulog_api::ulog::warn!(
                        "asset inspector: startup asset deferred ref='{}' frame={} attempt={} err='{}'",
                        asset_ref,
                        frame_index,
                        self.startup_asset_attempts,
                        error
                    );
                }
            }
        }
    }
}
