use super::*;

impl ScreenProfileRuntimeState {
    pub(super) fn active_presentation_state(&self) -> Option<&ScreenPresentationStateConfig> {
        let flow = self
            .config
            .presentation_flow
            .as_ref()?
            .is_valid()
            .then_some(self.config.presentation_flow.as_ref()?)?;
        flow.state(self.presentation_state_id.as_deref()?)
    }

    pub(super) fn presentation_flow_state(
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

    pub(super) fn active_input_focus_policy(&self) -> UiScreenInputFocusPolicy {
        self.active_presentation_state()
            .map(|state| state.input_focus_policy)
            .unwrap_or(self.descriptor.input_focus_policy)
    }

    pub(super) fn update_presentation_flow(
        &mut self,
        resources: &Resources,
        frame_index: u64,
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

    pub(super) fn prepare_presentation_flow_surface(
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

    pub(super) fn publish_focus_policy(&self, resources: &mut Resources) {
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

    pub(super) fn hide_profile_surface(&mut self, surface_id: &str, force: bool) -> bool {
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
}
