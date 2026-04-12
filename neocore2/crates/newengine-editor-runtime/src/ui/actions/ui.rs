#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::EntityId;
use newengine_editor_core::ToolId;
use newengine_materials::{MaterialId, MaterialRef};
use newengine_primitives::Primitive;
use newengine_transform_api::Transform;

use crate::scene_bridge::PrimitiveMaterialBase;
use crate::ui::commands::TypedEditorCommand;
use crate::ui::{dock, schema, PendingAssetSpawn};
use crate::ui::{providers, EditorUiBuild};

impl EditorUiBuild {
    #[inline]
    pub(crate) fn spawn_asset_contract_near_camera(
        &mut self,
        contract: &schema::AssetSpawnContract,
        source: &'static str,
    ) {
        let (cam_pos, cam_fwd) = self.viewport_bridge.read_camera_spawn();
        let mut position = cam_pos + cam_fwd * 3.0;
        position.y = position.y.max(0.5);
        self.scene_bridge.cmd_spawn_imported_asset(
            schema::to_scene_import_descriptor(contract),
            contract.actor_name.clone(),
            position,
        );
        log::info!(
            "asset import contract accepted source='{}' class='{}' path='{}'",
            source,
            contract.import.class.label(),
            contract.logical_path
        );
    }

    #[inline]
    pub(crate) fn execute_ui_action(&mut self, action: &providers::UiAction) {
        self.command_bus
            .push(TypedEditorCommand::UiAction(action.clone()));
    }

    pub(crate) fn dispatch_ui_action(&mut self, action: &providers::UiAction) {
        match action {
            providers::UiAction::NewScene => {
                self.scene_bridge.cmd_new_scene();
                self.editor.commands.clear();
                self.editor.selection.clear();
            }
            providers::UiAction::OpenScene(mode) => {
                if self.scene_io.is_some() {
                    self.scene_io_ui.open = true;
                    self.scene_io_ui.mode = *mode;
                }
            }
            providers::UiAction::OpenAssetManager => {
                if self.assets.is_some() {
                    self.asset_ui.open = true;
                }
            }
            providers::UiAction::OpenCommandPalette => self.open_command_palette(),
            providers::UiAction::QuitStub => {
                log::warn!("Quit: not implemented yet (need shutdown token in UI)");
            }
            providers::UiAction::Undo => {
                if let Some(cmd) = self.editor.commands.pop_undo() {
                    self.apply_editor_command_undo(cmd);
                }
            }
            providers::UiAction::Redo => {
                if let Some(cmd) = self.editor.commands.pop_redo() {
                    self.apply_editor_command_redo(cmd);
                }
            }
            providers::UiAction::Deselect => {
                self.editor.selection.clear();
                self.scene_bridge.set_selection(None);
            }
            providers::UiAction::ToggleConsole => {
                self.console_open = !self.console_open;
                if self.console_open {
                    self.ensure_dock_tab_open(dock::EditorDockTab::Console);
                }
            }
            providers::UiAction::TogglePlugins => {
                if let Ok(mut plugin_manager) = self.plugin_manager.lock() {
                    plugin_manager.toggle();
                }
            }
            providers::UiAction::FrameSelection => {
                self.viewport_bridge.publish_frame_request(false);
            }
            providers::UiAction::FrameAll => {
                self.viewport_bridge.publish_frame_request(true);
            }
            providers::UiAction::SetWorkspacePreset(preset) => {
                self.apply_workspace_preset(*preset);
            }
            providers::UiAction::SetViewportMode(mode) => {
                self.set_viewport_mode(*mode);
            }
            providers::UiAction::SetTool(tool) => self.set_active_tool(*tool),
            providers::UiAction::SetPlayMode(mode) => {
                let current = self.scene_bridge.play_mode();
                let next = if current == *mode {
                    crate::gameplay::EditorPlayMode::Edit
                } else {
                    *mode
                };
                self.scene_bridge.cmd_set_play_mode(next);
            }
            providers::UiAction::StopRuntime => {
                self.scene_bridge
                    .cmd_set_play_mode(crate::gameplay::EditorPlayMode::Edit);
            }
            providers::UiAction::OpenDockTab(tab) => {
                self.ensure_dock_tab_open(*tab);
            }
            providers::UiAction::CameraSpeedUp => self.camera_speed.step_up(),
            providers::UiAction::CameraSpeedDown => self.camera_speed.step_down(),
            providers::UiAction::SetCameraSpeedPreset(index) => {
                self.camera_speed.preset_index = *index;
                self.camera_speed.clamp_preset_index();
            }
            providers::UiAction::SpawnPlayer => {
                let (cam_pos, cam_fwd) = self.viewport_bridge.read_camera_spawn();
                let mut position = cam_pos + cam_fwd * 3.0;
                position.y = position.y.max(0.5);
                self.scene_bridge
                    .cmd_spawn_player("Player".to_string(), position);
            }
            providers::UiAction::SpawnPrimitive { id, name } => {
                let (cam_pos, cam_fwd) = self.viewport_bridge.read_camera_spawn();
                let mut position = cam_pos + cam_fwd * 3.0;
                position.y = position.y.max(0.5);
                self.scene_bridge
                    .cmd_spawn_primitive(*id, name.clone(), position);
            }
            providers::UiAction::SpawnDirectionalLight => {
                self.scene_bridge.cmd_spawn_directional_light(
                    "Sun".to_string(),
                    newengine_math::Vec3::new(0.0, 6.0, 0.0),
                    newengine_math::Vec3::new(-0.35, -1.0, -0.25),
                );
            }
            providers::UiAction::SpawnPointLight => {
                self.scene_bridge.cmd_spawn_point_light(
                    "PointLight".to_string(),
                    newengine_math::Vec3::new(0.0, 2.0, 0.0),
                );
            }
            providers::UiAction::TogglePanel(id) => self.toggle_panel(*id),
            providers::UiAction::AddCollisionToSelection => {
                self.execute_context_action(schema::ContextActionId::AddCollision);
            }
            providers::UiAction::RemoveCollisionFromSelection => {
                self.execute_context_action(schema::ContextActionId::RemoveCollision);
            }
            providers::UiAction::SpawnPendingAsset => {
                self.execute_context_action(schema::ContextActionId::SpawnAssetHere);
            }
            providers::UiAction::ToggleCollisionOverlay => {
                self.execute_context_action(schema::ContextActionId::ToggleCollisionOverlay);
            }
        }
    }

    #[inline]
    pub(crate) fn open_command_palette(&mut self) {
        self.command_palette.open = true;
        self.command_palette.selected_index = 0;
    }

    #[inline]
    pub(crate) fn set_active_tool(&mut self, tool: ToolId) {
        self.editor.active_tool = tool;
        match tool {
            ToolId::Select => {}
            ToolId::Translate => {
                self.gizmo.set_mode(newengine_gizmo::GizmoMode::Translate);
                self.editor.gizmo_mode = newengine_editor_core::GizmoMode::Translate;
            }
            ToolId::Rotate => {
                self.gizmo.set_mode(newengine_gizmo::GizmoMode::Rotate);
                self.editor.gizmo_mode = newengine_editor_core::GizmoMode::Rotate;
            }
            ToolId::Scale => {
                self.gizmo.set_mode(newengine_gizmo::GizmoMode::Scale);
                self.editor.gizmo_mode = newengine_editor_core::GizmoMode::Scale;
            }
        }
    }

    pub(crate) fn exec_markup_action(&mut self, action: &str) {
        match action {
            "editor.new_scene" => {
                self.scene_bridge.cmd_new_scene();
                self.editor.commands.clear();
                self.editor.selection.clear();
            }
            "editor.toggle_console" => {
                self.console_open = !self.console_open;
                if self.console_open {
                    self.ensure_dock_tab_open(dock::EditorDockTab::Console);
                }
            }
            "editor.toggle_plugins" => {
                if let Ok(mut plugin_manager) = self.plugin_manager.lock() {
                    plugin_manager.toggle();
                }
            }
            "editor.add_primitive" => {
                let Some(selected_value) = self.markup_state.vars.get("editor.primitive_sel").cloned() else {
                    return;
                };
                let Ok(id_u64) = selected_value.parse::<u64>() else {
                    return;
                };
                let primitive_id = newengine_primitives::PrimitiveId(id_u64);
                let name = self
                    .scene_bridge
                    .primitives_snapshot()
                    .iter()
                    .find(|item| item.1 == primitive_id)
                    .map(|item| item.0.clone())
                    .unwrap_or_else(|| "Primitive".to_string());
                let (cam_pos, cam_fwd) = self.viewport_bridge.read_camera_spawn();
                let mut position = cam_pos + cam_fwd * 3.0;
                position.y = position.y.max(0.5);
                self.scene_bridge
                    .cmd_spawn_primitive(primitive_id, name, position);
            }
            "editor.add_light" => {
                let kind = self
                    .markup_state
                    .vars
                    .get("editor.light_sel")
                    .map(|value| value.as_str())
                    .unwrap_or("point");
                match kind {
                    "dir" | "directional" => {
                        self.scene_bridge.cmd_spawn_directional_light(
                            "Sun".to_string(),
                            newengine_math::Vec3::new(0.0, 6.0, 0.0),
                            newengine_math::Vec3::new(-0.35, -1.0, -0.25),
                        );
                    }
                    _ => {
                        self.scene_bridge.cmd_spawn_point_light(
                            "PointLight".to_string(),
                            newengine_math::Vec3::new(0.0, 2.0, 0.0),
                        );
                    }
                }
            }
            _ => {}
        }
    }

    #[inline]
    pub(crate) fn queue_asset_spawn_from_path(
        &mut self,
        logical_path: impl Into<String>,
        source: &'static str,
    ) {
        let logical_path = logical_path.into();
        let schema_registry = self.schema_registry.read();
        let extension_registry = self.extension_registry.read();
        let contract = schema::infer_asset_spawn_contract_with_abi(
            &*schema_registry,
            Some(&*extension_registry),
            &logical_path,
        );
        self.asset_spawn_request = Some(PendingAssetSpawn { contract, source });
    }

    #[inline]
    pub(crate) fn spawn_pending_asset_near_camera(&mut self) {
        let Some(request) = self.asset_spawn_request.take() else {
            return;
        };
        self.spawn_asset_contract_near_camera(&request.contract, request.source);
    }

    pub(crate) fn read_selected_pose(
        &self,
        entity: EntityId,
    ) -> Option<(
        newengine_math::Vec3,
        newengine_math::Quat,
        newengine_math::Vec3,
        Option<[f32; 4]>,
    )> {
        let scene = self.scene_bridge.scene();
        let scene = scene.read();
        let world = scene.world();
        let transform = world.get::<Transform>(entity)?;
        let color = world.get::<Primitive>(entity).map(|primitive| primitive.color);
        Some((
            transform.position,
            transform.rotation,
            transform.scale,
            color,
        ))
    }

    pub(crate) fn refresh_inspector_cache(&mut self, entity: EntityId) {
        let scene = self.scene_bridge.scene();
        let scene = scene.read();
        let world = scene.world();

        if let Some(transform) = world.get::<Transform>(entity) {
            self.insp_pos = [transform.position.x, transform.position.y, transform.position.z];
            let (yaw, pitch, roll) = transform.yaw_pitch_roll();
            self.insp_rot_deg = [yaw.to_degrees(), pitch.to_degrees(), roll.to_degrees()];
            self.insp_scale = [transform.scale.x, transform.scale.y, transform.scale.z];
        }

        if let Some(primitive) = world.get::<Primitive>(entity) {
            self.insp_color = primitive.color;
        }

        if world.get::<Primitive>(entity).is_some() {
            if let Some(base) = world.get::<PrimitiveMaterialBase>(entity) {
                self.insp_material = base.id;
            } else {
                self.insp_material = MaterialId::invalid();
            }
        } else if let Some(material_ref) = world.get::<MaterialRef>(entity) {
            self.insp_material = material_ref.id;
        } else {
            self.insp_material = MaterialId::invalid();
        }

        self.selected_entity_cached = Some(entity);
    }
}
