#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_platform_winit::{egui, UiBuildFn};
use newengine_scene::Scene;
use newengine_ui::markup::{UiMarkupDoc, UiState};
use newengine_viewport::ViewportState;
use std::any::Any;
use std::sync::{Arc, Mutex};

use crate::viewport_bridge::ViewportBridge;

/// Minimal editor UI: foundation-first.
///
/// - Viewport is the only primary panel.
/// - Console is hidden by default.
pub struct EditorUiBuild {
    shared_doc: Arc<Mutex<Option<UiMarkupDoc>>>,
    state: UiState,

    scene: Scene,
    viewport: ViewportState,

    viewport_bridge: Arc<ViewportBridge>,

    // Orbit interaction (UI-driven, not via global input plugin).
    last_drag_pos: Option<egui::Pos2>,

    console_open: bool,
    console_input: String,
}

impl EditorUiBuild {
    #[inline]
    pub fn new(shared_doc: Arc<Mutex<Option<UiMarkupDoc>>>, viewport_bridge: Arc<ViewportBridge>) -> Self {
        let scene = Scene::demo();
        let viewport = ViewportState::new(Some(
            scene.active_camera().expect("scene has no active camera"),
        ));

        Self {
            shared_doc,
            state: UiState::default(),
            scene,
            viewport,
            viewport_bridge,
            last_drag_pos: None,
            console_open: false,
            console_input: String::new(),
        }
    }

    fn ui_topbar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("topbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("NewEngine Editor (Foundation)");
                ui.separator();

                let entities = self.scene.world().iter_entities().count();
                ui.label(format!("entities: {entities}"));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Console").clicked() {
                        self.console_open = !self.console_open;
                    }
                });
            });
        });
    }

    fn ui_viewport(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let avail = ui.available_size();
            let (rect, resp) = ui.allocate_exact_size(avail, egui::Sense::click_and_drag());

            ui.painter()
                .rect_filled(rect, 0.0, egui::Color32::from_rgb(12, 12, 14));
            ui.painter().rect_stroke(
                rect,
                0.0,
                egui::Stroke::new(1.0, egui::Color32::from_gray(45)),
            );

            let ppp = ctx.pixels_per_point().max(0.0001);
            let px_w = (rect.width() * ppp).round().max(1.0) as u32;
            let px_h = (rect.height() * ppp).round().max(1.0) as u32;

            self.viewport.set_pixel_extent(px_w, px_h);
            self.viewport_bridge.publish_extent(px_w, px_h);

            let hovered = resp.hovered();
            let dragging = resp.dragged_by(egui::PointerButton::Primary);

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

                // Draw as a full-rect mesh to avoid any aspect/fit heuristics.
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
        });
    }

    fn ui_console(&mut self, ctx: &egui::Context) {
        if !self.console_open {
            return;
        }

        egui::Window::new("Console")
            .open(&mut self.console_open)
            .resizable(true)
            .vscroll(true)
            .show(ctx, |ui| {
                ui.label("Foundation mode: console is intentionally minimal for now.");
                ui.add_space(6.0);

                let resp = ui.add(
                    egui::TextEdit::singleline(&mut self.console_input)
                        .hint_text("type a command (no-op)")
                        .desired_width(f32::INFINITY),
                );

                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    self.console_input.clear();
                }
            });
    }
}

impl UiBuildFn for EditorUiBuild {
    fn build(&mut self, ctx_any: &mut dyn Any) {
        let Some(ctx) = ctx_any.downcast_mut::<egui::Context>() else {
            return;
        };

        let _maybe_doc = { self.shared_doc.lock().ok().and_then(|g| g.as_ref().cloned()) };

        if ctx.input(|i| i.key_pressed(egui::Key::F1)) {
            self.console_open = !self.console_open;
        }

        self.ui_topbar(ctx);
        self.ui_viewport(ctx);
        self.ui_console(ctx);
    }
}