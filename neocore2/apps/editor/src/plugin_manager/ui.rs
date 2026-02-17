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

    load_path: String,
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

            load_path: String::new(),
        }
    }

    #[inline]
    #[allow(dead_code)]
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

                        egui::ComboBox::from_id_salt("plugins_sort")
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

                    ui.add_space(8.0);

                    // High-level actions.
                    egui::Frame::group(ui.style())
                        .inner_margin(egui::Margin::same(10))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                if ui.button("Rescan folder").clicked() {
                                    self.bridge.push_cmd(newengine_core::plugins::PluginControlCommand::Rescan);
                                }

                                ui.separator();

                                ui.add(
                                    egui::TextEdit::singleline(&mut self.load_path)
                                        .hint_text("Load plugin by path (dll/so/dylib)…")
                                        .desired_width(360.0),
                                );

                                let can_load = !self.load_path.trim().is_empty();
                                if ui.add_enabled(can_load, egui::Button::new("Load"))
                                    .clicked()
                                {
                                    let p = std::path::PathBuf::from(self.load_path.trim());
                                    self.bridge.push_cmd(
                                        newengine_core::plugins::PluginControlCommand::LoadPath(p),
                                    );
                                }

                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            egui::RichText::new(
                                                "Actions are applied on the next frame (deterministic).",
                                            )
                                                .small()
                                                .weak(),
                                        );
                                    },
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
                        .inner_margin(egui::Margin::same(10))
                        .show(&mut cols[0], |ui| {
                            ui.label(egui::RichText::new("Plugins").strong());

                            egui::ScrollArea::vertical()
                                .id_salt("plugin_list")
                                .auto_shrink([false; 2])
                                .show(ui, |ui| {
                                    for p in plugins.iter() {
                                        let selected_flag = self
                                            .selected_plugin
                                            .as_deref()
                                            .map(|s| s == p.id)
                                            .unwrap_or(false);

                                        let label = format!("{}  ·  {}", p.name, p.state);
                                        if ui
                                            .selectable_label(selected_flag, label)
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
                                ui.label(egui::RichText::new(&p.state).strong());
                            });

                            if let Some(reason) = p.disabled_reason.as_deref() {
                                ui.horizontal(|ui| {
                                    ui.label("Disabled reason:");
                                    ui.label(egui::RichText::new(reason).strong());
                                });
                            }

                            ui.add_space(8.0);

                            // Contextual actions for the selected plugin.
                            egui::Frame::group(ui.style())
                                .inner_margin(egui::Margin::same(10))
                                .show(ui, |ui| {
                                    ui.label(egui::RichText::new("Actions").strong());
                                    ui.add_space(6.0);

                                    ui.horizontal_wrapped(|ui| {
                                        let id = p.id.clone();

                                        let is_running = p.state == "running";
                                        let is_stopped = p.state == "stopped";
                                        let is_registered = p.state == "registered";
                                        let is_disabled = p.state == "disabled";

                                        if ui
                                            .add_enabled(
                                                is_registered || is_stopped,
                                                egui::Button::new("Start"),
                                            )
                                            .clicked()
                                        {
                                            self.bridge.push_cmd(
                                                newengine_core::plugins::PluginControlCommand::StartId(
                                                    id.clone(),
                                                ),
                                            );
                                        }

                                        if ui
                                            .add_enabled(is_running || is_registered, egui::Button::new("Stop"))
                                            .clicked()
                                        {
                                            self.bridge.push_cmd(
                                                newengine_core::plugins::PluginControlCommand::StopId(
                                                    id.clone(),
                                                ),
                                            );
                                        }

                                        if ui
                                            .add_enabled(!is_disabled, egui::Button::new("Disable"))
                                            .clicked()
                                        {
                                            self.bridge.push_cmd(
                                                newengine_core::plugins::PluginControlCommand::DisableId(
                                                    id.clone(),
                                                ),
                                            );
                                        }

                                        if ui
                                            .add_enabled(is_disabled, egui::Button::new("Enable"))
                                            .clicked()
                                        {
                                            self.bridge.push_cmd(
                                                newengine_core::plugins::PluginControlCommand::EnableId(
                                                    id.clone(),
                                                ),
                                            );
                                        }

                                        if ui.button("Reload").clicked() {
                                            self.bridge.push_cmd(
                                                newengine_core::plugins::PluginControlCommand::ReloadId(
                                                    id,
                                                ),
                                            );
                                        }
                                    });
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