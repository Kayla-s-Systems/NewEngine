#![forbid(unsafe_op_in_unsafe_fn)]

use std::any::Any;

use egui;
use newengine_ui::input::keys as ui_keys;

use crate::ui::providers;
use crate::ui::{panels, shell, theme, EditorUiBuild, SceneIoMode};

impl EditorUiBuild {
    #[inline]
    pub(crate) fn build_ui(&mut self, ctx_any: &mut dyn Any) {
        let Some(ctx) = ctx_any.downcast_mut::<egui::Context>() else {
            return;
        };

        let _shared_doc = self.shared_doc.lock().ok().and_then(|guard| guard.clone());

        theme::apply_editor_theme(ctx);
        self.process_command_bus();

        let materials = self.scene_bridge.materials();
        self.material_pipeline.pump(&materials);
        self.icons.pump_into_state(ctx, &mut self.markup_state);

        let wants_keyboard = ctx.wants_keyboard_input();
        let play_mode = self.scene_bridge.play_mode();
        let runtime_mode = play_mode.is_runtime();

        if !runtime_mode {
            self.process_editor_shortcuts(ctx, wants_keyboard);
            shell::draw(self, ctx);
        } else {
            panels::viewport::draw(self, ctx);
        }

        self.process_command_bus();

        let mut user_data = ();
        self.ui_hub.run(ctx_any, &mut user_data);
    }

    #[inline]
    pub(crate) fn process_editor_shortcuts(
        &mut self,
        ctx: &egui::Context,
        wants_keyboard: bool,
    ) {
        if self.key_pressed(ui_keys::F1) {
            self.execute_ui_action(&providers::UiAction::ToggleConsole);
        }
        if self.key_pressed(ui_keys::F2) {
            self.execute_ui_action(&providers::UiAction::TogglePlugins);
        }

        if !wants_keyboard {
            if self.command_pressed(ui_keys::KEY_N) {
                self.execute_ui_action(&providers::UiAction::NewScene);
            }
            if self.command_pressed(ui_keys::KEY_O) {
                self.execute_ui_action(&providers::UiAction::OpenScene(SceneIoMode::Load));
            }
            if self.command_pressed(ui_keys::KEY_S) {
                self.execute_ui_action(&providers::UiAction::OpenScene(SceneIoMode::Save));
            }
            if self.command_down() && ctx.input(|input| input.key_pressed(egui::Key::P)) {
                self.execute_ui_action(&providers::UiAction::OpenCommandPalette);
            }
        }

        if wants_keyboard {
            return;
        }

        let undo = self.command_pressed(ui_keys::KEY_Z);
        let redo = self.command_pressed(ui_keys::KEY_Y)
            || (self.command_down()
            && self.shift_down()
            && self.key_pressed(ui_keys::KEY_Z));

        if undo {
            if let Some(cmd) = self.editor.commands.pop_undo() {
                self.apply_editor_command_undo(cmd);
            }
        } else if redo {
            if let Some(cmd) = self.editor.commands.pop_redo() {
                self.apply_editor_command_redo(cmd);
            }
        }
    }
}
