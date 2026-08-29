use super::super::presentation::{
    document_exposes_entries, preview_entry_selection, source_asset_ref,
};
use super::super::*;
use super::browser::sort_entries;
use super::{activation_is_deferred, log_preview_open_timing};

impl AssetInspectorRuntimeModule {
    pub(in crate::runtime) fn prepare_preview_entries(
        &mut self,
        document: &AssetDocument,
        frame_index: u64,
    ) {
        let source_ref = source_asset_ref(&document.asset_ref).to_owned();
        let preserves_existing = self.preview_entries_source == source_ref
            && self.selected_container_available
            && !self.preview_entries.is_empty();
        let available = document_exposes_entries(document)
            || (document.asset_ref.contains('@') && preserves_existing);
        if !available {
            self.preview_entries.clear();
            self.preview_entries_source.clear();
            self.selected_preview_entry = None;
            self.preview_entries_window_start = 0;
            self.pending_preview_entries_load = None;
            self.selected_container_entry_count = 0;
            self.selected_container_available = false;
            return;
        }

        self.selected_container_available = true;
        if preserves_existing {
            self.selected_preview_entry = preview_entry_selection(
                &self.preview_entries,
                &document.asset_ref,
                self.selected_preview_entry,
            );
            self.selected_container_entry_count = self.preview_entries.len();
            return;
        }

        self.preview_entries_source = source_ref.clone();
        if let Some(entries) = self.preview_entry_cache.get(&source_ref) {
            self.preview_entries = entries;
            self.selected_container_entry_count = self.preview_entries.len();
            self.selected_preview_entry =
                preview_entry_selection(&self.preview_entries, &document.asset_ref, None);
            self.preview_entries_window_start = selected_window_start(
                self.selected_preview_entry,
                self.preview_entries.len(),
                PREVIEW_ENTRY_ROWS,
            );
            self.pending_preview_entries_load = None;
            return;
        }

        self.preview_entries.clear();
        self.selected_preview_entry = None;
        self.preview_entries_window_start = 0;
        self.selected_container_entry_count = 0;
        self.pending_preview_entries_load = Some(PendingPreviewEntriesLoad {
            source_ref,
            requested_frame: frame_index,
        });
    }

    pub(in crate::runtime) fn execute_pending_preview_entries_load(&mut self, frame_index: u64) {
        let Some(pending) = self.pending_preview_entries_load.take() else {
            return;
        };
        if frame_index.saturating_sub(pending.requested_frame) < PREVIEW_ENTRY_LOAD_DELAY_FRAMES
            || self
                .preview_snapshot
                .as_ref()
                .is_some_and(|preview| preview.kind == AssetPreviewKind::Scene3d && !preview.ready)
        {
            self.pending_preview_entries_load = Some(pending);
            return;
        }
        if self.preview_entries_source != pending.source_ref {
            return;
        }
        match self.facade.list_container(&pending.source_ref) {
            Ok(mut entries) => {
                sort_entries(&mut entries, false);
                self.preview_entry_cache
                    .insert(&pending.source_ref, &entries);
                self.preview_entries = entries;
                self.selected_container_entry_count = self.preview_entries.len();
                self.selected_preview_entry = preview_entry_selection(
                    &self.preview_entries,
                    self.document
                        .as_ref()
                        .map(|document| document.asset_ref.as_str())
                        .unwrap_or_default(),
                    None,
                );
                self.preview_entries_window_start = self
                    .selected_preview_entry
                    .unwrap_or(0)
                    .div_euclid(PREVIEW_ENTRY_ROWS);
                self.status = format!(
                    "{} preview entries loaded | provider manifest cached",
                    self.preview_entries.len()
                );
                newengine_ulog_api::ulog::info!(
                    "asset inspector: preview entries loaded source='{}' count={} cache='store'",
                    pending.source_ref,
                    self.preview_entries.len()
                );
                self.dirty = true;
            }
            Err(error) => {
                newengine_ulog_api::ulog::warn!(
                    "asset inspector: preview entries unavailable source='{}' err='{}'",
                    pending.source_ref,
                    error
                );
                self.status = format!("Preview entries unavailable: {error}");
                self.selected_container_entry_count = 0;
                self.dirty = true;
            }
        }
    }

    pub(in crate::runtime) fn refresh_preview_entries(&mut self, frame_index: u64) {
        let source_ref = if self.preview_entries_source.trim().is_empty() {
            self.document
                .as_ref()
                .map(|document| source_asset_ref(&document.asset_ref).to_owned())
                .unwrap_or_default()
        } else {
            self.preview_entries_source.clone()
        };
        if source_ref.is_empty() || !self.selected_container_available {
            self.status = "The selected asset exposes no provider entries".to_owned();
            return;
        }
        self.preview_entry_cache.invalidate(&source_ref);
        self.preview_entries.clear();
        self.selected_preview_entry = None;
        self.selected_container_entry_count = 0;
        self.pending_preview_entries_load = Some(PendingPreviewEntriesLoad {
            source_ref,
            requested_frame: frame_index,
        });
        self.status = "Refreshing preview entries".to_owned();
        self.dirty = true;
    }

    pub(in crate::runtime) fn activate_preview_entry(&mut self, row: usize, frame_index: u64) {
        let absolute = self.preview_entries_window_start + row;
        let Some(entry) = self.preview_entries.get(absolute).cloned() else {
            return;
        };
        self.selected_preview_entry = Some(absolute);
        let already_open = self
            .document
            .as_ref()
            .is_some_and(|document| document.asset_ref == entry.logical_path)
            && self.current_preview_cache_valid;
        if already_open {
            self.status = format!("{} entry preview already open", entry.name);
            self.dirty = true;
            return;
        }
        self.pending_preview_entry_activation = Some(PendingPreviewEntryActivation {
            entry: entry.clone(),
            row: absolute,
            requested_frame: frame_index,
        });
        self.info_modal_visible = false;
        self.status = format!("Opening entry {}", entry.name);
        self.begin_activity("OPENING", frame_index);
    }

    pub(in crate::runtime) fn execute_pending_preview_entry_activation(
        &mut self,
        frame_index: u64,
    ) {
        let Some(pending) = self.pending_preview_entry_activation.take() else {
            return;
        };
        if activation_is_deferred(pending.requested_frame, frame_index) {
            self.pending_preview_entry_activation = Some(pending);
            return;
        }
        let opened_at = Instant::now();
        match self.inspect_cached(&pending.entry.logical_path) {
            Ok((document, document_cache_hit)) => {
                let inspect_ms = opened_at.elapsed().as_secs_f64() * 1000.0;
                let preview_started = Instant::now();
                self.install_document(&document, true);
                let preview_ms = preview_started.elapsed().as_secs_f64() * 1000.0;
                self.selected_preview_entry = Some(pending.row);
                self.selected_container_available = true;
                self.selected_container_entry_count = self.preview_entries.len();
                self.status = format!("Opened entry {}", pending.entry.name);
                self.finish_activity_after_preview_request(frame_index);
                self.dirty = true;
                log_preview_open_timing(
                    "entry",
                    &pending.entry.logical_path,
                    inspect_ms,
                    preview_ms,
                    document_cache_hit,
                    self.preview_api.last_request_cache_hit(),
                );
            }
            Err(error) => {
                self.current_preview_cache_valid = false;
                self.complete_activity(frame_index);
                self.status = format!("Entry preview failed: {error}");
                self.dirty = true;
            }
        }
    }
}

fn selected_window_start(selected: Option<usize>, total: usize, visible: usize) -> usize {
    let max_start = total.saturating_sub(visible.max(1));
    selected.unwrap_or(0).min(max_start)
}
