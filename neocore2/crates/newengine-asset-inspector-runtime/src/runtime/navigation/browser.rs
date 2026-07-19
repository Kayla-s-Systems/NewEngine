use super::super::*;

impl AssetInspectorRuntimeModule {
    pub(in crate::runtime) fn refresh_with_activity(&mut self, frame_index: u64) {
        self.document_cache.clear();
        self.preview_entry_cache.clear();
        self.preview_api.invalidate_all();
        self.current_preview_cache_valid = false;
        self.begin_activity("REFRESHING", frame_index);
        self.refresh(frame_index);
        if self.selected_container_available {
            self.refresh_preview_entries(frame_index);
        }
        self.complete_activity(frame_index);
    }

    pub(in crate::runtime) fn refresh(&mut self, frame_index: u64) {
        let listing = if self.inside_container {
            self.facade.list_container(&self.current_path)
        } else {
            self.facade.list_path(&self.current_path)
        };
        match listing {
            Ok(mut entries) => {
                entries.retain(|entry| self.mode.accepts(entry.is_directory));
                sort_entries(&mut entries, true);
                let content_entry_count = entries.len();
                prepend_parent_navigation(&mut entries, &self.current_path, self.inside_container);
                self.entries = entries;
                let max_start = self.entries.len().saturating_sub(ENTRY_ROWS);
                self.browser_window_start = self.browser_window_start.min(max_start);
                if self
                    .selected_index
                    .is_some_and(|index| index >= self.entries.len())
                {
                    self.clear_browser_selection();
                }
                self.status = format!(
                    "{} entries | {} | provider-routed through engine.assets",
                    content_entry_count,
                    self.mode.label()
                );
            }
            Err(error) => {
                self.entries.clear();
                self.status = format!("engine.assets listing unavailable: {error}");
            }
        }
        self.last_refresh_frame = Some(frame_index);
        self.dirty = true;
    }

    pub(in crate::runtime) fn set_mode(&mut self, mode: AssetInspectorMode) {
        if self.mode == mode {
            return;
        }
        self.mode = mode;
        self.reset_browser_listing();
    }

    pub(in crate::runtime) fn navigate_up(&mut self) {
        let parent = parent_logical_path(&self.current_path);
        self.enter_browser_location(parent, false);
    }

    fn reset_browser_listing(&mut self) {
        self.browser_window_start = 0;
        self.clear_browser_selection();
        self.last_refresh_frame = None;
    }

    fn enter_browser_location(&mut self, logical_path: String, inside_container: bool) {
        self.current_path = logical_path;
        self.inside_container = inside_container;
        self.reset_browser_listing();
    }

    pub(in crate::runtime) fn activate_row(&mut self, row: usize, open: bool, frame_index: u64) {
        let absolute = self.browser_window_start + row;
        let Some(entry) = self.entries.get(absolute).cloned() else {
            return;
        };
        if entry.is_parent_navigation() {
            self.navigate_up();
            return;
        }
        if entry.is_directory {
            self.enter_browser_location(entry.logical_path, false);
            return;
        }
        let already_selected = self.selected_index == Some(absolute);
        let already_open = self
            .document
            .as_ref()
            .is_some_and(|document| document.asset_ref == entry.logical_path)
            && self.preview_snapshot.is_some()
            && self.current_preview_cache_valid;
        self.selected_index = Some(absolute);
        self.last_patch_result = None;
        if already_open {
            self.pending_entry_activation = None;
            self.status = format!("{} preview already open | cached target reused", entry.name);
            self.dirty = true;
            return;
        }
        if self
            .pending_entry_activation
            .as_ref()
            .is_some_and(|pending| {
                pending.absolute_index == absolute
                    && pending.entry.logical_path == entry.logical_path
            })
        {
            return;
        }
        if !open && !already_selected {
            self.pending_entry_activation = None;
            self.status = format!("Selected {} | click again to open preview", entry.name);
            self.dirty = true;
            return;
        }

        // Files always open their provider-routed preview on double-click.
        // Provider entries are projected into the embedded panel under Preview.
        self.pending_entry_activation = Some(PendingEntryActivation {
            entry: entry.clone(),
            absolute_index: absolute,
            requested_frame: frame_index,
        });
        self.info_modal_visible = false;
        self.status = format!("Opening preview for {} through engine.assets", entry.name);
        self.begin_activity("OPENING", frame_index);
    }

    pub(in crate::runtime) fn open_selected_container(&mut self, frame_index: u64) {
        let path = if !self.preview_entries_source.trim().is_empty() {
            self.preview_entries_source.clone()
        } else if let Some(path) = self
            .selected_index
            .and_then(|index| self.entries.get(index))
            .map(|entry| entry.logical_path.clone())
        {
            path
        } else {
            self.status = "Select an asset with provider-declared entries".to_owned();
            return;
        };
        if !self.open_container_path(&path, frame_index) {
            self.status = format!("Provider did not expose addressable entries for {path}");
        }
    }

    pub(in crate::runtime) fn open_container_path(
        &mut self,
        logical_path: &str,
        frame_index: u64,
    ) -> bool {
        let owns_activity = self.activity.is_none();
        if owns_activity {
            self.begin_activity("OPENING", frame_index);
        }
        let mut entries = match self.facade.list_container(logical_path) {
            Ok(entries) => entries,
            Err(_) => {
                if owns_activity {
                    self.complete_activity(frame_index);
                }
                return false;
            }
        };
        sort_entries(&mut entries, false);
        let content_entry_count = entries.len();
        self.selected_container_entry_count = content_entry_count;
        self.selected_container_available = content_entry_count > 0;
        prepend_parent_navigation(&mut entries, logical_path, true);
        self.current_path = logical_path.to_owned();
        self.inside_container = true;
        self.mode = AssetInspectorMode::All;
        self.browser_window_start = 0;
        self.entries = entries;
        self.clear_browser_selection();
        self.last_refresh_frame = Some(frame_index);
        self.status = format!(
            "Opened provider manifest {} | {} entries",
            logical_path, content_entry_count
        );
        self.complete_activity(frame_index);
        self.dirty = true;
        true
    }
}

pub(in crate::runtime::navigation) fn sort_entries(
    entries: &mut [InspectorEntry],
    directories_first: bool,
) {
    entries.sort_by(|a, b| {
        let directory_order = directories_first
            .then(|| b.is_directory.cmp(&a.is_directory))
            .unwrap_or(std::cmp::Ordering::Equal);
        directory_order.then_with(|| {
            a.name
                .to_ascii_lowercase()
                .cmp(&b.name.to_ascii_lowercase())
        })
    });
}

pub(super) fn parent_logical_path(path: &str) -> String {
    path.trim_matches('/')
        .rsplit_once('/')
        .map(|(parent, _)| parent.to_owned())
        .unwrap_or_default()
}

pub(in crate::runtime) fn prepend_parent_navigation(
    entries: &mut Vec<InspectorEntry>,
    current_path: &str,
    inside_container: bool,
) {
    if inside_container || !current_path.trim_matches('/').is_empty() {
        entries.insert(
            0,
            InspectorEntry::parent_navigation(parent_logical_path(current_path)),
        );
    }
}
