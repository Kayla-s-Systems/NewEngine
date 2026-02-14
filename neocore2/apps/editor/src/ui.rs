use newengine_platform_winit::{egui, UiBuildFn};
use newengine_ui::markup::{UiMarkupDoc, UiState};
use std::any::Any;
use std::sync::{Arc, Mutex};

use newengine_scene::Scene;
use newengine_viewport::ViewportState;

use crate::plugin_manager_bridge::PluginManagerBridge;
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
    plugins_bridge: Arc<PluginManagerBridge>,

    // Orbit interaction (UI-driven, not via global input plugin).
    last_drag_pos: Option<egui::Pos2>,

    console_open: bool,
    console_input: String,

    plugins_open: bool,
    selected_plugin: Option<String>,
}


impl EditorUiBuild {
    #[inline]
    pub fn new(
        shared_doc: Arc<Mutex<Option<UiMarkupDoc>>>,
        viewport_bridge: Arc<ViewportBridge>,
        plugins_bridge: Arc<PluginManagerBridge>,
    ) -> Self {
        let scene = Scene::demo();
        let viewport = ViewportState::new(Some(scene.active_camera().expect("scene has no active camera")));

        Self {
            shared_doc,
            state: UiState::default(),
            scene,
            viewport,
            viewport_bridge,
            plugins_bridge,
            last_drag_pos: None,
            console_open: false,
            console_input: String::new(),
            plugins_open: false,
            selected_plugin: None,
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
                    if ui.button("Plugins").clicked() {
                        self.plugins_open = !self.plugins_open;
                    }
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
            // The viewport must be able to capture drag + wheel *only when hovered*.
            // We do NOT want global mouse delta from the input plugin to rotate the model.
            let (rect, resp) = ui.allocate_exact_size(avail, egui::Sense::click_and_drag());

            // Frame for viewport.
            ui.painter().rect_filled(rect, 0.0, egui::Color32::from_rgb(12, 12, 14));
            ui.painter().rect_stroke(rect, 0.0, egui::Stroke::new(1.0, egui::Color32::from_gray(45)));

            // Convert to physical pixels.
            let ppp = ctx.pixels_per_point().max(0.0001);
            let px_w = (rect.width() * ppp).round().max(1.0) as u32;
            let px_h = (rect.height() * ppp).round().max(1.0) as u32;

            // ViewportState might want this too.
            self.viewport.set_pixel_extent(px_w, px_h);

            // Publish desired size each frame (UI -> render).
            self.viewport_bridge.publish_extent(px_w, px_h);

            // --- Orbit input (UI -> render) ---
            // Rotate ONLY while the viewport is capturing a primary-button drag.
            // Zoom ONLY while hovered.
            let hovered = resp.hovered();
            let dragging = resp.dragged_by(egui::PointerButton::Primary);

            // Compute per-frame drag delta in points.
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

            // Wheel delta: take from egui input, but only apply it when hovered.
            let wheel_y_points = if hovered {
                ctx.input(|i| i.raw_scroll_delta.y)
            } else {
                0.0
            };
            // Normalize wheel delta.
            // On Windows mouse wheels typically report 120 units per notch; touchpads may report smaller deltas.
            // Keeping this normalization here prevents the camera from "teleporting" on high-res wheels.
            let wheel_y = (wheel_y_points / 120.0).clamp(-12.0, 12.0);

            self.viewport_bridge
                .publish_orbit_input(dx_px, dy_px, wheel_y, hovered, dragging);

            // --- Movement keys (UI -> render) ---
            // Only publish when viewport is hovered and UI is not capturing keyboard for text input.
            let wants_kb = ctx.wants_keyboard_input();
            let mut move_mask: u64 = 0;
            if hovered && !wants_kb {
                ctx.input(|i| {
                    if i.key_down(egui::Key::W) { move_mask |= 1 << 0; }
                    if i.key_down(egui::Key::A) { move_mask |= 1 << 1; }
                    if i.key_down(egui::Key::S) { move_mask |= 1 << 2; }
                    if i.key_down(egui::Key::D) { move_mask |= 1 << 3; }
                    if i.key_down(egui::Key::Q) { move_mask |= 1 << 4; }
                    if i.key_down(egui::Key::E) { move_mask |= 1 << 5; }
                    if i.modifiers.shift { move_mask |= 1 << 6; }
                });
            }
            self.viewport_bridge.publish_move_keys(move_mask);

            // Read current external texture id (published by renderer).
            let tex_user = self.viewport_bridge.read_tex_user();

            // Draw the rendered texture if available.
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

    fn ui_plugins(&mut self, ctx: &egui::Context) {
        if !self.plugins_open {
            return;
        }

        let snap = self.plugins_bridge.read();

        egui::Window::new("Plugin Manager")
            .open(&mut self.plugins_open)
            .resizable(true)
            .vscroll(false)
            .show(ctx, |ui| {
                ui.label(format!("loaded: {}", snap.plugins.len()));
                ui.add_space(8.0);

                ui.columns(2, |cols| {
                    // Left: list
                    cols[0].heading("Plugins");
                    cols[0].add_space(6.0);

                    egui::ScrollArea::vertical()
                        .id_source("plugin_list")
                        .max_height(460.0)
                        .show(&mut cols[0], |ui| {
                            for p in snap.plugins.iter() {
                                let label = format!("{}  ({})", p.name, p.id);
                                let selected = self
                                    .selected_plugin
                                    .as_deref()
                                    .map(|s| s == p.id)
                                    .unwrap_or(false);

                                let resp = ui.selectable_label(selected, label);
                                if resp.clicked() {
                                    self.selected_plugin = Some(p.id.clone());
                                }

                                ui.label(format!("{}  ·  {}", p.version, p.state));
                                if let Some(reason) = p.disabled_reason.as_deref() {
                                    ui.label(format!("disabled: {reason}"));
                                }
                                ui.add_space(6.0);
                                ui.separator();
                            }
                        });

                    // Right: details
                    cols[1].heading("Details");
                    cols[1].add_space(6.0);

                    let selected = self
                        .selected_plugin
                        .as_deref()
                        .and_then(|id| snap.plugins.iter().find(|p| p.id == id));

                    if let Some(p) = selected {
                        cols[1].label(format!("id: {}", p.id));
                        cols[1].label(format!("name: {}", p.name));
                        cols[1].label(format!("version: {}", p.version));
                        cols[1].label(format!("state: {}", p.state));
                        cols[1].label(format!("path: {}", p.path.display()));

                        if let Some(k) = p.kind {
                            cols[1].label(format!("kind: {k:?}"));
                        } else {
                            cols[1].label("kind: <v1 plugin>");
                        }

                        if let Some(reason) = p.disabled_reason.as_deref() {
                            cols[1].add_space(6.0);
                            cols[1].label(format!("disabled_reason: {reason}"));
                        }

                        cols[1].add_space(10.0);
                        cols[1].heading("Capabilities");
                        cols[1].add_space(6.0);

                        egui::ScrollArea::vertical()
                            .id_source("plugin_caps")
                            .max_height(360.0)
                            .show(&mut cols[1], |ui| {
                                if p.capabilities.is_empty() {
                                    ui.label("<none>");
                                    return;
                                }

                                for c in p.capabilities.iter() {
                                    ui.group(|ui| {
                                        ui.label(format!("id: {}", c.id));
                                        ui.label(format!("role: {:?}", c.role));
                                        ui.label(format!("kind: {:?}", c.kind));
                                        ui.label(format!("version: {}", c.version));
                                        if !c.describe_json.is_empty() {
                                            ui.label(format!("describe_json: {}", c.describe_json));
                                        }
                                    });
                                    ui.add_space(6.0);
                                }
                            });
                    } else {
                        cols[1].label("Select a plugin on the left.");
                    }
                });
            });
    }
}

impl UiBuildFn for EditorUiBuild {
    fn build(&mut self, ctx_any: &mut dyn Any) {
        let Some(ctx) = ctx_any.downcast_mut::<egui::Context>() else {
            return;
        };

        // Keep markup state synced (even if we don't render markup in foundation mode yet).
        let _maybe_doc = { self.shared_doc.lock().ok().and_then(|g| g.as_ref().cloned()) };
        //self.state.begin_frame();

        // Hotkey: F1 toggles console.
        if ctx.input(|i| i.key_pressed(egui::Key::F1)) {
            self.console_open = !self.console_open;
        }

        // Hotkey: F2 toggles plugin manager.
        if ctx.input(|i| i.key_pressed(egui::Key::F2)) {
            self.plugins_open = !self.plugins_open;
        }

        self.ui_topbar(ctx);
        self.ui_viewport(ctx);
        self.ui_console(ctx);
        self.ui_plugins(ctx);

        //self.state.end_frame();
    }
}