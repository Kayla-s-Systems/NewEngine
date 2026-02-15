#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_platform_winit::egui;
use newengine_ui::markup::{UiMarkupDoc, UiState};
use newengine_ui::UiBuildFn;

use std::any::Any;
use std::sync::{Arc, Mutex};

use newengine_scene::Scene;
use newengine_viewport::ViewportState;

use crate::plugin_manager_bridge::PluginManagerBridge;
use crate::viewport_bridge::ViewportBridge;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PluginSort {
    Name,
    Id,
    State,
}

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

    plugins_search: String,
    plugins_show_disabled: bool,
    plugins_sort: PluginSort,
}

impl EditorUiBuild {
    #[inline]
    pub fn new(
        shared_doc: Arc<Mutex<Option<UiMarkupDoc>>>,
        viewport_bridge: Arc<ViewportBridge>,
        plugins_bridge: Arc<PluginManagerBridge>,
    ) -> Self {
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
            plugins_bridge,
            last_drag_pos: None,
            console_open: false,
            console_input: String::new(),
            plugins_open: false,
            selected_plugin: None,
            plugins_search: String::new(),
            plugins_show_disabled: true,
            plugins_sort: PluginSort::Name,
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
        let q = self.plugins_search.trim().to_ascii_lowercase();

        let mut plugins: Vec<_> = snap
            .plugins
            .iter()
            .filter(|p| {
                if !self.plugins_show_disabled && p.state == "disabled" {
                    return false;
                }
                if q.is_empty() {
                    return true;
                }

                let hay = format!(
                    "{} {} {} {}",
                    p.id,
                    p.name,
                    p.version,
                    p.kind
                        .map(|k| format!("{k:?}"))
                        .unwrap_or_else(|| "v1".to_string())
                )
                    .to_ascii_lowercase();

                hay.contains(&q)
            })
            .collect();

        match self.plugins_sort {
            PluginSort::Name => plugins.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id))),
            PluginSort::Id => plugins.sort_by(|a, b| a.id.cmp(&b.id)),
            PluginSort::State => {
                plugins.sort_by(|a, b| a.state.cmp(&b.state).then(a.id.cmp(&b.id)))
            }
        }

        let total = snap.plugins.len();
        let running = snap.plugins.iter().filter(|p| p.state == "running").count();
        let registered = snap
            .plugins
            .iter()
            .filter(|p| p.state == "registered")
            .count();
        let stopped = snap.plugins.iter().filter(|p| p.state == "stopped").count();
        let disabled = snap
            .plugins
            .iter()
            .filter(|p| p.state == "disabled")
            .count();

        let selected = self
            .selected_plugin
            .as_deref()
            .and_then(|id| snap.plugins.iter().find(|p| p.id == id))
            .or_else(|| plugins.first().copied());

        if self.selected_plugin.is_none() {
            self.selected_plugin = selected.map(|p| p.id.clone());
        }

        egui::Window::new("Plugin Manager")
            .open(&mut self.plugins_open)
            .resizable(true)
            .min_width(860.0)
            .min_height(520.0)
            .vscroll(false)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.plugins_search)
                                .hint_text("Search: id / name / version / kind…")
                                .desired_width(320.0),
                        );

                        ui.separator();

                        egui::ComboBox::from_id_source("plugins_sort")
                            .selected_text(match self.plugins_sort {
                                PluginSort::Name => "Sort: Name",
                                PluginSort::Id => "Sort: Id",
                                PluginSort::State => "Sort: State",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.plugins_sort, PluginSort::Name, "Name");
                                ui.selectable_value(&mut self.plugins_sort, PluginSort::Id, "Id");
                                ui.selectable_value(&mut self.plugins_sort, PluginSort::State, "State");
                            });

                        ui.checkbox(&mut self.plugins_show_disabled, "Show disabled");

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                egui::RichText::new(format!(
                                    "{running} running · {registered} registered · {stopped} stopped · {disabled} disabled · {total} total"
                                ))
                                    .small(),
                            );
                        });
                    });

                    ui.add_space(6.0);
                    ui.separator();
                });

                ui.add_space(8.0);

                ui.columns(2, |cols| {
                    cols[0].set_min_width(340.0);

                    egui::Frame::group(cols[0].style())
                        .inner_margin(egui::Margin::same(10.0))
                        .show(&mut cols[0], |ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("Plugins").strong());
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(egui::RichText::new(format!("{}", plugins.len())).small());
                                });
                            });

                            ui.add_space(6.0);

                            egui::ScrollArea::vertical()
                                .id_source("plugin_list")
                                .auto_shrink([false; 2])
                                .show(ui, |ui| {
                                    for p in plugins.iter() {
                                        let is_selected = self
                                            .selected_plugin
                                            .as_deref()
                                            .map(|s| s == p.id)
                                            .unwrap_or(false);

                                        let state_rt = match p.state.as_str() {
                                            "running" => egui::RichText::new("RUNNING").strong(),
                                            "registered" => egui::RichText::new("READY").strong(),
                                            "stopped" => egui::RichText::new("STOPPED").strong(),
                                            "disabled" => egui::RichText::new("DISABLED").strong(),
                                            _ => egui::RichText::new(p.state.to_ascii_uppercase()).strong(),
                                        };

                                        let kind_txt = p
                                            .kind
                                            .map(|k| format!("{k:?}"))
                                            .unwrap_or_else(|| "V1".to_string());

                                        let resp = ui
                                            .add(egui::SelectableLabel::new(is_selected, ""))
                                            .on_hover_text("Click to inspect");

                                        let rect = resp.rect;
                                        ui.allocate_ui_at_rect(rect.shrink(6.0), |ui| {
                                            ui.horizontal(|ui| {
                                                ui.vertical(|ui| {
                                                    ui.label(egui::RichText::new(&p.name).strong());
                                                    ui.label(egui::RichText::new(&p.id).small().monospace());
                                                    ui.label(
                                                        egui::RichText::new(format!("{} · {}", p.version, kind_txt))
                                                            .small(),
                                                    );
                                                });

                                                ui.with_layout(
                                                    egui::Layout::right_to_left(egui::Align::Min),
                                                    |ui| {
                                                        ui.label(state_rt);
                                                    },
                                                );
                                            });

                                            if let Some(reason) = p.disabled_reason.as_deref() {
                                                ui.add_space(4.0);
                                                ui.label(egui::RichText::new(format!("disabled: {reason}")).small());
                                            }
                                        });

                                        if resp.clicked() {
                                            self.selected_plugin = Some(p.id.clone());
                                        }

                                        ui.add_space(6.0);
                                        ui.separator();
                                    }

                                    if plugins.is_empty() {
                                        ui.label("No plugins match the filter.");
                                    }
                                });
                        });

                    egui::Frame::group(cols[1].style())
                        .inner_margin(egui::Margin::same(12.0))
                        .show(&mut cols[1], |ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("Inspector").strong());
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.button("Copy id").clicked() {
                                        if let Some(p) = selected {
                                            ui.output_mut(|o| o.copied_text = p.id.clone());
                                        }
                                    }
                                });
                            });

                            ui.add_space(8.0);

                            let Some(p) = selected else {
                                ui.label("Select a plugin on the left.");
                                return;
                            };

                            ui.label(egui::RichText::new(&p.name).size(18.0).strong());
                            ui.add_space(2.0);
                            ui.label(egui::RichText::new(&p.id).monospace());
                            ui.add_space(6.0);

                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("Version:").strong());
                                ui.label(&p.version);
                                ui.separator();
                                ui.label(egui::RichText::new("State:").strong());
                                ui.label(&p.state);
                                ui.separator();
                                ui.label(egui::RichText::new("Kind:").strong());
                                ui.label(
                                    p.kind
                                        .map(|k| format!("{k:?}"))
                                        .unwrap_or_else(|| "<v1>".to_string()),
                                );
                            });

                            ui.add_space(6.0);

                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("Path:").strong());
                                ui.label(egui::RichText::new(p.path.display().to_string()).monospace());
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.button("Copy path").clicked() {
                                        ui.output_mut(|o| o.copied_text = p.path.display().to_string());
                                    }
                                });
                            });

                            if let Some(reason) = p.disabled_reason.as_deref() {
                                ui.add_space(8.0);
                                egui::Frame::none()
                                    .fill(ui.visuals().faint_bg_color)
                                    .inner_margin(egui::Margin::same(10.0))
                                    .show(ui, |ui| {
                                        ui.label(egui::RichText::new("Disabled reason").strong());
                                        ui.add_space(4.0);
                                        ui.label(reason);
                                    });
                            }

                            ui.add_space(10.0);
                            ui.separator();
                            ui.add_space(10.0);

                            ui.label(egui::RichText::new("Capabilities").strong());
                            ui.add_space(6.0);

                            if p.capabilities.is_empty() {
                                ui.label("<none>");
                                return;
                            }

                            egui::ScrollArea::vertical()
                                .id_source("plugin_caps")
                                .auto_shrink([false; 2])
                                .show(ui, |ui| {
                                    for (idx, c) in p.capabilities.iter().enumerate() {
                                        let title = format!(
                                            "{} · {:?} · {:?} · v{}",
                                            c.id, c.role, c.kind, c.version
                                        );

                                        egui::CollapsingHeader::new(title)
                                            .id_source(("cap", idx))
                                            .default_open(idx == 0)
                                            .show(ui, |ui| {
                                                ui.horizontal(|ui| {
                                                    ui.label(egui::RichText::new("id:").strong());
                                                    ui.label(egui::RichText::new(c.id.to_string()).monospace());
                                                    ui.with_layout(
                                                        egui::Layout::right_to_left(egui::Align::Center),
                                                        |ui| {
                                                            if ui.button("Copy").clicked() {
                                                                ui.output_mut(|o| o.copied_text = c.id.to_string());
                                                            }
                                                        },
                                                    );
                                                });

                                                ui.label(format!("role: {:?}", c.role));
                                                ui.label(format!("kind: {:?}", c.kind));
                                                ui.label(format!("version: {}", c.version));

                                                if !c.describe_json.is_empty() {
                                                    ui.add_space(6.0);
                                                    ui.label(egui::RichText::new("describe_json").strong());

                                                    let mut json = c.describe_json.to_string();
                                                    ui.add(
                                                        egui::TextEdit::multiline(&mut json)
                                                            .font(egui::TextStyle::Monospace)
                                                            .desired_width(f32::INFINITY)
                                                            .desired_rows(6)
                                                            .lock_focus(true),
                                                    );
                                                }
                                            });

                                        ui.add_space(6.0);
                                    }
                                });
                        });
                });
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
            self.plugins_open = !self.plugins_open;
        }

        self.ui_topbar(ctx);
        self.ui_viewport(ctx);
        self.ui_console(ctx);
        self.ui_plugins(ctx);
    }
}

impl UiBuildFn for EditorUiBuild {
    #[inline]
    fn build(&mut self, ctx_any: &mut dyn Any) {
        self.build_ui(ctx_any);
    }
}
