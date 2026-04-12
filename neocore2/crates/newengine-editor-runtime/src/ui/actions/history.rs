#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::EntityId;
use newengine_editor_core::EditorCommand;
use newengine_materials::MaterialId;

use crate::ui::{
    from_command_collision_body, from_command_display_mode, to_command_collision_body,
    to_command_display_mode, EditorUiBuild,
};

impl EditorUiBuild {
    #[inline]
    pub(crate) fn apply_display_mode_with_history(
        &mut self,
        entity: EntityId,
        before: crate::gameplay::DisplayMode,
        after: crate::gameplay::DisplayMode,
    ) {
        if before == after {
            return;
        }
        self.editor.commands.push(EditorCommand::SetDisplayMode {
            entity,
            before: to_command_display_mode(before),
            after: to_command_display_mode(after),
        });
        self.scene_bridge.cmd_set_display_visibility(entity, after);
    }

    #[inline]
    pub(crate) fn apply_primitive_color_with_history(
        &mut self,
        entity: EntityId,
        before: [f32; 4],
        after: [f32; 4],
    ) {
        if before == after {
            return;
        }
        self.editor
            .commands
            .push(EditorCommand::SetPrimitiveColor { entity, before, after });
        self.scene_bridge.cmd_set_primitive_color(entity, after);
    }

    #[inline]
    pub(crate) fn apply_material_with_history(
        &mut self,
        entity: EntityId,
        before: MaterialId,
        after: MaterialId,
    ) {
        if before == after {
            return;
        }
        self.editor
            .commands
            .push(EditorCommand::SetMaterial { entity, before, after });
        self.scene_bridge.cmd_set_material(entity, after);
    }

    #[inline]
    pub(crate) fn apply_collision_with_history(
        &mut self,
        entity: EntityId,
        before: Option<crate::gameplay::CollisionBody>,
        after: Option<crate::gameplay::CollisionBody>,
    ) {
        if before == after {
            return;
        }
        self.editor.commands.push(EditorCommand::SetCollisionBody {
            entity,
            before: before.map(to_command_collision_body),
            after: after.map(to_command_collision_body),
        });
        match after {
            Some(body) => self.scene_bridge.cmd_set_collision_body(entity, body),
            None => self.scene_bridge.cmd_clear_collision_body(entity),
        }
    }

    #[inline]
    pub(crate) fn apply_editor_command_undo(&self, cmd: EditorCommand) {
        match cmd {
            EditorCommand::SetTransform { entity, before, .. } => {
                self.scene_bridge.cmd_set_transform(
                    entity,
                    before.position,
                    before.rotation_ypr,
                    before.scale,
                );
            }
            EditorCommand::SetDisplayMode { entity, before, .. } => {
                self.scene_bridge
                    .cmd_set_display_visibility(entity, from_command_display_mode(before));
            }
            EditorCommand::SetPrimitiveColor { entity, before, .. } => {
                self.scene_bridge.cmd_set_primitive_color(entity, before);
            }
            EditorCommand::SetMaterial { entity, before, .. } => {
                self.scene_bridge.cmd_set_material(entity, before);
            }
            EditorCommand::SetCollisionBody { entity, before, .. } => match before {
                Some(body) => self
                    .scene_bridge
                    .cmd_set_collision_body(entity, from_command_collision_body(body)),
                None => self.scene_bridge.cmd_clear_collision_body(entity),
            },
        }
    }

    #[inline]
    pub(crate) fn apply_editor_command_redo(&self, cmd: EditorCommand) {
        match cmd {
            EditorCommand::SetTransform { entity, after, .. } => {
                self.scene_bridge.cmd_set_transform(
                    entity,
                    after.position,
                    after.rotation_ypr,
                    after.scale,
                );
            }
            EditorCommand::SetDisplayMode { entity, after, .. } => {
                self.scene_bridge
                    .cmd_set_display_visibility(entity, from_command_display_mode(after));
            }
            EditorCommand::SetPrimitiveColor { entity, after, .. } => {
                self.scene_bridge.cmd_set_primitive_color(entity, after);
            }
            EditorCommand::SetMaterial { entity, after, .. } => {
                self.scene_bridge.cmd_set_material(entity, after);
            }
            EditorCommand::SetCollisionBody { entity, after, .. } => match after {
                Some(body) => self
                    .scene_bridge
                    .cmd_set_collision_body(entity, from_command_collision_body(body)),
                None => self.scene_bridge.cmd_clear_collision_body(entity),
            },
        }
    }
}
