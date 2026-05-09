#![forbid(unsafe_op_in_unsafe_fn)]

use egui;

use super::super::{providers, EditorUiBuild};

pub(crate) fn draw(me: &mut EditorUiBuild, ctx: &egui::Context) {
    egui::TopBottomPanel::top("ne_menubar")
        .resizable(false)
        .exact_height(24.0)
        .show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                for menu in providers::menubar_descriptors(me) {
                    ui.menu_button(menu.label, |ui| {
                        for entry in menu.entries {
                            match entry {
                                providers::UiMenuEntry::Action(desc) => {
                                    let response = ui.add_enabled(
                                        desc.enabled,
                                        egui::Button::new(desc.label.as_ref()),
                                    );
                                    if response.clicked() {
                                        me.execute_ui_action(&desc.action);
                                        ui.close();
                                    }
                                }
                                providers::UiMenuEntry::Separator => {
                                    ui.separator();
                                }
                                providers::UiMenuEntry::Info(text) => {
                                    ui.label(text.as_ref());
                                }
                            }
                        }
                    });
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let entities = me.scene_bridge.scene().read().world().entity_count();
                    ui.label(format!("Entities: {entities}"));
                });
            });
        });
}
