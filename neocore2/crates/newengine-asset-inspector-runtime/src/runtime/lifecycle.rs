use super::*;

impl AssetInspectorRuntimeModule {
    pub(super) fn ensure_surface_mounted(&mut self, frame_index: u64) {
        if self.surface_mounted {
            return;
        }
        let should_attempt = self
            .last_surface_mount_attempt_frame
            .is_none_or(|last| frame_index.saturating_sub(last) >= 30);
        if !should_attempt {
            return;
        }
        self.last_surface_mount_attempt_frame = Some(frame_index);
        match mount_asset_inspector_surface() {
            Ok(_) => {
                self.surface_mounted = true;
                self.status = "Provider-driven Asset Inspector surface mounted".to_owned();
                self.dirty = true;
            }
            Err(error) => {
                self.status = format!("Waiting for authored UI services: {error}");
                if frame_index == 0 || frame_index.is_multiple_of(120) {
                    newengine_ulog_api::ulog::warn!(
                        "asset inspector: surface mount deferred frame={} err='{}'",
                        frame_index,
                        error
                    );
                }
            }
        }
    }
}

impl<E: Send + 'static> Module<E> for AssetInspectorRuntimeModule {
    fn id(&self) -> &'static str {
        "app.asset_inspector.runtime"
    }

    fn startup_requires(&self) -> &'static [EngineReadinessKey] {
        const REQUIRES: &[EngineReadinessKey] = &[EngineReadinessKey::EnginePluginsReady];
        REQUIRES
    }

    fn start(&mut self, _ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        newengine_ulog_api::ulog::info!(
            "asset inspector: runtime start startup_asset_ref={:?} env='{}'",
            self.startup_asset_ref,
            STARTUP_ASSET_ENV
        );
        self.dirty = true;
        Ok(())
    }

    fn update(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        let frame_index = ctx.frame().map(|frame| frame.frame_index).unwrap_or(0);
        self.ensure_surface_mounted(frame_index);
        self.try_open_startup_asset(frame_index);
        let input = ctx.resources().get::<UiInputFrame>().cloned();
        if let Some(dispatch) = ctx.resources().get::<UiEventDispatchFrame>().cloned() {
            self.handle_actions(&dispatch);
            self.handle_preview_camera_input(&dispatch, input.as_ref());
        }
        self.execute_pending_entry_activation(frame_index);
        self.execute_pending_preview_entry_activation(frame_index);
        self.execute_pending_preview_entries_load(frame_index);
        if self.last_refresh_frame.is_none()
            && self.pending_entry_activation.is_none()
            && self.pending_preview_entry_activation.is_none()
            && self.activity.is_none()
        {
            // Listings are invalidated explicitly by navigation, mode changes,
            // mutations and the Refresh action. Do not poll VFS by frame count:
            // high-refresh-rate tool windows otherwise rescan several times per
            // second and can restart expensive preview work unexpectedly.
            self.refresh(frame_index);
        }
        if self.preview_snapshot.is_some() {
            let snapshot = self.preview_api.snapshot();
            if self.preview_snapshot.as_ref() != Some(&snapshot) {
                self.preview_snapshot = Some(snapshot);
                self.finish_activity_after_preview_request(frame_index);
                self.dirty = true;
            }
        }
        self.tick_activity(frame_index);
        if self.dirty && self.surface_mounted {
            let (activity_progress_01, activity_label) = self.activity_view(frame_index);
            let published = publish_inspector_state(InspectorUiSnapshot {
                frame_index,
                current_path: &self.current_path,
                inside_container: self.inside_container,
                mode: self.mode,
                browser_window_start: self.browser_window_start,
                entries: &self.entries,
                selected_index: self.selected_index,
                document: self.document.as_ref(),
                preview: self.preview_snapshot.as_ref(),
                last_patch_result: self.last_patch_result.as_ref(),
                selected_container_entry_count: self.selected_container_entry_count,
                selected_container_available: self.selected_container_available,
                preview_entries: &self.preview_entries,
                selected_preview_entry: self.selected_preview_entry,
                preview_entries_window_start: self.preview_entries_window_start,
                preview_entries_loading: self.pending_preview_entries_load.is_some(),
                info_modal_visible: self.info_modal_visible,
                status: &self.status,
                hover_hint: &self.hover_hint,
                activity_progress_01,
                activity_width_px: activity_progress_01 * ACTIVITY_BAR_INNER_WIDTH_PX,
                activity_label,
                text_asset_ref: self
                    .text_editor
                    .as_ref()
                    .map(|editor| editor.asset_ref.as_str()),
                text_lines: self
                    .text_editor
                    .as_ref()
                    .map(|editor| editor.lines.as_slice()),
                text_page: self
                    .text_editor
                    .as_ref()
                    .map(|editor| editor.page)
                    .unwrap_or(0),
                text_language: self
                    .text_editor
                    .as_ref()
                    .map(|editor| editor.language.as_str())
                    .unwrap_or_default(),
                text_editable: self
                    .text_editor
                    .as_ref()
                    .is_some_and(|editor| editor.editable),
                text_dirty: self.text_editor.as_ref().is_some_and(|editor| editor.dirty),
                syntax_preview: self.syntax_preview.as_ref(),
                syntax_editor: self.syntax_editor.as_ref(),
                preview_pointer_captured: self.preview_pointer_captured,
            });
            self.dirty = !published;
        }
        Ok(())
    }
}
