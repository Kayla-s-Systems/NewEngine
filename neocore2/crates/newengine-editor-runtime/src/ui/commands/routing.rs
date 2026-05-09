#![forbid(unsafe_op_in_unsafe_fn)]

use super::TypedEditorCommand;
use crate::ui::extension_abi;

use super::super::schema;
use super::super::EditorUiBuild;

impl EditorUiBuild {
    #[inline]
    pub(crate) fn process_command_bus(&mut self) {
        if self.command_bus.is_empty() {
            return;
        }

        let mut guard = 0usize;
        while let Some(command) = self.command_bus.pop() {
            self.dispatch_typed_command(command);
            guard += 1;
            if guard >= 256 {
                log::warn!("editor ui: command bus guard tripped pending={}", self.command_bus.len());
                break;
            }
        }
    }

    #[inline]
    pub(crate) fn dispatch_typed_command(&mut self, command: TypedEditorCommand) {
        if self.dispatch_abi_command_handlers(&command) {
            return;
        }
        match command {
            TypedEditorCommand::UiAction(action) => self.dispatch_ui_action(&action),
            TypedEditorCommand::ContextAction(action) => self.dispatch_context_action(action),
            TypedEditorCommand::SpawnAsset { contract, source } => {
                self.spawn_asset_contract_near_camera(&contract, source)
            }
            TypedEditorCommand::SetTool(tool) => {
                self.dispatch_ui_action(&super::super::providers::UiAction::SetTool(tool))
            }
            TypedEditorCommand::SetPlayMode(mode) => {
                self.dispatch_ui_action(&super::super::providers::UiAction::SetPlayMode(mode))
            }
            TypedEditorCommand::SetWorkspacePreset(preset) => {
                self.dispatch_ui_action(&super::super::providers::UiAction::SetWorkspacePreset(preset))
            }
            TypedEditorCommand::SetViewportMode(mode) => {
                self.dispatch_ui_action(&super::super::providers::UiAction::SetViewportMode(mode))
            }
            TypedEditorCommand::PublishFrameSelection => self.viewport_bridge.publish_frame_request(false),
            TypedEditorCommand::PublishFrameAll => self.viewport_bridge.publish_frame_request(true),
            TypedEditorCommand::ToggleCollisionOverlay => {
                self.dispatch_context_action(schema::ContextActionId::ToggleCollisionOverlay)
            }
        }
    }

    #[inline]
    pub(crate) fn dispatch_abi_command_handlers(&mut self, command: &TypedEditorCommand) -> bool {
        let results = {
            let registry = self.extension_registry.read();
            if registry.command_handlers.is_empty() {
                return false;
            }
            let invocation = extension_abi::to_abi_command_invocation(self, command, "typed_command_bus");
            registry
                .command_handlers
                .iter()
                .map(|(_plugin_id, handler)| handler.handle_command(invocation.clone()))
                .collect::<Vec<_>>()
        };

        let mut handled = false;
        for result in results {
            handled |= extension_abi::apply_command_handler_result(self, result);
        }
        handled
    }
}
