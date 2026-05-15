#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::collections_prelude::NeHashMap as HashMap;
use std::sync::Arc;

use egui;
use newengine_assets::{AssetAccess, AssetServiceClient, AssetState};
use newengine_plugin_host::{PluginControlCommand, PluginSnapshotEntry};

use super::bridge::PluginManagerBridge;

const DEFAULT_PLUGIN_ICON_PATH: &str = "ui/plugin_icons/plugin_icons.neytd@default_plugin_icon";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PluginSort {
    Name,
    Id,
    State,
}

enum AssetIconSlot {
    Loading {
        path: String,
        id_hex32: String,
    },
    Ready {
        texture: egui::TextureHandle,
    },
    Failed {
        path: String,
        error: String,
    },
}

struct AssetPluginIconCache {
    assets: AssetServiceClient,
    slots: HashMap<String, AssetIconSlot>,
}

impl AssetPluginIconCache {
    #[inline]
    fn new() -> Self {
        Self {
            assets: AssetServiceClient::new(newengine_plugin_host::default_host_api()),
            slots: HashMap::default(),
        }
    }

    fn texture_id(
        &mut self,
        ctx: &egui::Context,
        key: impl Into<String>,
        path: impl Into<String>,
    ) -> Option<egui::TextureId> {
        let key = key.into();
        let path = path.into();
        self.assets.pump();

        if !self.slots.contains_key(&key) {
            match self.assets.import_v1(&path) {
                Ok(id_hex32) => {
                    log::debug!(
                        "plugin icon asset: requested key='{}' path='{}' id='{}'",
                        key,
                        path,
                        id_hex32
                    );
                    self.slots.insert(key.clone(), AssetIconSlot::Loading { path, id_hex32 });
                }
                Err(e) => {
                    log::warn!(
                        "plugin icon asset: request failed key='{}' path='{}' err='{}'",
                        key,
                        path,
                        e
                    );
                    self.slots.insert(key.clone(), AssetIconSlot::Failed { path, error: e });
                }
            }
        }

        let mut replacement = None;
        if let Some(slot) = self.slots.get_mut(&key) {
            match slot {
                AssetIconSlot::Ready { texture } => return Some(texture.id()),
                AssetIconSlot::Failed { path, error } => {
                    let _ = (path, error);
                    return None;
                }
                AssetIconSlot::Loading { path, id_hex32 } => match self.assets.state(id_hex32) {
                    Ok(AssetState::Ready) => match self.assets.texture_rgba8_v1(id_hex32) {
                        Ok(texture_asset) => match rgba8_icon_image(
                            texture_asset.width,
                            texture_asset.height,
                            &texture_asset.rgba,
                        ) {
                            Ok((img, w, h)) => {
                                let digest_hex = blake3::hash(&texture_asset.rgba)
                                    .to_hex()
                                    .to_string();
                                let texture = ctx.load_texture(
                                    format!("plugin_icon_asset:{}:{}", key, digest_hex),
                                    img,
                                    egui::TextureOptions::LINEAR,
                                );
                                log::debug!(
                                    "plugin icon asset: ready key='{}' path='{}' size={}x{} rgba8_bytes={} source='AssetManager.texture_rgba8_v1'",
                                    key,
                                    path,
                                    w,
                                    h,
                                    texture_asset.rgba.len()
                                );
                                replacement = Some(AssetIconSlot::Ready { texture });
                            }
                            Err(e) => {
                                log::warn!(
                                    "plugin icon asset: rgba8 convert failed key='{}' path='{}' err='{}'",
                                    key,
                                    path,
                                    e
                                );
                                replacement = Some(AssetIconSlot::Failed {
                                    path: path.clone(),
                                    error: e,
                                });
                            }
                        },
                        Err(e) => {
                            log::warn!(
                                "plugin icon asset: texture_rgba8_v1 failed key='{}' path='{}' id='{}' err='{}'",
                                key,
                                path,
                                id_hex32,
                                e
                            );
                            replacement = Some(AssetIconSlot::Failed {
                                path: path.clone(),
                                error: e,
                            });
                        }
                    },
                    Ok(AssetState::Failed) => {
                        log::debug!(
                            "plugin icon asset: asset failed key='{}' path='{}' id='{}'",
                            key,
                            path,
                            id_hex32
                        );
                        replacement = Some(AssetIconSlot::Failed {
                            path: path.clone(),
                            error: "asset failed".to_owned(),
                        });
                    }
                    Ok(AssetState::Loading | AssetState::Unloaded | AssetState::Unknown) => {}
                    Err(e) => {
                        log::warn!(
                            "plugin icon asset: state failed key='{}' path='{}' id='{}' err='{}'",
                            key,
                            path,
                            id_hex32,
                            e
                        );
                        replacement = Some(AssetIconSlot::Failed {
                            path: path.clone(),
                            error: e,
                        });
                    }
                },
            }
        }

        if let Some(new_slot) = replacement {
            self.slots.insert(key.clone(), new_slot);
            if let Some(AssetIconSlot::Ready { texture }) = self.slots.get(&key) {
                return Some(texture.id());
            }
        }

        None
    }
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
    icon_cache: AssetPluginIconCache,
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
            icon_cache: AssetPluginIconCache::new(),
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
            ui.label(plugin_icon_asset_path(&p.id));
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
        let plugin_path = plugin_icon_asset_path(&p.id);
        self.icon_cache
            .texture_id(ctx, format!("plugin:{}", p.id), plugin_path)
            .or_else(|| {
                self.icon_cache
                    .texture_id(ctx, "plugin:default", DEFAULT_PLUGIN_ICON_PATH)
            })
    }


}

fn plugin_icon_asset_path(plugin_id: &str) -> String {
    let sanitized = plugin_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>()
        .to_ascii_lowercase();
    format!("ui/plugin_icons/plugin_icons.neytd@{sanitized}")
}

fn rgba8_icon_image(
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<(egui::ColorImage, u32, u32), String> {
    if width == 0 || height == 0 {
        return Err(format!("rgba8 texture has zero extent {width}x{height}"));
    }
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|px| px.checked_mul(4))
        .ok_or_else(|| "rgba8 texture dimensions overflow".to_string())?;
    if rgba.len() != expected {
        return Err(format!(
            "rgba8 payload size mismatch bytes={} expected={} extent={}x{}",
            rgba.len(),
            expected,
            width,
            height
        ));
    }

    let mut pixels = Vec::with_capacity((width as usize).saturating_mul(height as usize));
    for px in rgba.chunks_exact(4) {
        pixels.push(egui::Color32::from_rgba_unmultiplied(
            px[0], px[1], px[2], px[3],
        ));
    }

    Ok((
        egui::ColorImage {
            size: [width as usize, height as usize],
            source_size: Default::default(),
            pixels,
        },
        width,
        height,
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
