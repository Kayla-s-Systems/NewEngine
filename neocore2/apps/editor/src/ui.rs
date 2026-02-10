#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_platform_winit::{egui, UiBuildFn};
use newengine_ui::markup::{render_doc_with_widgets, EguiWidgetProvider, UiMarkupDoc, UiState};
use std::any::Any;
use std::sync::{Arc, Mutex};

use newengine_ecs::EntityId;
use newengine_scene::name_or;
use newengine_transform::Transform;

use crate::shared::EditorShared;
use crate::viewport_bridge::ViewportBridge;

/// Minimal editor UI: foundation-first.
///
/// - Viewport is the only primary panel.
/// - Console is hidden by default.
pub struct EditorUiBuild {
    shared_doc: Arc<Mutex<Option<UiMarkupDoc>>>,
    state: UiState,

    shared: EditorShared,

    viewport_bridge: Arc<ViewportBridge>,

    // Viewport orbit interaction (UI-driven, not via global input plugin).
    last_drag_pos: Option<egui::Pos2>,

    selected: Option<EntityId>,
}

impl EditorUiBuild {
    #[inline]
    pub fn new(
        shared_doc: Arc<Mutex<Option<UiMarkupDoc>>>,
        viewport_bridge: Arc<ViewportBridge>,
        shared: EditorShared,
    ) -> Self {
        Self {
            shared_doc,
            state: UiState::default(),
            shared,
            viewport_bridge,
            last_drag_pos: None,
            selected: None,
        }
    }
}

impl EditorUiBuild {
    fn attrs_get<'a>(attrs: &'a [(String, String)], key: &str) -> Option<&'a str> {
        attrs
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }

    fn render_viewport(&mut self, ui: &mut egui::Ui, _attrs: &[(String, String)]) {
        let avail = ui.available_size();
        let (rect, resp) = ui.allocate_exact_size(avail, egui::Sense::click_and_drag());

        ui.painter()
            .rect_filled(rect, 0.0, egui::Color32::from_rgb(12, 12, 14));
        ui.painter().rect_stroke(
            rect,
            0.0,
            egui::Stroke::new(1.0, egui::Color32::from_gray(45)),
        );

        let ctx = ui.ctx();
        let ppp = ctx.pixels_per_point().max(0.0001);
        let px_w = (rect.width() * ppp).round().max(1.0) as u32;
        let px_h = (rect.height() * ppp).round().max(1.0) as u32;

        self.viewport_bridge.publish_extent(px_w, px_h);

        let hovered = resp.hovered();

        // Blender-style: orbit on middle mouse drag.
        let dragging = resp.dragged_by(egui::PointerButton::Middle);

        let mut dx_px = 0.0f32;
        let mut dy_px = 0.0f32;
        if dragging {
            if let Some(pos) = resp.interact_pointer_pos() {
                if let Some(prev) = self.last_drag_pos {
                    let d = pos - prev;
                    dx_px = d.x * ppp;
                    dy_px = d.y * ppp;
                }
                self.last_drag_pos = Some(pos);
            }
        } else {
            self.last_drag_pos = None;
        }

        let wheel_y_points = if hovered {
            ctx.input(|i| i.raw_scroll_delta.y)
        } else {
            0.0
        };
        let wheel_y = (wheel_y_points / 120.0).clamp(-12.0, 12.0);

        self.viewport_bridge
            .publish_orbit_input(dx_px, dy_px, wheel_y, hovered, dragging);

        let wants_kb = ctx.wants_keyboard_input();
        let mut move_mask: u64 = 0;
        if hovered && !wants_kb {
            ctx.input(|i| {
                if i.key_down(egui::Key::W) {
                    move_mask |= 1 << 0;
                }
                if i.key_down(egui::Key::A) {
                    move_mask |= 1 << 1;
                }
                if i.key_down(egui::Key::S) {
                    move_mask |= 1 << 2;
                }
                if i.key_down(egui::Key::D) {
                    move_mask |= 1 << 3;
                }
                if i.key_down(egui::Key::Q) {
                    move_mask |= 1 << 4;
                }
                if i.key_down(egui::Key::E) {
                    move_mask |= 1 << 5;
                }
                if i.modifiers.shift {
                    move_mask |= 1 << 6;
                }
            });
        }
        self.viewport_bridge.publish_move_keys(move_mask);

        let tex_user = self.viewport_bridge.read_tex_user();
        if tex_user != 0 {
            let tid = egui::TextureId::User(tex_user);
            let painter = ui.painter_at(rect);
            let mut mesh = egui::Mesh::with_texture(tid);
            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            mesh.add_rect_with_uv(rect, uv, egui::Color32::WHITE);
            painter.add(egui::Shape::mesh(mesh));
        } else {
            ui.allocate_ui_at_rect(rect, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.label("Viewport: waiting for render target...");
                });
            });
        }
    }

    fn render_ecs_hierarchy(&mut self, ui: &mut egui::Ui, _attrs: &[(String, String)], state: &mut UiState) {
        let world_guard = self.shared.scene.read();
        let world = world_guard.world();

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for id in world.iter_entities() {
                let label = name_or(world, id, "Entity");
                let selected = self.selected == Some(id);
                let resp = ui.selectable_label(selected, format!("{label}  [{:?}]", id));
                if resp.clicked() {
                    self.selected = Some(id);
                    state.set_var("ecs.selected", format!("{:?}", id));
                }
            }
        });
    }

    fn render_ecs_inspector(&mut self, ui: &mut egui::Ui, _attrs: &[(String, String)], state: &mut UiState) {
        let Some(id) = self.selected else {
            ui.label("No selection");
            return;
        };

        let mut scene = self.shared.scene.write();
        let world = scene.world_mut();

        ui.label(format!("Selected: {}", name_or(world, id, "Entity")));
        ui.separator();

        if let Some(t) = world.get_mut::<Transform>(id) {
            ui.collapsing("Transform", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Position");
                    ui.add(egui::DragValue::new(&mut t.position.x).speed(0.01));
                    ui.add(egui::DragValue::new(&mut t.position.y).speed(0.01));
                    ui.add(egui::DragValue::new(&mut t.position.z).speed(0.01));
                });

                ui.horizontal(|ui| {
                    ui.label("Rotation (quat)");
                    ui.add(egui::DragValue::new(&mut t.rotation.x).speed(0.01));
                    ui.add(egui::DragValue::new(&mut t.rotation.y).speed(0.01));
                    ui.add(egui::DragValue::new(&mut t.rotation.z).speed(0.01));
                    ui.add(egui::DragValue::new(&mut t.rotation.w).speed(0.01));
                });

                ui.horizontal(|ui| {
                    ui.label("Scale");
                    ui.add(egui::DragValue::new(&mut t.scale.x).speed(0.01));
                    ui.add(egui::DragValue::new(&mut t.scale.y).speed(0.01));
                    ui.add(egui::DragValue::new(&mut t.scale.z).speed(0.01));
                });
            });

            if ui.button("Frame selection").clicked() {
                let mut flags = self.shared.flags.write();
                flags.auto_frame = true;
                state.set_var("viewport.frame", "selection");
            }
        } else {
            ui.label("No Transform component");
        }
    }
}

impl EguiWidgetProvider for EditorUiBuild {
    fn render(
        &mut self,
        tag: &str,
        attrs: &[(String, String)],
        ui: &mut egui::Ui,
        state: &mut UiState,
    ) -> bool {
        match tag {
            "viewport" => {
                self.render_viewport(ui, attrs);
                true
            }
            "ecs_hierarchy" => {
                self.render_ecs_hierarchy(ui, attrs, state);
                true
            }
            "ecs_inspector" => {
                self.render_ecs_inspector(ui, attrs, state);
                true
            }
            "flags" => {
                let mut flags = self.shared.flags.write();
                let id = Self::attrs_get(attrs, "id").unwrap_or("flags");
                ui.horizontal(|ui| {
                    if ui.checkbox(&mut flags.show_grid, "Grid").changed() {
                        state.set_var(format!("{id}.show_grid"), format!("{}", flags.show_grid));
                    }
                    if ui.checkbox(&mut flags.show_model, "Model").changed() {
                        state.set_var(format!("{id}.show_model"), format!("{}", flags.show_model));
                    }
                });
                true
            }
            _ => false,
        }
    }
}

impl UiBuildFn for EditorUiBuild {
    fn build(&mut self, ctx_any: &mut dyn Any) {
        let Some(ctx) = ctx_any.downcast_mut::<egui::Context>() else {
            return;
        };

        let Some(doc) = ({ self.shared_doc.lock().ok().and_then(|g| g.as_ref().cloned()) }) else {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.label("UI markup: not loaded yet");
                });
            });
            return;
        };

        // Avoid double-borrowing `self` by temporarily moving the UI state out.
        let mut state = std::mem::take(&mut self.state);
        // Seed common vars every frame.
        {
            let scene = self.shared.scene.read();
            let entities = scene.world().iter_entities().count();
            state.set_var("ecs.entity_count", entities.to_string());
        }
        render_doc_with_widgets(&doc, ctx, &mut state, self);
        self.state = state;

        // Apply markup-driven actions.
        if self.state.take_clicked("btn_load_model") {
            let path = self
                .state
                .strings
                .get("model_path")
                .map(String::as_str)
                .unwrap_or("")
                .trim();

            if !path.is_empty() {
                self.shared.requests.lock().load_model_path = Some(path.to_string());
            }
        }
    }
}