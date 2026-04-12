#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_editor_core::ToolId;

use crate::ui::commands::TypedEditorCommand;
use crate::ui::{providers, schema, EditorUiBuild};

impl EditorUiBuild {
    #[inline]
    pub(crate) fn execute_context_action(&mut self, action: schema::ContextActionId) {
        self.command_bus.push(TypedEditorCommand::ContextAction(action));
    }

    pub(crate) fn dispatch_context_action(&mut self, action: schema::ContextActionId) {
        match action {
            schema::ContextActionId::FrameSelection => {
                self.viewport_bridge.publish_frame_request(false)
            }
            schema::ContextActionId::Deselect => {
                self.execute_ui_action(&providers::UiAction::Deselect)
            }
            schema::ContextActionId::ToggleCollisionOverlay => {
                let next = !self.scene_bridge.collision_wireframe_enabled();
                self.scene_bridge.cmd_set_collision_wireframe(next);
            }
            schema::ContextActionId::EnterPlay => {
                self.execute_ui_action(&providers::UiAction::SetPlayMode(
                    crate::gameplay::EditorPlayMode::Play,
                ))
            }
            schema::ContextActionId::EnterSimulate => {
                self.execute_ui_action(&providers::UiAction::SetPlayMode(
                    crate::gameplay::EditorPlayMode::Simulate,
                ))
            }
            schema::ContextActionId::StopRuntime => {
                self.execute_ui_action(&providers::UiAction::StopRuntime)
            }
            schema::ContextActionId::SelectTool => {
                self.execute_ui_action(&providers::UiAction::SetTool(ToolId::Select))
            }
            schema::ContextActionId::MoveTool => {
                self.execute_ui_action(&providers::UiAction::SetTool(ToolId::Translate))
            }
            schema::ContextActionId::RotateTool => {
                self.execute_ui_action(&providers::UiAction::SetTool(ToolId::Rotate))
            }
            schema::ContextActionId::ScaleTool => {
                self.execute_ui_action(&providers::UiAction::SetTool(ToolId::Scale))
            }
            schema::ContextActionId::AddCollision => {
                if let Some(entity) = self.editor.selection.primary() {
                    self.scene_bridge
                        .cmd_set_collision_body(entity, crate::gameplay::CollisionBody::default());
                }
            }
            schema::ContextActionId::RemoveCollision => {
                if let Some(entity) = self.editor.selection.primary() {
                    self.scene_bridge.cmd_clear_collision_body(entity);
                }
            }
            schema::ContextActionId::SpawnAssetHere => self.spawn_pending_asset_near_camera(),
        }
    }
}
