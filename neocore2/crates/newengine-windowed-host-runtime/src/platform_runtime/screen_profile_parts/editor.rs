use super::*;

impl ScreenProfileRuntimeState {
    pub(super) fn update_menu_interaction(&mut self, resources: &Resources, frame_index: u64) {
        if !editing_tools_available(resources)
            || !in_game_editor_active(resources)
            || self.last_menu_click_frame == frame_index
        {
            return;
        }
        if clicked_dispatch_action(resources, "editor.runtime.more").is_some() {
            self.last_menu_click_frame = frame_index;
            if self.active_menu_id.as_deref() == Some("__runtime_more") {
                self.active_menu_id = None;
            } else {
                self.active_menu_id = Some("__runtime_more".to_owned());
            }
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

    pub(super) fn update_editor_runtime_state(
        &mut self,
        resources: &mut Resources,
        frame_index: u64,
    ) {
        install_runtime_session_resources(resources);
        for command in drain_external_runtime_session_commands() {
            submit_runtime_session_command(
                resources,
                frame_index,
                RUNTIME_SESSION_COMMAND_SOURCE_CONSOLE,
                command,
            );
        }

        let needs_play_session = resources
            .get::<RuntimeSessionState>()
            .map(|state| state.mode != Some(RuntimeSessionMode::Play) || !state.is_active())
            .unwrap_or(true);
        if needs_play_session {
            submit_runtime_session_command(
                resources,
                frame_index,
                RUNTIME_SESSION_COMMAND_SOURCE_GAME,
                RuntimeSessionCommand::Start {
                    mode: RuntimeSessionMode::Play,
                },
            );
        }

        if !editing_tools_available(resources) {
            let session = advance_runtime_session(resources, frame_index);
            resources.insert(UiEditorRuntimeState {
                version: 1,
                frame_index,
                mode: UiEditorRuntimeMode::Play,
                paused: session.paused,
                source_surface: self.descriptor.surface_id.clone(),
                reason: session.last_reason,
            });
            return;
        }

        if self.last_runtime_command_frame != frame_index {
            let current = resources
                .get::<RuntimeSessionState>()
                .cloned()
                .unwrap_or_default();
            let context = editor_command_context(&current);
            let registry = resources
                .get::<EditorCommandRegistry>()
                .cloned()
                .unwrap_or_else(default_runtime_editor_commands);

            let clicked_command =
                clicked_dispatch_action(resources, "editor.runtime.").filter(|command_id| {
                    registry
                        .get(command_id)
                        .is_some_and(|command| command.enabled(context))
                });
            let shortcut_command = resources
                .get::<UiInputFrame>()
                .and_then(|input| registry.resolve_pressed(input, context))
                .map(|command| command.id.clone());
            let requested_command = clicked_command.or(shortcut_command);

            if let Some(command_id) = requested_command {
                self.last_runtime_command_frame = frame_index;
                if self.active_menu_id.as_deref() == Some("__runtime_more") {
                    self.active_menu_id = None;
                }
                if let Some(command) = runtime_session_command_from_editor_command(&command_id) {
                    submit_runtime_session_command(
                        resources,
                        frame_index,
                        RUNTIME_SESSION_COMMAND_SOURCE_EDITOR,
                        command,
                    );
                    newengine_ulog_api::ulog::info!(
                        "editing tools command: submitted '{}' to live runtime-session controller from session={} phase={:?}",
                        command_id,
                        current.session_id.0,
                        current.phase,
                    );
                }
            }
        }

        let session = advance_runtime_session(resources, frame_index);
        let (mode, paused) = editor_runtime_projection(&session);
        resources.insert(UiEditorRuntimeState {
            version: 1,
            frame_index,
            mode,
            paused,
            source_surface: self.descriptor.surface_id.clone(),
            reason: session.last_reason.clone(),
        });
    }

    pub(super) fn update_editor_viewport_interaction(
        &mut self,
        resources: &mut Resources,
        frame_index: u64,
    ) {
        if !editing_tools_available(resources) || !in_game_editor_active(resources) {
            return;
        }

        let mut state = resources
            .get::<UiEditorViewportState>()
            .cloned()
            .unwrap_or_default();
        state.frame_index = frame_index;
        let mut changed = false;

        if let Some(action) = clicked_dispatch_action(resources, "editor.viewport.") {
            match action.as_str() {
                "editor.viewport.projection" => {
                    state.projection = match state.projection {
                        UiEditorViewportProjection::Perspective => UiEditorViewportProjection::Top,
                        UiEditorViewportProjection::Top => UiEditorViewportProjection::Front,
                        UiEditorViewportProjection::Front => UiEditorViewportProjection::Side,
                        UiEditorViewportProjection::Side => UiEditorViewportProjection::Perspective,
                    };
                    self.active_menu_id = None;
                    changed = true;
                }
                "editor.viewport.shading" => {
                    state.shading = match state.shading {
                        UiEditorViewportShading::Lit => UiEditorViewportShading::Unlit,
                        UiEditorViewportShading::Unlit => UiEditorViewportShading::Wireframe,
                        UiEditorViewportShading::Wireframe => UiEditorViewportShading::Lit,
                    };
                    self.active_menu_id = None;
                    changed = true;
                }
                "editor.viewport.show" => {
                    self.active_menu_id =
                        if self.active_menu_id.as_deref() == Some("__viewport_show") {
                            None
                        } else {
                            Some("__viewport_show".to_owned())
                        };
                }
                "editor.viewport.show.grid" => {
                    state.show_grid = !state.show_grid;
                    changed = true;
                }
                "editor.viewport.show.collision" => {
                    state.show_collision = !state.show_collision;
                    changed = true;
                }
                "editor.viewport.show.bounds" => {
                    state.show_bounds = !state.show_bounds;
                    changed = true;
                }
                "editor.viewport.show.gizmos" => {
                    state.gizmo_visible = !state.gizmo_visible;
                    changed = true;
                }
                "editor.viewport.transform.select" => {
                    state.transform_mode = UiEditorTransformMode::Select;
                    self.active_menu_id = None;
                    changed = true;
                }
                "editor.viewport.transform.translate" => {
                    state.transform_mode = UiEditorTransformMode::Translate;
                    self.active_menu_id = None;
                    changed = true;
                }
                "editor.viewport.transform.rotate" => {
                    state.transform_mode = UiEditorTransformMode::Rotate;
                    self.active_menu_id = None;
                    changed = true;
                }
                "editor.viewport.transform.scale" => {
                    state.transform_mode = UiEditorTransformMode::Scale;
                    self.active_menu_id = None;
                    changed = true;
                }
                "editor.viewport.transform.space" => {
                    state.transform_space = match state.transform_space {
                        UiEditorTransformSpace::World => UiEditorTransformSpace::Local,
                        UiEditorTransformSpace::Local => UiEditorTransformSpace::World,
                    };
                    changed = true;
                }
                "editor.viewport.snap.translate.toggle" => {
                    state.translation_snap_enabled = !state.translation_snap_enabled;
                    changed = true;
                }
                "editor.viewport.snap.translate.value" => {
                    state.translation_snap_units = next_editor_snap_value(
                        state.translation_snap_units,
                        &[1.0, 5.0, 10.0, 50.0, 100.0],
                    );
                    changed = true;
                }
                "editor.viewport.snap.rotate.toggle" => {
                    state.rotation_snap_enabled = !state.rotation_snap_enabled;
                    changed = true;
                }
                "editor.viewport.snap.rotate.value" => {
                    state.rotation_snap_degrees = next_editor_snap_value(
                        state.rotation_snap_degrees,
                        &[5.0, 10.0, 15.0, 30.0, 45.0, 90.0],
                    );
                    changed = true;
                }
                "editor.viewport.snap.scale.toggle" => {
                    state.scale_snap_enabled = !state.scale_snap_enabled;
                    changed = true;
                }
                "editor.viewport.snap.scale.value" => {
                    state.scale_snap_percent = next_editor_snap_value(
                        state.scale_snap_percent,
                        &[1.0, 5.0, 10.0, 25.0, 50.0],
                    );
                    changed = true;
                }
                _ => {}
            }
        }

        let edit_mode = in_game_editor_active(resources)
            || resources
                .get::<RuntimeSessionState>()
                .map(|session| !session.is_active())
                .unwrap_or(true);
        if edit_mode {
            if let Some(input) = resources.get::<UiInputFrame>() {
                let text_input_active = !input.text.is_empty()
                    || !input.text_edit_ops.is_empty()
                    || !input.ime_preedit.is_empty();
                let fly_navigation_active =
                    input.is_mouse_down(newengine_input_api::mouse_button::RIGHT);
                if !text_input_active && !fly_navigation_active {
                    use newengine_input_api::key_code::{KEY_E, KEY_Q, KEY_R, KEY_W};
                    let shortcut_mode = if input.is_key_pressed(KEY_Q) {
                        Some(UiEditorTransformMode::Select)
                    } else if input.is_key_pressed(KEY_W) {
                        Some(UiEditorTransformMode::Translate)
                    } else if input.is_key_pressed(KEY_E) {
                        Some(UiEditorTransformMode::Rotate)
                    } else if input.is_key_pressed(KEY_R) {
                        Some(UiEditorTransformMode::Scale)
                    } else {
                        None
                    };
                    if let Some(mode) = shortcut_mode {
                        state.transform_mode = mode;
                        self.active_menu_id = None;
                        changed = true;
                    }
                }
            }
        }

        if changed {
            newengine_ulog_api::ulog::info!(
                "editor viewport: projection={} shading={} transform={} grid={} collision={} bounds={} gizmo={}",
                state.projection.label(),
                state.shading.label(),
                state.transform_mode.label(),
                state.show_grid,
                state.show_collision,
                state.show_bounds,
                state.gizmo_visible,
            );
        }
        resources.insert(state);
    }

    pub(super) fn update_dock_interaction(&mut self, resources: &Resources, frame_index: u64) {
        if !editing_tools_available(resources)
            || !in_game_editor_active(resources)
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

    pub(super) fn publish_editor_layout_state(&self, resources: &mut Resources, frame_index: u64) {
        if !editing_tools_available(resources)
            || !in_game_editor_active(resources)
            || !self.config.publish_editor_shell
        {
            resources.insert(UiDockLayoutState {
                version: 1,
                frame_index,
                panels: Vec::new(),
            });
            resources.insert(UiViewportSlot::default());
            return;
        }
        let layout = editor_layout_metrics(resources, &self.hidden_panels);
        let runtime_mode = UiEditorRuntimeMode::Edit;
        resources.insert(UiViewportSlot {
            version: 1,
            frame_index,
            surface_id: DEFAULT_VIEWPORT_SURFACE.to_owned(),
            x_px: layout.viewport_x,
            y_px: layout.viewport_y,
            w_px: layout.viewport_w,
            h_px: layout.viewport_h,
            // Editor viewport input remains live while simulation is paused: RMB+WASD
            // is camera navigation and never possession/gameplay movement.
            input_enabled: true,
            simulation_enabled: false,
            paused: true,
            runtime_mode,
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
                    "bottom.script_editor",
                    layout.bottom_visible && !self.hidden_panels.contains("bottom.script_editor"),
                    false,
                    layout.hovered_dock_slot == Some("bottom.script_editor"),
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
    }

    pub(super) fn prepare_editing_overlay(
        &mut self,
        resources: &Resources,
        frame_index: u64,
        profile_changed: bool,
    ) -> bool {
        if !self.config.publish_editor_shell
            || !editing_tools_available(resources)
            || !in_game_editor_active(resources)
        {
            return self.hide_profile_surface(UI_SURFACE_EDITOR_SHELL, profile_changed);
        }

        let layout = editor_layout_metrics(resources, &self.hidden_panels);
        let runtime_mode = UiEditorRuntimeMode::Edit;
        let runtime_paused = true;
        let authoring_state = resources
            .get::<UiInGameEditorState>()
            .cloned()
            .unwrap_or_default();
        let viewport_state = resources
            .get::<UiEditorViewportState>()
            .cloned()
            .unwrap_or_default();
        let scene_snapshot = resources
            .get::<UiEditorSceneSnapshot>()
            .cloned()
            .unwrap_or_default();
        let inspector_snapshot = resources
            .get::<UiEditorInspectorSnapshot>()
            .cloned()
            .unwrap_or_default();
        let mut node = EditorScreen::default().surface_node(
            frame_index,
            runtime_mode,
            runtime_paused,
            &viewport_state,
            &scene_snapshot,
            &inspector_snapshot,
            &authoring_state,
            &layout,
            self.active_menu_id.as_deref(),
        );
        let asset_document_selected =
            resources
                .get::<EditorSelectionContext>()
                .is_some_and(|selection| {
                    matches!(
                        selection.kind,
                        EditorSelectionKind::Asset
                            | EditorSelectionKind::AssetEntry
                            | EditorSelectionKind::Material
                    )
                });
        if asset_document_selected {
            self.append_right_edit_window(resources, &mut node, &layout);
        }
        self.append_script_editor_panel(&mut node, &layout);
        sort_components_by_layout_y(&mut node.components);
        publish_screen_node_tree_request(&UiNodeTreeRequest::from_surface_node(
            &node,
            UiNodeRequestSourceKind::Generated,
        ));
        self.published_surfaces
            .insert(UI_SURFACE_EDITOR_SHELL.to_owned());
        true
    }

    pub(super) fn append_toast_components(
        &self,
        resources: &Resources,
        node: &mut UiSurfaceNode,
        layout: &EditorLayoutMetrics,
    ) {
        let Some(stack) = resources.get::<UiToastStack>() else {
            return;
        };
        let toast_w = 360.0_f32.min((layout.screen_w * 0.34).max(260.0));
        let toast_x = 0.0;
        let mut toast_y = 0.0;
        for toast in stack.notifications.iter().take(4) {
            let mut component = UiComponentNode::row(
                format!(
                    "engine.ui.notify.toast.{}",
                    component_id_fragment(&toast.id)
                ),
                toast.title.clone(),
            )
            .with_detail(toast.detail.clone())
            .with_tone(match toast.severity {
                UiToastSeverity::Info | UiToastSeverity::Success => UiNodeTone::Accent,
                UiToastSeverity::Warning | UiToastSeverity::Error => UiNodeTone::Danger,
            })
            .tagged("toast")
            .tagged("notification")
            .tagged(match toast.severity {
                UiToastSeverity::Info => "info",
                UiToastSeverity::Success => "success",
                UiToastSeverity::Warning => "warning",
                UiToastSeverity::Error => "error",
            });
            if let Some(progress) = toast.progress_permille {
                component.value = Some(format!("{}%", progress as f32 / 10.0));
            }
            node.components
                .push(with_rect(component, toast_x, toast_y, toast_w, 52.0));
            toast_y += 60.0;
        }
    }
}

fn next_editor_snap_value(current: f32, values: &[f32]) -> f32 {
    let index = values
        .iter()
        .position(|value| (*value - current).abs() < f32::EPSILON)
        .unwrap_or(0);
    values[(index + 1) % values.len()]
}
