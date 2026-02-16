#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use newengine_platform_winit::egui;
use newengine_ui::markup::{UiEventKind, UiMarkupDoc, UiState};
use newengine_ui::AssetServiceClient;

use super::bridge::PluginManagerBridge;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PluginSort {
    Name,
    Id,
    State,
}

/// Plugin Manager UI: markup-driven.
///
/// Goals:
/// - no per-button hardcode in egui layer
/// - styling + icons come from markup + assets
/// - deterministic: apply actions next frame via bridge queue
pub struct PluginManagerUi {
    bridge: Arc<PluginManagerBridge>,

    open: bool,

    doc: Option<UiMarkupDoc>,
    state: UiState,

    selected_plugin: Option<String>,

    sort: PluginSort,
}

impl PluginManagerUi {
    #[inline]
    pub fn new(bridge: Arc<PluginManagerBridge>) -> Self {
        let mut state = UiState::default();

        // Asset service is required for icon loading in markup.
        state.set_var(AssetServiceClient::new(
            newengine_core::plugins::default_host_api(),
        ));

        // Defaults (must match ids/binds used in ui/plugin_manager.xml).
        state.set_var("pm.search", "");
        state.set_var("pm.show_disabled", "true");
        state.set_var("pm.sort", "name");
        state.set_var("pm.load_path", "");
        state.set_var("pm.can_load", "false");

        Self {
            bridge,
            open: false,
            doc: None,
            state,
            selected_plugin: None,
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

    fn ensure_doc_loaded(&mut self) {
        if self.doc.is_some() {
            return;
        }

        let assets = AssetServiceClient::new(newengine_core::plugins::default_host_api());
        if let Ok(doc) = UiMarkupDoc::load(&assets, "ui/plugin_manager.xml", Duration::from_millis(150)) {
            self.doc = Some(doc);
        }
    }

    fn sync_inputs_from_vars(&mut self) {
        let sort = self
            .state
            .vars
            .get("pm.sort")
            .map(|s| s.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "name".to_string());

        self.sort = match sort.as_str() {
            "id" => PluginSort::Id,
            "state" => PluginSort::State,
            _ => PluginSort::Name,
        };
    }

    fn apply_events(&mut self, ctx: &egui::Context) {
        let events = self.state.drain_events();
        for ev in events {
            // For now we only care about clicks. TextBox/Checkbox/Select mutate vars directly.
            if ev.kind != UiEventKind::Click {
                continue;
            }

            for act in ev.actions.iter() {
                if act == "pm.close" {
                    self.open = false;
                    continue;
                }

                if act == "pm.rescan" {
                    self.bridge
                        .push_cmd(newengine_core::plugins::PluginControlCommand::Rescan);
                    continue;
                }

                if let Some(id) = act.strip_prefix("pm.select:") {
                    let id = id.trim();
                    if !id.is_empty() {
                        self.selected_plugin = Some(id.to_string());
                        self.state.set_var("pm.selected_id", id);
                    }
                    continue;
                }

                if act == "pm.copy_id" {
                    if let Some(id) = self.selected_plugin.as_deref() {
                        ctx.copy_text(id.to_string());
                    }
                    continue;
                }

                if act == "pm.copy_path" {
                    let snap = self.bridge.read();
                    if let Some(id) = self.selected_plugin.as_deref() {
                        if let Some(p) = snap.plugins.iter().find(|p| p.id == id) {
                            ctx.copy_text(p.path.display().to_string());
                        }
                    }
                    continue;
                }

                if act == "pm.load" {
                    let p = self
                        .state
                        .vars
                        .get("pm.load_path")
                        .map(|s| s.trim())
                        .unwrap_or("");
                    if !p.is_empty() {
                        self.bridge.push_cmd(newengine_core::plugins::PluginControlCommand::LoadPath(
                            PathBuf::from(p),
                        ));
                    }
                    continue;
                }

                if let Some(cmd) = act.strip_prefix("pm.cmd:") {
                    let cmd = cmd.trim();
                    let Some(id) = self.selected_plugin.as_deref() else {
                        continue;
                    };

                    match cmd {
                        "start" => self.bridge.push_cmd(
                            newengine_core::plugins::PluginControlCommand::Start(id.to_string()),
                        ),
                        "stop" => self.bridge.push_cmd(
                            newengine_core::plugins::PluginControlCommand::Stop(id.to_string()),
                        ),
                        "enable" => self.bridge.push_cmd(
                            newengine_core::plugins::PluginControlCommand::Enable(id.to_string()),
                        ),
                        "disable" => self.bridge.push_cmd(
                            newengine_core::plugins::PluginControlCommand::Disable(id.to_string()),
                        ),
                        "reload" => self.bridge.push_cmd(
                            newengine_core::plugins::PluginControlCommand::Reload(id.to_string()),
                        ),
                        _ => {}
                    }

                    continue;
                }
            }
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        if !self.open {
            return;
        }

        self.ensure_doc_loaded();
        self.sync_inputs_from_vars();

        let snap = self.bridge.read();

        let q = self
            .state
            .vars
            .get("pm.search")
            .map(|s| s.trim().to_ascii_lowercase())
            .unwrap_or_default();

        let show_disabled = self
            .state
            .vars
            .get("pm.show_disabled")
            .map(|s| {
                let v = s.trim().to_ascii_lowercase();
                v == "1" || v == "true" || v == "yes" || v == "on"
            })
            .unwrap_or(true);

        let mut plugins: Vec<_> = snap
            .plugins
            .iter()
            .filter(|p| {
                if !show_disabled && p.state == "disabled" {
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

        let selected = self
            .selected_plugin
            .as_deref()
            .and_then(|id| snap.plugins.iter().find(|p| p.id == id))
            .or_else(|| plugins.first().copied());

        if self.selected_plugin.is_none() {
            self.selected_plugin = selected.map(|p| p.id.clone());
        }
        if let Some(id) = self.selected_plugin.as_deref() {
            self.state.set_var("pm.selected_id", id);
        }

        let total = snap.plugins.len();
        let running = snap.plugins.iter().filter(|p| p.state == "running").count();
        let registered = snap.plugins.iter().filter(|p| p.state == "registered").count();
        let stopped = snap.plugins.iter().filter(|p| p.state == "stopped").count();
        let disabled = snap.plugins.iter().filter(|p| p.state == "disabled").count();

        self.state.set_var(
            "pm.stats",
            format!(
                "{running} running · {registered} registered · {stopped} stopped · {disabled} disabled · {total} total"
            ),
        );

        // Load path: update can_load.
        let load_path = self
            .state
            .vars
            .get("pm.load_path")
            .map(|s| s.trim())
            .unwrap_or("");
        self.state
            .set_var("pm.can_load", if load_path.is_empty() { "false" } else { "true" });

        // Plugin list JSON for <repeat>.
        // Deterministic order is already applied above.
        let selected_id = self.selected_plugin.clone().unwrap_or_default();
        let mut arr = Vec::with_capacity(plugins.len());
        for p in plugins.iter() {
            let row_class = if p.id == selected_id { "row selected" } else { "row" };
            arr.push(serde_json::json!({
                "id": p.id,
                "name": p.name,
                "version": p.version,
                "state": p.state,
                "kind": p.kind.map(|k| format!("{k:?}")).unwrap_or_else(|| "v1".to_string()),
                "path": p.path.display().to_string(),
                "row_class": row_class,
            }));
        }
        self.state
            .set_var("pm.plugins_json", serde_json::Value::Array(arr).to_string());

        // Selected details.
        if let Some(p) = selected {
            self.state.set_var("pm.sel.name", p.name.clone());
            self.state.set_var("pm.sel.id", p.id.clone());
            self.state.set_var("pm.sel.version", p.version.clone());
            self.state.set_var(
                "pm.sel.kind",
                p.kind
                    .map(|k| format!("{k:?}"))
                    .unwrap_or_else(|| "v1".to_string()),
            );
            self.state.set_var("pm.sel.state", p.state.clone());
            self.state
                .set_var("pm.sel.path", p.path.display().to_string());

            let dis = p
                .disabled_reason
                .clone()
                .unwrap_or_else(|| "".to_string());
            self.state.set_var("pm.sel.disabled_reason", dis);
        } else {
            self.state.set_var("pm.sel.name", "<none>");
            self.state.set_var("pm.sel.id", "");
            self.state.set_var("pm.sel.version", "");
            self.state.set_var("pm.sel.kind", "");
            self.state.set_var("pm.sel.state", "");
            self.state.set_var("pm.sel.path", "");
            self.state.set_var("pm.sel.disabled_reason", "");
        }

        let Some(doc) = self.doc.as_ref() else {
            egui::Window::new("Plugin Manager")
                .open(&mut self.open)
                .resizable(true)
                .min_width(860.0)
                .min_height(520.0)
                .show(ctx, |ui| {
                    ui.label("ui/plugin_manager.xml not found");
                });
            return;
        };

        // The document itself describes the window + layout.
        newengine_ui::markup::render_egui(doc, ctx, &mut self.state);

        self.apply_events(ctx);
    }
}
