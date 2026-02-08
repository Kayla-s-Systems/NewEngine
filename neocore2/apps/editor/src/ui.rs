use newengine_platform_winit::{egui, UiBuildFn};
use newengine_ui::markup::{UiMarkupDoc, UiState};
use std::any::Any;
use std::sync::{Arc, Mutex};

use newengine_scene::Scene;
use newengine_viewport::ViewportState;

/// Minimal editor UI: foundation-first.
///
/// - Viewport is the only primary panel.
/// - Console is hidden by default.
pub struct EditorUiBuild {
    shared_doc: Arc<Mutex<Option<UiMarkupDoc>>>,
    state: UiState,

    scene: Scene,
    viewport: ViewportState,

    console_open: bool,
    console_input: String,
}

impl EditorUiBuild {
    #[inline]
    pub fn new(shared_doc: Arc<Mutex<Option<UiMarkupDoc>>>) -> Self {
        let scene = Scene::demo();
        let viewport = ViewportState::new(scene.active_camera());

        Self {
            shared_doc,
            state: UiState::default(),
            scene,
            viewport,
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
            let w = avail.x.max(1.0) as u32;
            let h = avail.y.max(1.0) as u32;
            self.viewport.set_pixel_extent(w, h);

            egui::Frame::none()
                .fill(egui::Color32::from_rgb(12, 12, 14))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(45)))
                .show(ui, |ui| {
                    ui.set_min_size(avail);
                    ui.centered_and_justified(|ui| {
                        ui.label("Viewport render target is not wired yet.");
                    });
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

        self.ui_topbar(ctx);
        self.ui_viewport(ctx);
        self.ui_console(ctx);

        //self.state.end_frame();
    }
}
