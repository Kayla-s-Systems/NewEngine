#![forbid(unsafe_op_in_unsafe_fn)]

use egui;
use newengine_ui::BuiltinUiIcon;

use crate::gameplay::DisplayMode;
use crate::scene_bridge::SceneImportedAssetDescriptor;
use crate::ui::panels::inspector::components::{
    imported_kind_label, imported_repr_label,
};
use crate::ui::{property_grid, schema, theme, EditorUiBuild};

pub(crate) fn draw_header(me: &mut EditorUiBuild, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.heading("Details");
        ui.separator();
        ui.label(format!("{} selected", me.editor.selection.len()));
    });

    ui.add(
        egui::TextEdit::singleline(&mut me.details_filter)
            .hint_text("Search transform, material, collision, lighting...")
            .desired_width(f32::INFINITY),
    );

    ui.horizontal(|ui| {
        if me
            .icons
            .icon_button(ui, BuiltinUiIcon::Close, "Clear")
            .on_hover_text("Clear filter")
            .clicked()
        {
            me.details_filter.clear();
        }
    });

    ui.add_space(6.0);
}

pub(crate) fn draw_summary(
    me: &mut EditorUiBuild,
    ui: &mut egui::Ui,
    selection_ctx: &schema::SelectionContext,
) {
    let schema_ctx = schema::build_editor_schema_context(me, Some(selection_ctx));
    let archetype = schema::archetype_provider(selection_ctx);
    let runtime_provider = schema::runtime_state_provider(me);
    let editor_provider = schema::editor_state_provider(me);
    let selection_count = me.editor.selection.len();
    let entity = selection_ctx.entity;

    theme::section_frame(ui).show(ui, |ui| {
        ui.label(egui::RichText::new(&selection_ctx.name).strong());
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(format!("{:?}", selection_ctx.kind)).small().weak());
            ui.label(
                egui::RichText::new(format!("Archetype: {:?}", archetype.archetype))
                    .small()
                    .weak(),
            );
            ui.label(
                egui::RichText::new(format!("Entity #{}", entity.stable_u64()))
                    .small()
                    .weak(),
            );
            ui.label(
                egui::RichText::new(format!("Mode: {:?}", schema_ctx.play_mode))
                    .small()
                    .weak(),
            );
            ui.label(
                egui::RichText::new(format!("Tool: {:?}", editor_provider.active_tool))
                    .small()
                    .weak(),
            );
            ui.label(
                egui::RichText::new(format!("Camera: {}", editor_provider.camera_speed_label))
                    .small()
                    .weak(),
            );
            if runtime_provider.collision_overlay {
                ui.label(egui::RichText::new("Collision Overlay").small());
            }
            if selection_count > 1 {
                ui.label(
                    egui::RichText::new(format!("Editing primary of {selection_count}"))
                        .small(),
                );
            }
        });
    });

    ui.separator();

    let fields = schema::property_fields(me, selection_ctx, schema::PropertySectionId::Summary);
    property_grid::section_card_descriptor(ui, "Summary", &fields, |ui, field| {
        property_grid::field_label(ui, field);
        match field.id {
            schema::PropertyFieldId::Name => {
                ui.monospace(&selection_ctx.name);
            }
            schema::PropertyFieldId::Kind => {
                ui.label(format!("{:?}", selection_ctx.kind));
            }
            schema::PropertyFieldId::Entity => {
                ui.monospace(format!("#{}", selection_ctx.entity.stable_u64()));
            }
            schema::PropertyFieldId::DisplayMode => {
                ui.label(match selection_ctx.display_mode {
                    DisplayMode::Both => "Editor + Game",
                    DisplayMode::EditorOnly => "Editor only",
                    DisplayMode::GameOnly => "Game only",
                });
            }
            schema::PropertyFieldId::ImportedAssetPath
            | schema::PropertyFieldId::ImportedAssetKind
            | schema::PropertyFieldId::ImportedAssetRepresentation => {
                let scene = me.scene_bridge.scene();
                let scene = scene.read();
                let world = scene.world();
                if let Some(imported) = world.get::<SceneImportedAssetDescriptor>(entity) {
                    match field.id {
                        schema::PropertyFieldId::ImportedAssetPath => {
                            ui.monospace(&imported.logical_path);
                        }
                        schema::PropertyFieldId::ImportedAssetKind => {
                            ui.label(imported_kind_label(imported.import_kind));
                        }
                        schema::PropertyFieldId::ImportedAssetRepresentation => {
                            ui.label(imported_repr_label(imported.representation));
                        }
                        _ => {
                            ui.label("-");
                        }
                    }
                } else {
                    ui.label("-");
                }
            }
            _ => {
                ui.label("-");
            }
        }
        property_grid::end_row(ui);
    });
}

pub(crate) fn draw_context_actions(
    me: &mut EditorUiBuild,
    ui: &mut egui::Ui,
    selection_ctx: &schema::SelectionContext,
) {
    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        for action in schema::selection_context_actions(me, Some(selection_ctx)) {
            let button = egui::Button::selectable(action.selected, action.label);
            if ui.add_enabled(action.enabled, button).clicked() {
                me.execute_context_action(action.id);
            }
        }
    });
}
