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
///
/// Owns all UI state and renders the window. The editor shell just calls
/// `topbar_button()` and `show()`.
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
            PluginSort::Name => plugins.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id))),
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
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            egui::RichText::new(format!("{}", plugins.len()))
                                                .small(),
                                        );
                                    },
                                );
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
                                            _ => egui::RichText::new(p.state.to_ascii_uppercase())
                                                .strong(),
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
                                                    ui.label(
                                                        egui::RichText::new(&p.id)
                                                            .small()
                                                            .monospace(),
                                                    );
                                                    ui.label(
                                                        egui::RichText::new(format!(
                                                            "{} · {}",
                                                            p.version, kind_txt
                                                        ))
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
                                                ui.label(
                                                    egui::RichText::new(format!(
                                                        "disabled: {reason}"
                                                    ))
                                                        .small(),
                                                );
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
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.button("Copy id").clicked() {
                                            if let Some(p) = selected {
                                                ui.output_mut(|o| o.copied_text = p.id.clone());
                                            }
                                        }
                                    },
                                );
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
                                ui.label(
                                    egui::RichText::new(p.path.display().to_string())
                                        .monospace(),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.button("Copy path").clicked() {
                                            ui.output_mut(|o| {
                                                o.copied_text = p.path.display().to_string()
                                            });
                                        }
                                    },
                                );
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
                                                    ui.label(
                                                        egui::RichText::new("id:").strong(),
                                                    );
                                                    ui.label(
                                                        egui::RichText::new(c.id.to_string())
                                                            .monospace(),
                                                    );
                                                    ui.with_layout(
                                                        egui::Layout::right_to_left(
                                                            egui::Align::Center,
                                                        ),
                                                        |ui| {
                                                            if ui.button("Copy").clicked() {
                                                                ui.output_mut(|o| {
                                                                    o.copied_text = c.id.to_string()
                                                                });
                                                            }
                                                        },
                                                    );
                                                });

                                                ui.label(format!("role: {:?}", c.role));
                                                ui.label(format!("kind: {:?}", c.kind));
                                                ui.label(format!("version: {}", c.version));

                                                if !c.describe_json.is_empty() {
                                                    ui.add_space(6.0);
                                                    ui.label(
                                                        egui::RichText::new("describe_json")
                                                            .strong(),
                                                    );

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
}
