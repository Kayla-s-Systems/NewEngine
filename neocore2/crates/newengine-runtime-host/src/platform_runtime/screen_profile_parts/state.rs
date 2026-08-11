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
    pub(crate) fn load() -> Self {
        let config = load_screen_profile_config();
        let descriptor =
            screen_profile_descriptor(config.profile, config.game_ui_root_surface_id.clone());
        let mut descriptor = descriptor;
        descriptor.game_ui_document_ref = config.game_ui_document_ref.clone();
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
            editor_runtime_mode: UiEditorRuntimeMode::Edit,
            hidden_panels: BTreeSet::new(),
            last_runtime_button_pointer_frame: u64::MAX,
            last_dock_click_frame: u64::MAX,
            last_menu_click_frame: u64::MAX,
            active_menu_id: None,
        }
    }

    pub(crate) fn install_initial_resources(&self, resources: &mut Resources) {
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
    }

    fn active_presentation_state(&self) -> Option<&ScreenPresentationStateConfig> {
        let flow = self
            .config
            .presentation_flow
            .as_ref()?
            .is_valid()
            .then_some(self.config.presentation_flow.as_ref()?)?;
        flow.state(self.presentation_state_id.as_deref()?)
    }

    fn presentation_flow_state(
        &self,
        frame_index: u64,
        reason: impl Into<String>,
    ) -> Option<UiPresentationFlowState> {
        let flow = self
            .config
            .presentation_flow
            .as_ref()?
            .is_valid()
            .then_some(self.config.presentation_flow.as_ref()?)?;
        let state = flow.state(self.presentation_state_id.as_deref()?)?;
        Some(UiPresentationFlowState {
            version: 1,
            frame_index,
            flow_id: flow.id.clone(),
            state_id: state.id.clone(),
            active_surface_id: state
                .surface_id
                .as_ref()
                .map(|surface| surface.trim().to_owned())
                .filter(|surface| !surface.is_empty()),
            blocks_world_bootstrap: state.blocks_world_bootstrap,
            blocks_gameplay_input: state.blocks_gameplay_input,
            runtime_ready: self.presentation_runtime_ready,
            reason: reason.into(),
        })
    }

    fn active_input_focus_policy(&self) -> UiScreenInputFocusPolicy {
        self.active_presentation_state()
            .map(|state| state.input_focus_policy)
            .unwrap_or(self.descriptor.input_focus_policy)
    }

    /// Publishes profile DTOs and optional UI surface nodes for the current frame.
    ///
    /// Returns true when provider UI should be refreshed this frame. This does not
    /// touch render backend state, scene state or ECS storage; it only publishes
    /// `engine.ui` composition data and provider-safe input-focus policy.
    pub(crate) fn prepare_frame(&mut self, resources: &mut Resources, frame_index: u64) -> bool {
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
        self.update_dock_interaction(resources, frame_index);
        self.publish_editor_layout_state(resources, frame_index);
        self.publish_focus_policy(resources);
        let profile_changed = self.last_published_profile != Some(self.descriptor.profile);
        let mut refresh_ui = presentation_changed;

        match self.descriptor.profile {
            UiScreenProfile::Editor => {
                refresh_ui |=
                    self.hide_profile_surface(UI_SURFACE_GAME_PRESENTATION, profile_changed);
                if self.config.publish_editor_shell {
                    let layout = editor_layout_metrics(resources, &self.hidden_panels);
                    let mut node = EditorScreen::default().surface_node(
                        frame_index,
                        self.editor_runtime_mode,
                        &layout,
                        self.active_menu_id.as_deref(),
                    );
                    self.append_right_edit_window(resources, &mut node, &layout);
                    self.append_toast_components(resources, &mut node, &layout);
                    sort_components_by_layout_y(&mut node.components);
                    publish_screen_node_tree_request(&UiNodeTreeRequest::from_surface_node(
                        &node,
                        UiNodeRequestSourceKind::Generated,
                    ));
                    self.published_surfaces
                        .insert(UI_SURFACE_EDITOR_SHELL.to_owned());
                    refresh_ui = true;
                } else {
                    refresh_ui |=
                        self.hide_profile_surface(UI_SURFACE_EDITOR_SHELL, profile_changed);
                }
            }
            UiScreenProfile::Game => {
                refresh_ui |= self.hide_profile_surface(UI_SURFACE_EDITOR_SHELL, profile_changed);
                if self.active_presentation_state().is_some() {
                    refresh_ui |= self
                        .prepare_presentation_flow_surface(profile_changed, presentation_changed);
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
            }
            UiScreenProfile::Headless => {
                refresh_ui |= self.hide_profile_surface(UI_SURFACE_EDITOR_SHELL, profile_changed);
                refresh_ui |=
                    self.hide_profile_surface(UI_SURFACE_GAME_PRESENTATION, profile_changed);
            }
        }

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

    fn update_presentation_flow(&mut self, resources: &Resources, frame_index: u64) -> bool {
        let Some(flow) = self
            .config
            .presentation_flow
            .as_ref()
            .filter(|flow| flow.is_valid())
            .cloned()
        else {
            return false;
        };

        if self.presentation_state_id.is_none() {
            self.presentation_state_id = Some(flow.initial_state.trim().to_owned());
        }
        if let Some(shared) = resources.get::<UiPresentationFlowState>() {
            if shared.flow_id == flow.id {
                self.presentation_runtime_ready |= shared.runtime_ready;
            }
        }

        let current_state = self
            .presentation_state_id
            .as_deref()
            .unwrap_or(flow.initial_state.as_str())
            .to_owned();
        let detected_action_id = if self.last_presentation_action_frame == frame_index {
            None
        } else {
            let click_action = resources
                .get::<UiEventDispatchFrame>()
                .and_then(|dispatch| {
                    dispatch.actions.iter().find(|action| {
                        action.trigger == UiNodeEventTrigger::Click
                            && flow.transitions.iter().any(|transition| {
                                transition.from == current_state
                                    && transition.on_action.as_deref()
                                        == Some(action.action_id.as_str())
                            })
                    })
                })
                .map(|action| action.action_id.clone());
            click_action.or_else(|| {
                let input = resources.get::<UiInputFrame>();
                let escape_pressed =
                    input.is_some_and(|input| input.is_key_pressed(newengine_ui_api::keys::ESCAPE));
                let gamepad_start_pressed = input.is_some_and(|input| {
                    input
                        .gamepad_buttons_pressed
                        .contains(newengine_input_api::gamepad_button::START)
                });
                let gamepad_back_pressed = input.is_some_and(|input| {
                    input
                        .gamepad_buttons_pressed
                        .contains(newengine_input_api::gamepad_button::EAST)
                });
                if (escape_pressed || gamepad_back_pressed)
                    && flow.has_action_transition(&current_state, "ui.back")
                {
                    return Some("ui.back".to_owned());
                }
                if (escape_pressed || gamepad_start_pressed)
                    && flow.has_action_transition(
                        &current_state,
                        newengine_ui::UI_ACTION_TOGGLE_PRIMARY_UI,
                    )
                {
                    return Some(newengine_ui::UI_ACTION_TOGGLE_PRIMARY_UI.to_owned());
                }
                None
            })
        };

        const FRONTEND_ACTION_FEEDBACK_HOLD_FRAMES: u64 = 7;
        let action_id = if let Some(pending_action) = self.pending_presentation_action_id.clone() {
            if frame_index.saturating_sub(self.pending_presentation_action_frame)
                < FRONTEND_ACTION_FEEDBACK_HOLD_FRAMES
            {
                return false;
            }
            self.pending_presentation_action_id = None;
            self.pending_presentation_action_frame = u64::MAX;
            Some(pending_action)
        } else if let Some(action_id) = detected_action_id {
            self.pending_presentation_action_id = Some(action_id.clone());
            self.pending_presentation_action_frame = frame_index;
            newengine_ulog_api::ulog::debug!(
                "screen profile: frontend action feedback hold flow='{}' state='{}' action='{}' frames={}",
                flow.id,
                current_state,
                action_id,
                FRONTEND_ACTION_FEEDBACK_HOLD_FRAMES,
            );
            return false;
        } else {
            None
        };

        let transition = action_id
            .as_deref()
            .and_then(|action_id| {
                flow.transitions.iter().find(|transition| {
                    transition.from == current_state
                        && transition.on_action.as_deref() == Some(action_id)
                })
            })
            .or_else(|| {
                self.presentation_runtime_ready.then(|| {
                    flow.transitions.iter().find(|transition| {
                        transition.from == current_state && transition.on_runtime_ready
                    })
                })?
            })
            .cloned();

        let Some(transition) = transition else {
            return false;
        };
        if flow.state(transition.to.trim()).is_none() {
            return false;
        }

        if action_id.is_some() {
            self.last_presentation_action_frame = frame_index;
        }
        self.presentation_state_id = Some(transition.to.trim().to_owned());
        if transition.reset_runtime_ready {
            self.presentation_runtime_ready = false;
        }
        newengine_ulog_api::ulog::info!(
            "screen profile: presentation flow transition flow='{}' from='{}' to='{}' trigger='{}' frame={}",
            flow.id,
            transition.from,
            transition.to,
            action_id.as_deref().unwrap_or("runtime_ready"),
            frame_index,
        );
        true
    }

    fn prepare_presentation_flow_surface(
        &mut self,
        profile_changed: bool,
        state_changed: bool,
    ) -> bool {
        let Some(flow) = self
            .config
            .presentation_flow
            .as_ref()
            .filter(|flow| flow.is_valid())
            .cloned()
        else {
            return false;
        };
        let Some(active_state_id) = self.presentation_state_id.clone() else {
            return false;
        };
        let Some(active_state) = flow.state(&active_state_id).cloned() else {
            return false;
        };

        let mut refresh = false;
        for state in &flow.states {
            if state.id == active_state_id {
                continue;
            }
            if let Some(surface_id) = state
                .surface_id
                .as_deref()
                .map(str::trim)
                .filter(|surface| !surface.is_empty())
            {
                refresh |= self.hide_profile_surface(surface_id, profile_changed || state_changed);
            }
            if let Some(document_ref) = state
                .document_ref
                .as_deref()
                .map(str::trim)
                .filter(|document| !document.is_empty())
            {
                if let Some(surface_id) = self
                    .mounted_presentation_documents
                    .get(document_ref)
                    .cloned()
                {
                    refresh |= self.hide_profile_surface(
                        surface_id.as_str(),
                        profile_changed || state_changed,
                    );
                }
            }
        }

        refresh |= self.hide_profile_surface(
            UI_SURFACE_GAME_PRESENTATION,
            profile_changed || state_changed,
        );

        let Some(document_ref) = active_state
            .document_ref
            .as_deref()
            .map(str::trim)
            .filter(|document| !document.is_empty())
            .map(str::to_owned)
        else {
            self.last_published_presentation_state_id = Some(active_state_id);
            return refresh || state_changed;
        };

        let already_mounted = self
            .mounted_presentation_documents
            .contains_key(document_ref.as_str());
        let failed_same_document = self
            .failed_presentation_documents
            .contains(document_ref.as_str());
        let should_mount =
            profile_changed || state_changed || (!already_mounted && !failed_same_document);
        if should_mount {
            match self.mount_authored_ui_document(document_ref.as_str()) {
                Ok(surface_id) => {
                    self.published_surfaces.insert(surface_id.clone());
                    self.mounted_presentation_documents
                        .insert(document_ref.clone(), surface_id);
                    self.failed_presentation_documents
                        .remove(document_ref.as_str());
                    refresh = true;
                }
                Err(error) => {
                    self.failed_presentation_documents
                        .insert(document_ref.clone());
                    newengine_ulog_api::ulog::warn!(
                        "screen profile: authored presentation document mount failed flow='{}' state='{}' ref='{}' err='{}'",
                        flow.id,
                        active_state_id,
                        document_ref,
                        error,
                    );
                }
            }
        }
        self.last_published_presentation_state_id = Some(active_state_id);
        refresh
    }

    fn mount_authored_ui_document(&mut self, document_ref: &str) -> Result<String, String> {
        // Screen profile initialization can run before scene/content bootstrap. Mount
        // canonical runtime roots here as a synchronous prerequisite so authored HUD
        // compilation never depends on a later world tick or the process CWD.
        let assets =
            newengine_assets::AssetServiceClient::new(newengine_plugin_host::default_host_api());
        let roots = crate::asset_bootstrap::collect_app_asset_roots("", "NEWENGINE_APP_ASSETS");
        crate::asset_bootstrap::mount_asset_roots_best_effort(&assets, &roots);

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
    fn update_menu_interaction(&mut self, resources: &Resources, frame_index: u64) {
        if self.descriptor.profile != UiScreenProfile::Editor
            || self.last_menu_click_frame == frame_index
        {
            return;
        }
        let Some(action_id) = clicked_dispatch_action(resources, "editor.menu.") else {
            return;
        };
        let menu_id = action_id.trim_start_matches("editor.menu.");
        if !EDITOR_CHROME.menu.iter().any(|menu| menu.id == menu_id) {
            if self.active_menu_id.is_some() && !action_id.starts_with("editor.menu_popup.") {
                self.active_menu_id = None;
                self.last_menu_click_frame = frame_index;
            }
            return;
        }
        self.last_menu_click_frame = frame_index;
        if self.active_menu_id.as_deref() == Some(menu_id) {
            self.active_menu_id = None;
        } else {
            self.active_menu_id = Some(menu_id.to_owned());
        }
        newengine_ulog_api::ulog::info!(
            "editor menu: '{}' {} via ui.dispatch_input_v1",
            menu_id,
            if self.active_menu_id.as_deref() == Some(menu_id) {
                "opened"
            } else {
                "closed"
            }
        );
    }

    fn update_editor_runtime_state(&mut self, resources: &mut Resources, frame_index: u64) {
        if self.descriptor.profile != UiScreenProfile::Editor {
            resources.insert(UiEditorRuntimeState {
                version: 1,
                frame_index,
                mode: UiEditorRuntimeMode::Play,
                source_surface: self.descriptor.surface_id.clone(),
                reason: "game profile owns runtime presentation".to_owned(),
            });
            return;
        }

        let dispatch_requested = if self.last_runtime_button_pointer_frame != frame_index {
            clicked_dispatch_action(resources, "editor.runtime.").and_then(|action_id| {
                EDITOR_CHROME
                    .runtime_actions
                    .iter()
                    .find(|action| action.action_id == action_id)
                    .map(|action| action.mode)
            })
        } else {
            None
        };
        let requested = dispatch_requested.inspect(|_| {
            self.last_runtime_button_pointer_frame = frame_index;
        });

        if let Some(mode) = requested.filter(|mode| *mode != self.editor_runtime_mode) {
            self.editor_runtime_mode = mode;
            match mode {
                UiEditorRuntimeMode::Edit => newengine_ulog_api::ulog::info!("editor runtime: mode set to Edit via action route; simulation stopped and viewport remains preview-only"),
                UiEditorRuntimeMode::Simulate => newengine_ulog_api::ulog::info!("editor runtime: mode set to Simulate via action route; world simulation may run without direct player control"),
                UiEditorRuntimeMode::Play => newengine_ulog_api::ulog::info!("editor runtime: mode set to Play in Editor via action route; gameplay input may be handed to viewport policy"),
            }
        }

        resources.insert(UiEditorRuntimeState {
            version: 1,
            frame_index,
            mode: self.editor_runtime_mode,
            source_surface: self.descriptor.surface_id.clone(),
            reason: match self.editor_runtime_mode {
                UiEditorRuntimeMode::Edit => {
                    "editor boot default: simulation stopped until Simulate or Play".to_owned()
                }
                UiEditorRuntimeMode::Simulate => {
                    "toolbar/shortcut requested simulation preview".to_owned()
                }
                UiEditorRuntimeMode::Play => "toolbar/shortcut requested play in editor".to_owned(),
            },
        });
    }

    fn update_dock_interaction(&mut self, resources: &Resources, frame_index: u64) {
        if self.descriptor.profile != UiScreenProfile::Editor
            || self.last_dock_click_frame == frame_index
        {
            return;
        }
        let Some(action_id) = clicked_dispatch_action(resources, "editor.dock.toggle.") else {
            return;
        };
        let slot_id = action_id.trim_start_matches("editor.dock.toggle.");
        if !self
            .descriptor
            .panels
            .iter()
            .any(|panel| panel.slot_id == slot_id)
        {
            return;
        }
        self.last_dock_click_frame = frame_index;
        if self.hidden_panels.contains(slot_id) {
            self.hidden_panels.remove(slot_id);
            newengine_ulog_api::ulog::info!(
                "editor dock: panel '{}' shown via ui.dispatch_input_v1",
                slot_id
            );
        } else {
            self.hidden_panels.insert(slot_id.to_owned());
            newengine_ulog_api::ulog::info!(
                "editor dock: panel '{}' hidden via ui.dispatch_input_v1",
                slot_id
            );
        }
    }

    fn publish_editor_layout_state(&self, resources: &mut Resources, frame_index: u64) {
        if self.descriptor.profile != UiScreenProfile::Editor {
            resources.insert(UiDockLayoutState {
                version: 1,
                frame_index,
                panels: Vec::new(),
            });
            resources.insert(UiViewportSlot::default());
            return;
        }
        let layout = editor_layout_metrics(resources, &self.hidden_panels);
        resources.insert(UiViewportSlot {
            version: 1,
            frame_index,
            surface_id: DEFAULT_VIEWPORT_SURFACE.to_owned(),
            x_px: layout.viewport_x,
            y_px: layout.viewport_y,
            w_px: layout.viewport_w,
            h_px: layout.viewport_h,
            input_enabled: self.editor_runtime_mode != UiEditorRuntimeMode::Edit,
            simulation_enabled: self.editor_runtime_mode != UiEditorRuntimeMode::Edit,
            runtime_mode: self.editor_runtime_mode,
        });
        resources.insert(UiDockLayoutState {
            version: 1,
            frame_index,
            panels: vec![
                dock_state(
                    "left.scene_tree",
                    layout.left_visible,
                    false,
                    layout.hovered_dock_slot == Some("left.scene_tree"),
                ),
                dock_state(
                    "right.inspector",
                    layout.right_visible,
                    false,
                    layout.hovered_dock_slot == Some("right.inspector"),
                ),
                dock_state(
                    "bottom.asset_browser",
                    layout.bottom_visible,
                    false,
                    layout.hovered_dock_slot == Some("bottom.asset_browser"),
                ),
                dock_state(
                    "bottom.import_queue",
                    layout.bottom_visible,
                    false,
                    layout.hovered_dock_slot == Some("bottom.import_queue"),
                ),
                dock_state(
                    "bottom.output_log",
                    layout.bottom_visible,
                    false,
                    layout.hovered_dock_slot == Some("bottom.output_log"),
                ),
                dock_state(
                    "bottom.profiler_diagnostics",
                    layout.bottom_visible,
                    false,
                    layout.hovered_dock_slot == Some("bottom.profiler_diagnostics"),
                ),
                dock_state("center.viewport_gizmos", true, false, false),
            ],
        });
        resources.insert(UiToastStack {
            version: 1,
            frame_index,
            notifications: vec![UiToastNotification {
                id: "editor.boot.mode".to_owned(),
                title: match self.editor_runtime_mode {
                    UiEditorRuntimeMode::Edit => "Editor ready".to_owned(),
                    UiEditorRuntimeMode::Simulate => "Simulation running".to_owned(),
                    UiEditorRuntimeMode::Play => "Play In Editor".to_owned(),
                },
                detail: match self.editor_runtime_mode {
                    UiEditorRuntimeMode::Edit => "World/game bootstrap is deferred; viewport is a preview slot until Simulate or Play.".to_owned(),
                    UiEditorRuntimeMode::Simulate => "Scene bootstrap may run; player possession stays disabled.".to_owned(),
                    UiEditorRuntimeMode::Play => "Scene bootstrap may run and viewport can receive gameplay input.".to_owned(),
                },
                progress_permille: if self.editor_runtime_mode == UiEditorRuntimeMode::Edit { Some(1000) } else { None },
                severity: UiToastSeverity::Info,
                source: SCREEN_PROFILE_SOURCE.to_owned(),
            }],
        });
    }

    fn append_toast_components(
        &self,
        resources: &Resources,
        node: &mut UiSurfaceNode,
        layout: &EditorLayoutMetrics,
    ) {
        let Some(stack) = resources.get::<UiToastStack>() else {
            return;
        };
        let toast_w = 320.0_f32.min((layout.screen_w * 0.32).max(220.0));
        let toast_x = (layout.screen_w - toast_w - 16.0).max(8.0);
        let mut toast_y = layout.menu_h + layout.toolbar_h + 12.0;
        for toast in stack.notifications.iter().take(4) {
            let mut component = UiComponentNode::row(
                format!("editor.toast.{}", component_id_fragment(&toast.id)),
                toast.title.clone(),
            )
            .with_detail(toast.detail.clone())
            .with_tone(match toast.severity {
                UiToastSeverity::Info | UiToastSeverity::Success => UiNodeTone::Accent,
                UiToastSeverity::Warning | UiToastSeverity::Error => UiNodeTone::Danger,
            })
            .tagged("toast")
            .tagged("notification");
            if let Some(progress) = toast.progress_permille {
                component.value = Some(format!("{}%", progress as f32 / 10.0));
            }
            node.components
                .push(with_rect(component, toast_x, toast_y, toast_w, 34.0));
            toast_y += 40.0;
        }
    }

    fn publish_focus_policy(&self, resources: &mut Resources) {
        match self.active_input_focus_policy() {
            UiScreenInputFocusPolicy::EditorShell => {
                let mut capture = UiInputCaptureState::none();
                capture.gameplay_movement_gated = true;
                capture.draw_refresh_requested = true;
                capture.reason = SCREEN_PROFILE_CAPTURE_REASON.to_owned();
                capture.surfaces = vec![self.descriptor.surface_id.clone()];
                set_input_capture_contribution(resources, SCREEN_PROFILE_CAPTURE_OWNER, capture);
            }
            UiScreenInputFocusPolicy::UiSurface => {
                let mut capture = UiInputCaptureState::none();
                capture.gameplay_movement_gated = self
                    .active_presentation_state()
                    .map(|state| state.blocks_gameplay_input)
                    .unwrap_or(true);
                capture.draw_refresh_requested = true;
                capture.reason = "screen_profile.presentation_flow".to_owned();
                capture.surfaces = self
                    .active_presentation_state()
                    .and_then(|state| state.surface_id.clone())
                    .into_iter()
                    .collect();
                set_input_capture_contribution(resources, SCREEN_PROFILE_CAPTURE_OWNER, capture);
            }
            UiScreenInputFocusPolicy::GameViewport | UiScreenInputFocusPolicy::Headless => {
                remove_input_capture_contribution(resources, SCREEN_PROFILE_CAPTURE_OWNER, None);
            }
        }
    }

    fn hide_profile_surface(&mut self, surface_id: &str, force: bool) -> bool {
        if force || self.published_surfaces.contains(surface_id) {
            publish_screen_surface_node(&UiSurfaceNode::hidden(
                surface_id,
                "engine.ui.screen_profile",
            ));
            self.published_surfaces.remove(surface_id);
            true
        } else {
            false
        }
    }

    fn append_right_edit_window(
        &mut self,
        resources: &Resources,
        node: &mut UiSurfaceNode,
        layout: &EditorLayoutMetrics,
    ) {
        let selection = resources
            .get::<EditorSelectionContext>()
            .cloned()
            .unwrap_or_else(EditorSelectionContext::none);
        self.refresh_right_edit_cache(&selection);
        let first_right_component = node.components.len();
        node.components.push(
            UiComponentNode::row("right_edit_window.header", "Right Edit Window")
                .with_value(selection.kind.as_str())
                .with_detail(if selection.reference.is_empty() {
                    "no active editor selection".to_owned()
                } else {
                    selection.reference.clone()
                })
                .with_tone(if selection.kind == EditorSelectionKind::None {
                    UiNodeTone::Normal
                } else {
                    UiNodeTone::Accent
                })
                .tagged("right")
                .tagged("edit-window")
                .tagged("selection-context"),
        );

        match selection.kind {
            EditorSelectionKind::None => {
                node.components.push(
                    UiComponentNode::row("right_edit_window.empty", "No selection")
                        .with_detail(
                            "viewport/outliner/content browser can publish EditorSelectionContext",
                        )
                        .with_tone(UiNodeTone::Normal)
                        .tagged("right")
                        .tagged("edit-window"),
                );
            }
            EditorSelectionKind::Entity => {
                node.components.push(
                    UiComponentNode::row("right_edit_window.entity.route", "Entity Component Editor")
                        .with_value(format!("{} + engine.entity", ENGINE_SCHEMA_SERVICE_ID))
                        .with_detail("component properties must come from schema.describe_properties_v1; native EntityId must not cross this boundary")
                        .with_tone(UiNodeTone::Accent)
                        .tagged("right")
                        .tagged("entity")
                        .tagged("opaque-handles"),
                );
            }
            EditorSelectionKind::Asset
            | EditorSelectionKind::AssetEntry
            | EditorSelectionKind::Material => {
                self.push_asset_document_components(node, &selection);
            }
            EditorSelectionKind::World => {
                node.components.push(
                    UiComponentNode::row("right_edit_window.world.route", "World Settings Editor")
                        .with_value(format!("{} + engine.world", ENGINE_SCHEMA_SERVICE_ID))
                        .with_detail("settings editor consumes schema properties and emits transaction DTO patches")
                        .with_tone(UiNodeTone::Accent)
                        .tagged("right")
                        .tagged("world"),
                );
            }
        }

        let right_x = layout.screen_w - layout.right_w + 10.0;
        let right_w = (layout.right_w - 28.0).max(160.0);
        let mut y = layout.viewport_y + 84.0;
        for component in node.components.iter_mut().skip(first_right_component) {
            let h = if component.state_tags.iter().any(|tag| tag == "asset-field") {
                24.0
            } else {
                34.0
            };
            set_rect(component, right_x, y, right_w, h);
            y += h + 5.0;
            if y > layout.bottom_y - 12.0 {
                break;
            }
        }
    }

    fn refresh_right_edit_cache(&mut self, selection: &EditorSelectionContext) {
        let key = format!("{}:{}", selection.kind.as_str(), selection.reference);
        if self.last_right_edit_selection_key == key {
            return;
        }
        self.last_right_edit_selection_key = key;
        self.cached_right_edit_document = None;
        self.cached_right_edit_error = None;

        if !matches!(
            selection.kind,
            EditorSelectionKind::Asset
                | EditorSelectionKind::AssetEntry
                | EditorSelectionKind::Material
        ) {
            return;
        }
        if selection.reference.trim().is_empty() {
            self.cached_right_edit_error = Some("empty asset selection reference".to_owned());
            return;
        }

        let client = AssetServiceClient::new(newengine_plugin_host::default_host_api());
        match client.inspect_document_json_v1(AssetDocumentRequest {
            asset_ref: selection.reference.clone(),
            requester: RIGHT_EDIT_WINDOW_OWNER.to_owned(),
            ..AssetDocumentRequest::default()
        }) {
            Ok(document) => self.cached_right_edit_document = Some(document),
            Err(error) => self.cached_right_edit_error = Some(error),
        }
    }

    fn push_asset_document_components(
        &self,
        node: &mut UiSurfaceNode,
        selection: &EditorSelectionContext,
    ) {
        node.components.push(
            UiComponentNode::row("right_edit_window.asset.route", "Asset Document Editor")
                .with_value("engine.assets.inspect")
                .with_detail(format!(
                    "source={} semantic_gateway={}",
                    selection.source_surface, selection.semantic_gateway
                ))
                .with_tone(UiNodeTone::Accent)
                .tagged("right")
                .tagged("asset-document"),
        );

        if let Some(error) = self.cached_right_edit_error.as_ref() {
            node.components.push(
                UiComponentNode::row("right_edit_window.asset.error", "AssetDocument unavailable")
                    .with_value(error.clone())
                    .with_tone(UiNodeTone::Danger)
                    .tagged("right")
                    .tagged("diagnostic"),
            );
            return;
        }

        let Some(document) = self.cached_right_edit_document.as_ref() else {
            node.components.push(
                UiComponentNode::row("right_edit_window.asset.pending", "No AssetDocument DTO")
                    .with_detail("provider route missing or selection was not an asset")
                    .with_tone(UiNodeTone::Normal)
                    .tagged("right")
                    .tagged("asset-document"),
            );
            return;
        };

        node.components.push(
            UiComponentNode::row("right_edit_window.asset.header", document.title.clone())
                .with_value(document.document_kind.clone())
                .with_detail(format!(
                    "schema_editable={} can_apply_patch={} writer={}",
                    document.editable_fields_available,
                    document.can_apply_patch,
                    if document.writer_capability.is_empty() {
                        "missing"
                    } else {
                        document.writer_capability.as_str()
                    }
                ))
                .with_tone(if document.can_apply_patch {
                    UiNodeTone::Accent
                } else {
                    UiNodeTone::Normal
                })
                .tagged("right")
                .tagged("asset-document"),
        );
        node.components.push(
            UiComponentNode::row("right_edit_window.asset.contract", "Contract")
                .with_value(document.inspect_contract.clone())
                .with_detail(format!(
                    "edit_contract={} write_owner={}",
                    if document.edit_contract.is_empty() {
                        "none"
                    } else {
                        document.edit_contract.as_str()
                    },
                    document.write_owner
                ))
                .with_tone(UiNodeTone::Normal)
                .tagged("right")
                .tagged("asset-document"),
        );
        if let Some(schema_type) = document.schema_type.as_ref() {
            node.components.push(
                UiComponentNode::row("right_edit_window.asset.schema", "Schema Type")
                    .with_value(schema_type.type_id.clone())
                    .with_detail(format!(
                        "route={} contract={} properties={}",
                        ENGINE_SCHEMA_SERVICE_ID,
                        document.schema_contract,
                        schema_type.properties.len()
                    ))
                    .with_tone(UiNodeTone::Accent)
                    .tagged("right")
                    .tagged("asset-document")
                    .tagged("schema"),
            );
        }

        for section in document.sections.iter().take(3) {
            node.components.push(
                UiComponentNode::row(
                    format!(
                        "right_edit_window.asset.section.{}",
                        component_id_fragment(&section.id)
                    ),
                    section.title.clone(),
                )
                .with_value(format!("{} fields", section.fields.len()))
                .with_tone(UiNodeTone::Accent)
                .tagged("right")
                .tagged("asset-section"),
            );
            for field in section.fields.iter().take(4) {
                node.components.push(
                    UiComponentNode::row(
                        format!(
                            "right_edit_window.asset.field.{}.{}",
                            component_id_fragment(&section.id),
                            component_id_fragment(&field.id)
                        ),
                        field.label.clone(),
                    )
                    .with_value(asset_document_value_label(&field.value))
                    .with_detail(asset_document_field_detail(field))
                    .with_tone(if field.editable {
                        UiNodeTone::Accent
                    } else {
                        UiNodeTone::Normal
                    })
                    .tagged("right")
                    .tagged("asset-field")
                    .tagged("schema-property"),
                );
            }
        }
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
