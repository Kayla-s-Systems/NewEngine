#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;

use newengine_platform_winit::egui;

use super::bridge::PluginManagerBridge;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PluginSort {
    Name,
    Id,
    State,
}

/// Plugin Manager UI state + renderer.
pub struct PluginManagerUi {
    bridge: Arc<PluginManagerBridge>,

    open: bool,
    selected_plugin: Option<String>,

    search: String,
    show_disabled: bool,
    sort: PluginSort,
}

impl PluginManagerUi {
    #[inline]
    pub fn new(bridge: Arc<PluginManagerBridge>) -> Self {
        Self {
            bridge,
            open: false,
            selected_plugin: None,
            search: String::new(),
            show_disabled: true,
            sort: PluginSort::Name,
        }
    }

    #[inline]
    pub fn is_open(&self) -> bool {
        self.open
    }

    #[inline]
    pub fn toggle(&mut self) {
        self.open = !self.open;
    }

    #[inline]
    pub fn topbar_button(&mut self, ui: &mut egui::Ui) {
        if ui.button("Plugins").clicked() {
            self.toggle();
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        if !self.open {
            return;
        }

        let snap = self.bridge.read();
        let q = self.search.trim().to_ascii_lowercase();

        let mut plugins: Vec<_> = snap
            .plugins
            .iter()
            .filter(|p| {
                if !self.show_disabled && p.state == "disabled" {
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

        match self.sort {
            PluginSort::Name => {
                plugins.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)))
            }
            PluginSort::Id => plugins.sort_by(|a, b| a.id.cmp(&b.id)),
            PluginSort::State => {
                plugins.sort_by(|a, b| a.state.cmp(&b.state).then(a.id.cmp(&b.id)))
            }
        }

        let total = snap.plugins.len();
        let running = snap.plugins.iter().filter(|p| p.state == "running").count();
        let registered = snap.plugins.iter().filter(|p| p.state == "registered").count();
        let stopped = snap.plugins.iter().filter(|p| p.state == "stopped").count();
        let disabled = snap.plugins.iter().filter(|p| p.state == "disabled").count();

        let selected = self
            .selected_plugin
            .as_deref()
            .and_then(|id| snap.plugins.iter().find(|p| p.id == id))
            .or_else(|| plugins.first().copied());

        if self.selected_plugin.is_none() {
            self.selected_plugin = selected.map(|p| p.id.clone());
        }

        egui::Window::new("Plugin Manager")
            .open(&mut self.open)
            .resizable(true)
            .min_width(860.0)
            .min_height(520.0)
            .vscroll(false)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.search)
                                .hint_text("Search: id / name / version / kind…")
                                .desired_width(320.0),
                        );

                        ui.separator();

                        egui::ComboBox::from_id_source("plugins_sort")
                            .selected_text(match self.sort {
                                PluginSort::Name => "Sort: Name",
                                PluginSort::Id => "Sort: Id",
                                PluginSort::State => "Sort: State",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.sort, PluginSort::Name, "Name");
                                ui.selectable_value(&mut self.sort, PluginSort::Id, "Id");
                                ui.selectable_value(&mut self.sort, PluginSort::State, "State");
                            });

                        ui.checkbox(&mut self.show_disabled, "Show disabled");

                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{running} running · {registered} registered · {stopped} stopped · {disabled} disabled · {total} total"
                                    ))
                                        .small(),
                                );
                            },
                        );
                    });

                    ui.add_space(6.0);
                    ui.separator();
                });

                ui.add_space(8.0);

                ui.columns(2, |cols| {
                    cols[0].set_min_width(340.0);

                    egui::Frame::group(cols[0].style())
                        .inner_margin(egui::Margin::same(10))
                        .show(&mut cols[0], |ui| {
                            ui.label(egui::RichText::new("Plugins").strong());

                            egui::ScrollArea::vertical()
                                .id_source("plugin_list")
                                .auto_shrink([false; 2])
                                .show(ui, |ui| {
                                    for p in plugins.iter() {
                                        let selected_flag = self
                                            .selected_plugin
                                            .as_deref()
                                            .map(|s| s == p.id)
                                            .unwrap_or(false);

                                        if ui
                                            .selectable_label(selected_flag, &p.name)
                                            .clicked()
                                        {
                                            self.selected_plugin = Some(p.id.clone());
                                        }
                                    }
                                });
                        });

                    egui::Frame::group(cols[1].style())
                        .inner_margin(egui::Margin::same(12))
                        .show(&mut cols[1], |ui| {
                            let Some(p) = selected else {
                                ui.label("Select a plugin.");
                                return;
                            };

                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(&p.name).strong());

                                if ui.button("Copy id").clicked() {
                                    ctx.copy_text(p.id.clone());
                                }
                            });

                            ui.label(egui::RichText::new(&p.id).monospace());

                            ui.separator();

                            ui.horizontal(|ui| {
                                ui.label("Version:");
                                ui.label(&p.version);
                            });

                            ui.horizontal(|ui| {
                                ui.label("State:");
                                ui.label(&p.state);
                            });

                            ui.horizontal(|ui| {
                                ui.label("Path:");
                                ui.label(
                                    egui::RichText::new(p.path.display().to_string())
                                        .monospace(),
                                );

                                if ui.button("Copy path").clicked() {
                                    ctx.copy_text(p.path.display().to_string());
                                }
                            });

                            if !p.capabilities.is_empty() {
                                ui.separator();
                                ui.label(egui::RichText::new("Capabilities").strong());

                                for c in &p.capabilities {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new(c.id.to_string()).monospace(),
                                        );

                                        if ui.button("Copy").clicked() {
                                            ctx.copy_text(c.id.to_string());
                                        }
                                    });
                                }
                            }
                        });
                });
            });
    }
}