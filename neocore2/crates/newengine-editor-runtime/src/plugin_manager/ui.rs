#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::HashMap;
use std::sync::Arc;

use egui;
use newengine_plugin_host::{PluginControlCommand, PluginSnapshotEntry};

use super::bridge::PluginManagerBridge;

static DEFAULT_PLUGIN_ICON_PNG: &[u8] = include_bytes!("../../../../apps/editor/assets/plugin_icons/default_plugin_icon.png");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PluginSort {
    Name,
    Id,
    State,
}

#[derive(Clone)]
struct CachedPluginIcon {
    digest_hex: String,
    texture: egui::TextureHandle,
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
    icon_cache: HashMap<String, CachedPluginIcon>,
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
            icon_cache: HashMap::new(),
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
                    "{} {} {} {} {}",
                    p.id,
                    p.name,
                    p.version,
                    p.kind
                        .map(|k| format!("{k:?}"))
                        .unwrap_or_else(|| "v1".to_string()),
                    summarize_plugin(p),
                )
                    .to_ascii_lowercase();

                hay.contains(&q)
            })
            .collect();

        match self.sort {
            PluginSort::Name => plugins.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id))),
            PluginSort::Id => plugins.sort_by(|a, b| a.id.cmp(&b.id)),
            PluginSort::State => plugins.sort_by(|a, b| a.state.cmp(&b.state).then(a.id.cmp(&b.id))),
        }

        let total = snap.plugins.len();
        let running = snap.plugins.iter().filter(|p| p.state == "running").count();
        let registered = snap.plugins.iter().filter(|p| p.state == "registered").count();
        let stopped = snap.plugins.iter().filter(|p| p.state == "stopped").count();
        let disabled = snap.plugins.iter().filter(|p| p.state == "disabled").count();

        if self
            .selected_plugin
            .as_deref()
            .is_some_and(|id| !snap.plugins.iter().any(|p| p.id == id))
        {
            self.selected_plugin = None;
        }

        if self.selected_plugin.is_none() {
            self.selected_plugin = plugins.first().map(|p| p.id.clone());
        }

        let selected = self
            .selected_plugin
            .as_deref()
            .and_then(|id| snap.plugins.iter().find(|p| p.id == id));

        let mut open = self.open;

        egui::Window::new("Plugin Manager")
            .open(&mut open)
            .resizable(true)
            .min_width(1120.0)
            .min_height(620.0)
            .vscroll(false)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.search)
                                .hint_text("Search: id / name / version / state / kind…")
                                .desired_width(360.0),
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

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                egui::RichText::new(format!(
                                    "{running} running · {registered} registered · {stopped} stopped · {disabled} disabled · {total} total"
                                ))
                                    .small(),
                            );
                        });
                    });

                    ui.add_space(8.0);

                    egui::Frame::group(ui.style())
                        .inner_margin(egui::Margin::same(10))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                if ui.button("Rescan folder").clicked() {
                                    self.bridge.push_cmd(PluginControlCommand::Rescan);
                                }

                                ui.separator();

                                ui.add(
                                    egui::TextEdit::singleline(&mut self.load_path)
                                        .hint_text("Load plugin by path (dll/so/dylib)…")
                                        .desired_width(420.0),
                                );

                                if ui
                                    .add_enabled(
                                        !self.load_path.trim().is_empty(),
                                        egui::Button::new("Load"),
                                    )
                                    .clicked()
                                {
                                    self.bridge.push_cmd(PluginControlCommand::LoadPath(
                                        std::path::PathBuf::from(self.load_path.trim()),
                                    ));
                                }

                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(
                                        egui::RichText::new(
                                            "Commands are queued and applied on the next frame.",
                                        )
                                            .small()
                                            .weak(),
                                    );
                                });
                            });
                        });
                });

                ui.add_space(10.0);

                ui.columns(2, |cols| {
                    cols[0].set_min_width(700.0);

                    egui::Frame::group(cols[0].style())
                        .inner_margin(egui::Margin::same(10))
                        .show(&mut cols[0], |ui| {
                            ui.label(egui::RichText::new("All Plugins").strong());
                            ui.add_space(6.0);

                            egui::ScrollArea::vertical()
                                .id_salt("plugin_list")
                                .auto_shrink([false; 2])
                                .show(ui, |ui| {
                                    for p in plugins.iter() {
                                        self.draw_plugin_card(ui, ctx, p);
                                        ui.add_space(6.0);
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
                            self.draw_details(ui, ctx, p);
                        });
                });
            });

        self.open = open;
    }

    fn draw_plugin_card(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        p: &PluginSnapshotEntry,
    ) {
        let selected = self
            .selected_plugin
            .as_deref()
            .map(|id| id == p.id)
            .unwrap_or(false);

        let fill = if selected {
            ui.visuals().selection.bg_fill.gamma_multiply(0.16)
        } else {
            ui.visuals().faint_bg_color
        };

        let stroke = if selected {
            ui.visuals().selection.stroke
        } else {
            ui.visuals().widgets.noninteractive.bg_stroke
        };

        let response = egui::Frame::group(ui.style())
            .fill(fill)
            .stroke(stroke)
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let mut enabled = p.state != "disabled";
                    if ui.checkbox(&mut enabled, "").changed() {
                        self.bridge.push_cmd(if enabled {
                            PluginControlCommand::EnableId(p.id.clone())
                        } else {
                            PluginControlCommand::DisableId(p.id.clone())
                        });
                    }

                    ui.add_space(4.0);
                    self.draw_icon(ui, ctx, p, 56.0);
                    ui.add_space(6.0);

                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(&p.name).strong().size(20.0));
                        ui.label(
                            egui::RichText::new(summarize_plugin(p))
                                .small()
                                .weak(),
                        );
                        ui.label(egui::RichText::new(&p.id).monospace().small().weak());
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(format!("Version {}", p.version)).strong());
                            ui.label(
                                egui::RichText::new(
                                    p.kind
                                        .map(|k| format!("{k:?}"))
                                        .unwrap_or_else(|| "Legacy/V1".to_owned()),
                                )
                                    .small()
                                    .weak(),
                            );
                            ui.label(status_badge_text(p));
                        });
                    });
                });
            })
            .response;
        let response = ui.interact(
            response.rect,
            ui.make_persistent_id(("plugin_card", &p.id)),
            egui::Sense::click(),
        );

        if response.clicked() {
            self.selected_plugin = Some(p.id.clone());
        }
    }

    fn draw_details(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        p: &PluginSnapshotEntry,
    ) {
        ui.horizontal(|ui| {
            self.draw_icon(ui, ctx, p, 96.0);
            ui.add_space(10.0);
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(&p.name).strong().size(24.0));
                ui.label(egui::RichText::new(&p.id).monospace());
                ui.label(egui::RichText::new(summarize_plugin(p)).small().weak());
            });
        });

        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Version:");
            ui.label(&p.version);
        });

        ui.horizontal(|ui| {
            ui.label("Kind:");
            ui.label(
                p.kind
                    .map(|k| format!("{k:?}"))
                    .unwrap_or_else(|| "Legacy/V1".to_owned()),
            );
        });

        ui.horizontal(|ui| {
            ui.label("State:");
            ui.label(egui::RichText::new(&p.state).strong());
        });

        ui.horizontal(|ui| {
            ui.label("Capabilities:");
            ui.label(p.capabilities.len().to_string());
        });

        ui.horizontal(|ui| {
            ui.label("Icon source:");
            ui.label(if p.icon_small.is_some() { "embedded" } else { "default" });
        });

        if let Some(reason) = p.disabled_reason.as_deref() {
            ui.horizontal_wrapped(|ui| {
                ui.label("Disabled reason:");
                ui.label(egui::RichText::new(reason).strong());
            });
        }

        ui.add_space(8.0);

        egui::Frame::group(ui.style())
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Actions").strong());
                ui.add_space(6.0);

                let is_running = p.state == "running";
                let is_stopped = p.state == "stopped";
                let is_registered = p.state == "registered";
                let is_disabled = p.state == "disabled";

                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(is_registered || is_stopped, egui::Button::new("Start"))
                        .clicked()
                    {
                        self.bridge.push_cmd(PluginControlCommand::StartId(p.id.clone()));
                    }

                    if ui
                        .add_enabled(is_running || is_registered, egui::Button::new("Stop"))
                        .clicked()
                    {
                        self.bridge.push_cmd(PluginControlCommand::StopId(p.id.clone()));
                    }

                    if ui
                        .add_enabled(!is_disabled, egui::Button::new("Disable"))
                        .clicked()
                    {
                        self.bridge.push_cmd(PluginControlCommand::DisableId(p.id.clone()));
                    }

                    if ui
                        .add_enabled(is_disabled, egui::Button::new("Enable"))
                        .clicked()
                    {
                        self.bridge.push_cmd(PluginControlCommand::EnableId(p.id.clone()));
                    }

                    if ui.button("Reload").clicked() {
                        self.bridge.push_cmd(PluginControlCommand::ReloadId(p.id.clone()));
                    }
                });
            });

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label("Path:");
            ui.label(egui::RichText::new(p.path.display().to_string()).monospace());
            if ui.button("Copy path").clicked() {
                ctx.copy_text(p.path.display().to_string());
            }
        });

        if !p.capabilities.is_empty() {
            ui.separator();
            ui.label(egui::RichText::new("Capabilities").strong());

            for c in &p.capabilities {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(c.id.to_string()).monospace());
                    ui.label(egui::RichText::new(format!("{:?} · v{}", c.kind, c.version)).small().weak());
                    if ui.button("Copy").clicked() {
                        ctx.copy_text(c.id.to_string());
                    }
                });
            }
        }
    }

    fn draw_icon(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, p: &PluginSnapshotEntry, size: f32) {
        if let Some(tex_id) = self.icon_texture_id(ctx, p) {
            ui.add(egui::Image::new((tex_id, egui::vec2(size, size))));
        } else {
            ui.allocate_space(egui::vec2(size, size));
        }
    }

    fn icon_texture_id(
        &mut self,
        ctx: &egui::Context,
        p: &PluginSnapshotEntry,
    ) -> Option<egui::TextureId> {
        let bytes = p
            .icon_small
            .as_ref()
            .filter(|icon| icon.media_type.eq_ignore_ascii_case("image/png") && !icon.bytes.is_empty())
            .map(|icon| icon.bytes.as_slice())
            .unwrap_or(DEFAULT_PLUGIN_ICON_PNG);

        let digest_hex = blake3::hash(bytes).to_hex().to_string();

        if let Some(cached) = self.icon_cache.get(&p.id) {
            if cached.digest_hex == digest_hex {
                return Some(cached.texture.id());
            }
        }

        let (img, _, _) = match decode_icon_image(bytes) {
            Ok(v) => v,
            Err(_) => {
                if bytes != DEFAULT_PLUGIN_ICON_PNG {
                    return self.ensure_default_icon(ctx, &p.id);
                }
                return None;
            }
        };

        let texture = ctx.load_texture(
            format!("plugin_icon:{}:{}", p.id, digest_hex),
            img,
            egui::TextureOptions::LINEAR,
        );
        let tex_id = texture.id();
        self.icon_cache.insert(
            p.id.clone(),
            CachedPluginIcon {
                digest_hex,
                texture,
            },
        );
        Some(tex_id)
    }

    fn ensure_default_icon(
        &mut self,
        ctx: &egui::Context,
        cache_key: &str,
    ) -> Option<egui::TextureId> {
        let digest_hex = blake3::hash(DEFAULT_PLUGIN_ICON_PNG).to_hex().to_string();
        if let Some(cached) = self.icon_cache.get(cache_key) {
            if cached.digest_hex == digest_hex {
                return Some(cached.texture.id());
            }
        }

        let (img, _, _) = decode_icon_image(DEFAULT_PLUGIN_ICON_PNG).ok()?;
        let texture = ctx.load_texture(
            format!("plugin_icon:{}:{}", cache_key, digest_hex),
            img,
            egui::TextureOptions::LINEAR,
        );
        let tex_id = texture.id();
        self.icon_cache.insert(
            cache_key.to_owned(),
            CachedPluginIcon {
                digest_hex,
                texture,
            },
        );
        Some(tex_id)
    }
}

fn decode_icon_image(bytes: &[u8]) -> Result<(egui::ColorImage, u32, u32), String> {
    let dyn_img = image::load_from_memory(bytes).map_err(|e| e.to_string())?;
    let rgba = dyn_img.to_rgba8();
    let (w, h) = rgba.dimensions();

    let mut pixels = Vec::with_capacity((w as usize).saturating_mul(h as usize));
    for p in rgba.pixels() {
        let [r, g, b, a] = p.0;
        pixels.push(egui::Color32::from_rgba_unmultiplied(r, g, b, a));
    }

    Ok((
        egui::ColorImage {
            size: [w as usize, h as usize],
            source_size: Default::default(),
            pixels,
        },
        w,
        h,
    ))
}

fn summarize_plugin(p: &PluginSnapshotEntry) -> String {
    let cap_summary = if p.capabilities.is_empty() {
        "No declared capabilities".to_owned()
    } else if p.capabilities.len() == 1 {
        format!("1 declared capability: {}", p.capabilities[0].id)
    } else {
        format!("{} declared capabilities", p.capabilities.len())
    };

    let kind = p
        .kind
        .map(|k| format!("{k:?}"))
        .unwrap_or_else(|| "Legacy/V1".to_owned());

    format!("{kind} plugin · state={} · {cap_summary}", p.state)
}

fn status_badge_text(p: &PluginSnapshotEntry) -> egui::RichText {
    let text = match p.state.as_str() {
        "running" => "Running",
        "registered" => "Registered",
        "stopped" => "Stopped",
        "disabled" => "Disabled",
        _ => p.state.as_str(),
    };
    egui::RichText::new(text).small().strong()
}
