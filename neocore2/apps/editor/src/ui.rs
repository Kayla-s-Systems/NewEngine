#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_platform_winit::egui;
use newengine_ui::markup::{UiMarkupDoc, UiState};
use newengine_ui::UiBuildFn;

use std::any::Any;
use std::sync::{Arc, Mutex};

use newengine_viewport::ViewportState;

use crate::plugin_manager::PluginManagerUi;
use crate::scene_bridge::SceneBridge;
use crate::viewport_bridge::ViewportBridge;

/// Minimal editor UI: foundation-first.
///
/// - Viewport is the only primary panel.
/// - Console is hidden by default.
pub struct EditorUiBuild {
    shared_doc: Arc<Mutex<Option<UiMarkupDoc>>>,
    state: UiState,

    viewport: ViewportState,

    viewport_bridge: Arc<ViewportBridge>,
    scene_bridge: Arc<SceneBridge>,
    plugin_manager: PluginManagerUi,

    // Orbit interaction (UI-driven, not via global input plugin).
    last_drag_pos: Option<egui::Pos2>,

    console_open: bool,
    console_input: String,

    // Plugin manager UI (fully encapsulated).
}

impl EditorUiBuild {
    #[inline]
    pub fn new(
        shared_doc: Arc<Mutex<Option<UiMarkupDoc>>>,
        viewport_bridge: Arc<ViewportBridge>,
        plugins_bridge: Arc<crate::plugin_manager::PluginManagerBridge>,
        scene_bridge: Arc<SceneBridge>,
    ) -> Self {
        let cam = scene_bridge
            .scene()
            .read()
            .active_camera()
            .expect("scene has no active camera");
        let viewport = ViewportState::new(Some(cam));

        Self {
            shared_doc,
            state: UiState::default(),
            viewport,
            viewport_bridge,
            scene_bridge,
            plugin_manager: PluginManagerUi::new(plugins_bridge),
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

                let entities = self.scene_bridge.scene().read().world().entity_count();
                ui.label(format!("entities: {entities}"));

                ui.separator();

                if ui.button("New Scene").clicked() {
                    self.scene_bridge.cmd_new_scene();
                }
                if ui.button("Add Cube").clicked() {
                    self.scene_bridge.cmd_spawn_cube(glam::Vec3::new(0.0, 0.5, 0.0));
                }
                if ui.button("Add Plane").clicked() {
                    self.scene_bridge.cmd_spawn_plane(glam::Vec3::new(0.0, 0.0, 0.0));
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    self.plugin_manager.topbar_button(ui);
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
                egui::StrokeKind::Inside,
            );

            let ppp = ctx.pixels_per_point().max(0.0001);
            let px_w = (rect.width() * ppp).round().max(1.0) as u32;
            let px_h = (rect.height() * ppp).round().max(1.0) as u32;

            self.viewport.set_pixel_extent(px_w, px_h);
            self.viewport_bridge.publish_extent(px_w, px_h);

            let dragging = resp.dragged_by(egui::PointerButton::Primary);
            let active = resp.hovered() || dragging;

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

            let wheel_y_points = if active {
                ctx.input(|i| i.raw_scroll_delta.y)
            } else {
                0.0
            };

            let wheel_y = (wheel_y_points / 240.0).clamp(-2.0, 2.0);

            self.viewport_bridge
                .publish_orbit_input(dx_px, dy_px, wheel_y, active, dragging);

            let wants_kb = ctx.wants_keyboard_input();
            let mut move_mask: u64 = 0;

            if active && !wants_kb {
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

            ui.allocate_ui_at_rect(rect, |ui| {
                if tex_user != 0 {
                    let tid = egui::TextureId::User(tex_user);
                    let st = egui::load::SizedTexture::new(tid, rect.size());
                    ui.add(egui::Image::new(st).fit_to_exact_size(rect.size()));
                } else {
                    ui.centered_and_justified(|ui| {
                        ui.label("Viewport: waiting for render target...");
                    });
                }
            });
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

    #[inline]
    fn build_ui(&mut self, ctx_any: &mut dyn Any) {
        let Some(ctx) = ctx_any.downcast_mut::<egui::Context>() else {
            return;
        };

        let _maybe_doc = {
            self.shared_doc
                .lock()
                .ok()
                .and_then(|g| g.as_ref().cloned())
        };

        if ctx.input(|i| i.key_pressed(egui::Key::F1)) {
            self.console_open = !self.console_open;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::F2)) {
            self.plugin_manager.toggle();
        }
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::N)) {
            self.scene_bridge.cmd_new_scene();
        }

        self.ui_topbar(ctx);
        self.ui_viewport(ctx);
        self.ui_console(ctx);
        self.plugin_manager.show(ctx);
    }
}

impl UiBuildFn for EditorUiBuild {
    #[inline]
    fn build(&mut self, ctx_any: &mut dyn Any) {
        self.build_ui(ctx_any);
    }
}
