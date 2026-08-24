use super::*;

mod authored_ui;
mod editor;
mod game_gui;
mod presentation;
mod right_edit;
mod toasts;

impl ScreenProfileRuntimeState {
    pub(crate) fn load() -> Self {
        let config = load_screen_profile_config();
        let descriptor =
            screen_profile_descriptor(config.profile, config.game_ui_root_surface_id.clone());
        let mut descriptor = descriptor;
        descriptor.game_ui_document_ref = config.game_ui_document_ref.clone();
        if let Some(game_gui) = config.game_gui.as_ref().filter(|config| config.enabled) {
            let errors = game_gui.validation_errors();
            if errors.is_empty() {
                newengine_ulog_api::ulog::info!(
                    "screen profile: game_gui enabled layers={} policy='authored .neui layer stack'",
                    game_gui.layers.len(),
                );
            } else {
                newengine_ulog_api::ulog::warn!(
                    "screen profile: game_gui config invalid; layer stack disabled errors='{}'",
                    errors.join("; "),
                );
            }
        }
        let presentation_state_id = config.presentation_flow.as_ref().and_then(|flow| {
            let validation_errors = flow.validation_errors();
            if validation_errors.is_empty() {
                Some(flow.initial_state.trim().to_owned())
            } else {
                if flow.enabled {
                    newengine_ulog_api::ulog::warn!(
                        "screen profile: presentation_flow is enabled but invalid; legacy game_ui_document_ref path remains active flow_id='{}' initial_state='{}' states={} transitions={} errors='{}'",
                        flow.id,
                        flow.initial_state,
                        flow.states.len(),
                        flow.transitions.len(),
                        validation_errors.join("; "),
                    );
                }
                None
            }
        });
        newengine_ulog_api::ulog::info!(
            "screen profile: loaded profile='{}' layout='{}' focus={:?} panels={} game_ui_root={} game_ui_document_ref={}",
            descriptor.profile.id(),
            descriptor.layout_id,
            descriptor.input_focus_policy,
            descriptor.panels.len(),
            descriptor
                .game_ui_root_surface_id
                .as_deref()
                .unwrap_or("<none>"),
            descriptor
                .game_ui_document_ref
                .as_deref()
                .unwrap_or("<none>"),
        );
        Self {
            config,
            descriptor,
            last_published_profile: None,
            published_surfaces: BTreeSet::new(),
            mounted_game_ui_document_ref: None,
            failed_game_ui_document_ref: None,
            mounted_game_gui_layers: BTreeMap::new(),
            failed_game_gui_layers: BTreeSet::new(),
            game_gui_visibility_overrides: BTreeMap::new(),
            game_gui_applied_visibility: BTreeMap::new(),
            presentation_state_id,
            last_published_presentation_state_id: None,
            presentation_runtime_ready: false,
            mounted_presentation_documents: BTreeMap::new(),
            failed_presentation_documents: BTreeSet::new(),
            last_presentation_action_frame: u64::MAX,
            pending_presentation_action_id: None,
            pending_presentation_action_frame: u64::MAX,
            last_right_edit_selection_key: String::new(),
            cached_right_edit_document: None,
            cached_right_edit_error: None,
            hidden_panels: BTreeSet::new(),
            last_runtime_command_frame: u64::MAX,
            last_dock_click_frame: u64::MAX,
            last_menu_click_frame: u64::MAX,
            active_menu_id: None,
            last_toast_surface_version: None,
            last_toast_surface_extent: [0, 0],
        }
    }

    pub(crate) fn install_initial_resources(&self, resources: &mut Resources) {
        install_runtime_session_resources(resources);
        if resources.get::<EditorCommandRegistry>().is_none() {
            resources.insert(default_runtime_editor_commands());
        }
        if resources.get::<UiEditorViewportState>().is_none() {
            resources.insert(UiEditorViewportState::default());
        }
        let mut descriptor = self.descriptor.clone();
        descriptor.input_focus_policy = self.active_input_focus_policy();
        resources.insert(UiScreenProfileState {
            version: 1,
            frame_index: 0,
            descriptor,
        });
        if let Some(state) = self.presentation_flow_state(0, "presentation flow initialized") {
            resources.insert(state);
        }
        resources.insert(self.game_gui_stack_state(0));
        resources.insert(UiGameLayerCommandQueue::default());
    }

    /// Publishes profile DTOs and optional UI surface nodes for the current frame.
    ///
    /// Returns true when provider UI should be refreshed this frame. This does not
    /// touch render backend state, scene state or ECS storage; it only publishes
    /// `engine.ui` composition data and provider-safe input-focus policy.
    pub(crate) fn prepare_frame(&mut self, resources: &mut Resources, frame_index: u64) -> bool {
        if let Some(report) = newengine_asset_hot_reload_runtime::poll_asset_file_watcher(resources)
        {
            let failures = report
                .operations
                .iter()
                .filter(|operation| !operation.ok)
                .count();
            if failures == 0 {
                newengine_ulog_api::ulog::info!(
                    "asset hot reload: changed={} invalidated={} reimported={} cycles={}",
                    report.changed_refs.len(),
                    report.plan.invalidation_order.len(),
                    report.operations.len(),
                    report.plan.cycles.len(),
                );
            } else {
                newengine_ulog_api::ulog::warn!(
                    "asset hot reload: changed={} invalidated={} operations={} failures={} cycles={}",
                    report.changed_refs.len(),
                    report.plan.invalidation_order.len(),
                    report.operations.len(),
                    failures,
                    report.plan.cycles.len(),
                );
            }
        }
        let presentation_changed = self.update_presentation_flow(resources, frame_index);
        let mut published_descriptor = self.descriptor.clone();
        published_descriptor.input_focus_policy = self.active_input_focus_policy();
        resources.insert(UiScreenProfileState {
            version: 1,
            frame_index,
            descriptor: published_descriptor,
        });
        if let Some(state) = self.presentation_flow_state(
            frame_index,
            if presentation_changed {
                "presentation flow transitioned"
            } else {
                "presentation flow active"
            },
        ) {
            resources.insert(state);
        }

        self.update_menu_interaction(resources, frame_index);
        self.update_editor_runtime_state(resources, frame_index);
        self.update_editor_viewport_interaction(resources, frame_index);
        self.update_dock_interaction(resources, frame_index);
        self.publish_editor_layout_state(resources, frame_index);
        self.publish_focus_policy(resources);
        let profile_changed = self.last_published_profile != Some(self.descriptor.profile);
        let game_gui_changed = self.prepare_game_gui(resources, frame_index, profile_changed);
        let mut refresh_ui = presentation_changed || game_gui_changed;

        match self.descriptor.profile {
            UiScreenProfile::Game => {
                if self.active_presentation_state().is_some() {
                    refresh_ui |= self
                        .prepare_presentation_flow_surface(profile_changed, presentation_changed);
                } else if self.has_active_game_gui() {
                    // Game GUI owns authored HUD/menu/overlay layers. Keep the legacy
                    // single-document path disabled to avoid mounting the placeholder HUD twice.
                    refresh_ui |=
                        self.hide_profile_surface(UI_SURFACE_GAME_PRESENTATION, profile_changed);
                } else if let Some(document_ref) = self
                    .descriptor
                    .game_ui_document_ref
                    .as_ref()
                    .map(|it| it.trim().to_owned())
                    .filter(|it| !it.is_empty())
                {
                    let already_mounted =
                        self.mounted_game_ui_document_ref.as_deref() == Some(document_ref.as_str());
                    let failed_same_document =
                        self.failed_game_ui_document_ref.as_deref() == Some(document_ref.as_str());
                    let should_mount =
                        profile_changed || (!already_mounted && !failed_same_document);
                    if should_mount {
                        match self.mount_authored_ui_document(document_ref.as_str()) {
                            Ok(surface_id) => {
                                self.published_surfaces.insert(surface_id);
                                self.mounted_game_ui_document_ref = Some(document_ref.clone());
                                self.failed_game_ui_document_ref = None;
                                refresh_ui = true;
                            }
                            Err(error) => {
                                self.failed_game_ui_document_ref = Some(document_ref.clone());
                                newengine_ulog_api::ulog::warn!(
                                    "screen profile: authored game .neui mount failed ref='{}' err='{}' policy='no generated gameplay HUD fallback; retry only when document/profile changes'",
                                    document_ref,
                                    error
                                );
                            }
                        }
                    }
                    refresh_ui |=
                        self.hide_profile_surface(UI_SURFACE_GAME_PRESENTATION, profile_changed);
                } else {
                    if profile_changed {
                        newengine_ulog_api::ulog::warn!(
                            "screen profile: game profile has no authored game_ui_document_ref or presentation_flow; no generated gameplay UI fallback is allowed"
                        );
                    }
                    refresh_ui |=
                        self.hide_profile_surface(UI_SURFACE_GAME_PRESENTATION, profile_changed);
                }
                refresh_ui |= self.prepare_editing_overlay(resources, frame_index, profile_changed);
            }
            UiScreenProfile::Headless => {
                refresh_ui |= self.hide_profile_surface(UI_SURFACE_EDITOR_SHELL, profile_changed);
                refresh_ui |=
                    self.hide_profile_surface(UI_SURFACE_GAME_PRESENTATION, profile_changed);
            }
        }

        refresh_ui |= self.prepare_toast_surface(resources, frame_index, profile_changed);

        if profile_changed {
            newengine_ulog_api::ulog::info!(
                "screen profile: active profile='{}' focus={:?} surface='{}' panels={} render_backend='unchanged'",
                self.descriptor.profile.id(),
                self.descriptor.input_focus_policy,
                self.descriptor.surface_id,
                self.descriptor.panels.len(),
            );
            self.last_published_profile = Some(self.descriptor.profile);
            refresh_ui = true;
        }

        refresh_ui
    }
}
