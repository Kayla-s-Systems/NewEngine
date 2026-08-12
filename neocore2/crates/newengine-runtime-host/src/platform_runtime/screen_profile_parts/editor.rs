use super::*;

impl ScreenProfileRuntimeState {
    pub(super) fn update_menu_interaction(&mut self, resources: &Resources, frame_index: u64) {
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

    pub(super) fn update_editor_runtime_state(
        &mut self,
        resources: &mut Resources,
        frame_index: u64,
    ) {
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

    pub(super) fn update_dock_interaction(&mut self, resources: &Resources, frame_index: u64) {
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

    pub(super) fn publish_editor_layout_state(&self, resources: &mut Resources, frame_index: u64) {
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

    pub(super) fn append_toast_components(
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
}
