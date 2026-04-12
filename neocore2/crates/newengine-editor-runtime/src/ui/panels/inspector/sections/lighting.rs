#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_lighting::{AmbientLight, DirectionalLight, PointLight};
use newengine_transform::GlobalTransform;

use crate::ui::panels::inspector::components::{
    dir_to_yaw_pitch_deg, yaw_pitch_deg_to_dir,
};
use crate::ui::{property_grid, schema, EditorUiBuild};

pub(crate) fn draw(
    me: &mut EditorUiBuild,
    ui: &mut egui::Ui,
    selection_ctx: &schema::SelectionContext,
) {
    let entity = selection_ctx.entity;
    let fields = schema::property_fields(me, selection_ctx, schema::PropertySectionId::Lighting);
    property_grid::section_card_descriptor(ui, "Lighting", &fields, |ui, field| {
        let scene = me.scene_bridge.scene();
        let scene = scene.read();
        let world = scene.world();
        match field.id {
            schema::PropertyFieldId::LightAmbientColor => {
                property_grid::field_label(ui, field);
                let ambient = world.resource::<AmbientLight>().copied().unwrap_or_default();
                let mut rgb = ambient.color;
                if ui.color_edit_button_rgb(&mut rgb).changed() {
                    me.scene_bridge.cmd_set_ambient_light(rgb, ambient.intensity);
                }
                property_grid::end_row(ui);
            }
            schema::PropertyFieldId::LightAmbientIntensity => {
                property_grid::field_label(ui, field);
                let ambient = world.resource::<AmbientLight>().copied().unwrap_or_default();
                let mut intensity = ambient.intensity;
                if ui.add(egui::Slider::new(&mut intensity, 0.0..=2.0)).changed() {
                    me.scene_bridge.cmd_set_ambient_light(ambient.color, intensity);
                }
                property_grid::end_row(ui);
            }
            schema::PropertyFieldId::LightColor => {
                property_grid::field_label(ui, field);
                if let Some(directional) = world.get::<DirectionalLight>(entity).copied() {
                    let mut rgb = directional.color;
                    if ui.color_edit_button_rgb(&mut rgb).changed() {
                        me.scene_bridge.cmd_set_directional_light(
                            entity,
                            newengine_math::Vec3::new(
                                directional.direction_ws[0],
                                directional.direction_ws[1],
                                directional.direction_ws[2],
                            ),
                            rgb,
                            directional.intensity,
                        );
                    }
                } else if let Some(point) = world.get::<PointLight>(entity).copied() {
                    let mut rgb = point.color;
                    if ui.color_edit_button_rgb(&mut rgb).changed() {
                        me.scene_bridge
                            .cmd_set_point_light(entity, rgb, point.intensity, point.range);
                    }
                } else {
                    ui.label("-");
                }
                property_grid::end_row(ui);
            }
            schema::PropertyFieldId::LightIntensity => {
                property_grid::field_label(ui, field);
                if let Some(directional) = world.get::<DirectionalLight>(entity).copied() {
                    let mut intensity = directional.intensity;
                    if ui.add(egui::Slider::new(&mut intensity, 0.0..=50.0)).changed() {
                        me.scene_bridge.cmd_set_directional_light(
                            entity,
                            newengine_math::Vec3::new(
                                directional.direction_ws[0],
                                directional.direction_ws[1],
                                directional.direction_ws[2],
                            ),
                            directional.color,
                            intensity,
                        );
                    }
                } else if let Some(point) = world.get::<PointLight>(entity).copied() {
                    let mut intensity = point.intensity;
                    if ui.add(egui::Slider::new(&mut intensity, 0.0..=50.0)).changed() {
                        me.scene_bridge
                            .cmd_set_point_light(entity, point.color, intensity, point.range);
                    }
                } else {
                    ui.label("-");
                }
                property_grid::end_row(ui);
            }
            schema::PropertyFieldId::LightRange => {
                property_grid::field_label(ui, field);
                if let Some(point) = world.get::<PointLight>(entity).copied() {
                    let mut range = point.range;
                    if ui.add(egui::Slider::new(&mut range, 0.0..=200.0)).changed() {
                        me.scene_bridge
                            .cmd_set_point_light(entity, point.color, point.intensity, range);
                    }
                } else {
                    ui.label("-");
                }
                property_grid::end_row(ui);
            }
            schema::PropertyFieldId::LightYawDeg => {
                property_grid::field_label(ui, field);
                if let Some(directional) = world.get::<DirectionalLight>(entity).copied() {
                    let direction = newengine_math::Vec3::new(
                        directional.direction_ws[0],
                        directional.direction_ws[1],
                        directional.direction_ws[2],
                    );
                    let (mut yaw_deg, pitch_deg) = dir_to_yaw_pitch_deg(direction);
                    if ui.add(egui::DragValue::new(&mut yaw_deg).speed(0.25)).changed() {
                        let new_direction = yaw_pitch_deg_to_dir(yaw_deg, pitch_deg);
                        me.scene_bridge.cmd_set_directional_light(
                            entity,
                            new_direction,
                            directional.color,
                            directional.intensity,
                        );
                    }
                } else {
                    ui.label("-");
                }
                property_grid::end_row(ui);
            }
            schema::PropertyFieldId::LightPitchDeg => {
                property_grid::field_label(ui, field);
                if let Some(directional) = world.get::<DirectionalLight>(entity).copied() {
                    let direction = newengine_math::Vec3::new(
                        directional.direction_ws[0],
                        directional.direction_ws[1],
                        directional.direction_ws[2],
                    );
                    let (yaw_deg, mut pitch_deg) = dir_to_yaw_pitch_deg(direction);
                    if ui.add(egui::DragValue::new(&mut pitch_deg).speed(0.25)).changed() {
                        let new_direction = yaw_pitch_deg_to_dir(yaw_deg, pitch_deg);
                        me.scene_bridge.cmd_set_directional_light(
                            entity,
                            new_direction,
                            directional.color,
                            directional.intensity,
                        );
                    }
                } else {
                    ui.label("-");
                }
                property_grid::end_row(ui);
            }
            _ => {}
        }
    });

    let scene = me.scene_bridge.scene();
    let scene = scene.read();
    let world = scene.world();
    if let Some(global_transform) = world.get::<GlobalTransform>(entity) {
        let position = global_transform.0.w_axis;
        ui.label(format!(
            "World Position: [{:.2}, {:.2}, {:.2}]",
            position.x, position.y, position.z
        ));
    }
}
