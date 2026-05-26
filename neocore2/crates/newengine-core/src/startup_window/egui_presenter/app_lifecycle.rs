use super::*;

impl PreStartApp {
    pub(super) fn new(cc: &eframe::CreationContext<'_>, config_path: PathBuf, outcome: Arc<Mutex<WindowOutcome>>) -> Self {
        let (config, parse_warning) = read_config(&config_path);
        let plugin_tabs = collect_plugin_tabs(&config);
        let selected_plugin = plugin_tabs
            .iter()
            .find(|tab| tab.plugin_id != "newengine.engine" && tab.enabled)
            .map(|tab| tab.plugin_id.clone());
        let svg_icons = SvgIconRegistry::load(&config_path, &config);
        let icon_status = svg_icons.source_label();
        let mut app = Self {
            config_path,
            config,
            parse_warning,
            selected_tab: 0,
            selected_plugin,
            fields: FieldStore::default(),
            plugin_tabs,
            status: format!("Ready to launch. {icon_status}. Closing this window will NOT start the engine."),
            style_ready: false,
            svg_icons,
            center_attempts: 0,
            outcome,
        };
        app.seed_builtin_fields();
        app.seed_plugin_fields();
        app.apply_style(&cc.egui_ctx);
        app
    }

    pub(super) fn set_outcome(&self, outcome: WindowOutcome) {
        if let Ok(mut guard) = self.outcome.lock() {
            *guard = outcome;
        }
    }

    pub(super) fn open_project_folder(&mut self) {
        let raw = self.fields.string_value("project.path", ".");
        let path = resolve_project_folder(&self.config_path, &raw);
        match open_folder_in_shell(&path) {
            Ok(()) => {
                self.status = format!("Opened project folder {}", path.display());
            }
            Err(err) => {
                self.status = format!("Open Project Folder failed path='{}' err={err}", path.display());
            }
        }
    }

    pub(super) fn center_window_on_startup(&mut self, ctx: &egui::Context) {
        if self.center_attempts >= CENTER_ATTEMPT_LIMIT {
            return;
        }

        self.center_attempts = self.center_attempts.saturating_add(1);
        let target_position = ctx.input(|input| {
            let viewport = input.viewport();
            let monitor_size = viewport.monitor_size?;
            let rect = match (viewport.outer_rect, viewport.inner_rect) {
                (Some(rect), _) | (None, Some(rect)) => rect,
                (None, None) => return None,
            };
            let size = rect.size();
            if monitor_size.x <= 0.0 || monitor_size.y <= 0.0 || size.x <= 0.0 || size.y <= 0.0 {
                return None;
            }
            Some(egui::pos2(
                ((monitor_size.x - size.x) * 0.5).max(0.0),
                ((monitor_size.y - size.y) * 0.5).max(0.0),
            ))
        });

        if let Some(position) = target_position {
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(position));
            self.center_attempts = CENTER_ATTEMPT_LIMIT;
        } else {
            ctx.request_repaint();
        }
    }

    pub(super) fn apply_style(&mut self, ctx: &egui::Context) {
        if self.style_ready {
            return;
        }
        let mut style = (*ctx.style()).clone();
        style.visuals = egui::Visuals::dark();
        style.visuals.dark_mode = true;
        style.visuals.window_fill = color32(north_star_bootstrap_ui_style().palette.bg);
        style.visuals.panel_fill = color32(north_star_bootstrap_ui_style().palette.bg_deep);
        style.visuals.extreme_bg_color = color32(north_star_bootstrap_ui_style().palette.bg_deep);
        style.visuals.faint_bg_color = color32(north_star_bootstrap_ui_style().palette.panel);
        style.visuals.override_text_color = Some(TEXT_PRIMARY);
        style.visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(10, 13, 20);
        style.visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, TEXT_MUTED);
        style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(17, 22, 32);
        style.visuals.widgets.inactive.weak_bg_fill = egui::Color32::from_rgb(14, 18, 27);
        style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(44, 56, 77));
        style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, TEXT_PRIMARY);
        style.visuals.widgets.hovered.bg_fill = color32(north_star_bootstrap_ui_style().palette.panel_active);
        style.visuals.widgets.hovered.weak_bg_fill = egui::Color32::from_rgb(20, 28, 43);
        style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, ACCENT_BLUE_BRIGHT);
        style.visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
        style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(31, 58, 102);
        style.visuals.widgets.active.weak_bg_fill = egui::Color32::from_rgb(28, 48, 83);
        style.visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, ACCENT_BLUE_BRIGHT);
        style.visuals.widgets.active.fg_stroke = egui::Stroke::new(1.1, egui::Color32::WHITE);
        style.visuals.selection.bg_fill = color32(north_star_bootstrap_ui_style().palette.blue);
        style.visuals.selection.stroke = egui::Stroke::new(1.0, ACCENT_BLUE_BRIGHT);
        style.visuals.widgets.open.bg_fill = egui::Color32::from_rgb(24, 33, 50);
        style.visuals.widgets.open.weak_bg_fill = egui::Color32::from_rgb(18, 25, 38);
        style.visuals.widgets.open.bg_stroke = egui::Stroke::new(1.0, ACCENT_BLUE_BRIGHT);
        style.spacing.item_spacing = egui::vec2(10.0, 8.0);
        style.spacing.button_padding = egui::vec2(12.0, 9.0);
        style.spacing.indent = 16.0;
        ctx.set_style(style);
        self.style_ready = true;
    }

    pub(super) fn seed_builtin_fields(&mut self) {
        self.fields.string("engine.modules_dir", value_string_segments(&self.config, &["engine", "modules_dir"], "plugins"));
        self.fields.string("engine.cache_files", value_string_segments(&self.config, &["engine", "cache_files"], "cache"));
        self.fields.string("engine.config", value_string_segments(&self.config, &["engine", "config"], "config"));
        self.fields.string("project.name", value_string_segments(&self.config, &["plugins", "newengine", "startup_window", "project", "name"], "MyGameProject"));
        let project_path = self.config_path.parent().map(|path| path.display().to_string()).unwrap_or_else(|| ".".to_owned());
        self.fields.string("project.path", value_string_segments(&self.config, &["plugins", "newengine", "startup_window", "project", "path"], &project_path));
        self.fields.string("launch.parameters", value_string_segments(&self.config, &["plugins", "newengine", "startup_window", "launch_parameters"], "--windowed --devmode --log --console"));
        self.fields.string("launch.startup_scene", value_string_segments(&self.config, &["plugins", "newengine", "startup_window", "startup_scene"], "MainMenu"));
        self.fields.bool("launch.remember_last_options", value_bool_segments(&self.config, &["plugins", "newengine", "startup_window", "remember_last_options"], true));

        let title = first_string_segments(&self.config, &[&["plugins", "newengine", "platform.winit", "title"], &["window", "title"]], "North Star Engine");
        let width = first_i64_segments(&self.config, &[&["plugins", "newengine", "platform.winit", "width"], &["window", "width"]], 1600);
        let height = first_i64_segments(&self.config, &[&["plugins", "newengine", "platform.winit", "height"], &["window", "height"]], 900);

        self.fields.string("display.title", title);
        self.fields.string("display.width", width.to_string());
        self.fields.string("display.height", height.to_string());
        self.fields.string("display.monitor", first_string_segments(&self.config, &[
            &["plugins", "newengine", "platform.winit", "display", "monitor"],
            &["plugins", "newengine", "startup_window", "display", "monitor"],
        ], "primary"));
        let display_mode = display_window_mode_from_config(&self.config);
        let display_fullscreen = display_mode != "windowed";
        let display_borderless = display_mode == "borderless";
        self.fields.select("display.window_mode", display_mode);
        self.fields.bool("display.fullscreen", display_fullscreen);
        self.fields.bool("display.borderless_fullscreen", display_borderless);
        self.fields.bool("display.vsync", first_bool_segments(&self.config, &[
            &["plugins", "newengine", "platform.winit", "display", "vsync"],
            &["plugins", "newengine", "startup_window", "display", "vsync"],
        ], false));
        self.fields.string("display.refresh_rate", first_string_segments(&self.config, &[
            &["plugins", "newengine", "platform.winit", "display", "refresh_rate"],
            &["plugins", "newengine", "startup_window", "display", "refresh_rate"],
        ], "auto"));
        self.fields.string("display.render_scale", first_f64_segments(&self.config, &[
            &["plugins", "newengine", "platform.winit", "display", "render_scale"],
            &["plugins", "newengine", "startup_window", "display", "render_scale"],
        ], 1.0).to_string());
        self.fields.select("display.hdr", first_string_segments(&self.config, &[
            &["plugins", "newengine", "platform.winit", "display", "hdr"],
            &["plugins", "newengine", "startup_window", "display", "hdr"],
        ], "auto"));

        self.fields.select("graphics.renderer_backend", value_string_segments(&self.config, &["plugins", "newengine", "startup_window", "graphics_backend", "renderer_backend"], "auto"));
        self.fields.string("graphics.gpu_device", value_string_segments(&self.config, &["plugins", "newengine", "startup_window", "graphics_backend", "gpu_device"], "auto"));
        self.fields.select("graphics.graphics_profile", value_string_segments(&self.config, &["plugins", "newengine", "startup_window", "graphics_backend", "graphics_profile"], "auto"));
        self.fields.select("graphics.shader_cache_mode", value_string_segments(&self.config, &["plugins", "newengine", "startup_window", "graphics_backend", "shader_cache_mode"], "auto"));
        self.fields.bool("graphics.debug.enabled", value_bool_segments(&self.config, &["plugins", "newengine", "startup_window", "graphics_backend", "debug_renderer_tools", "enabled"], false));
        self.fields.bool("graphics.debug.renderdoc_capture", value_bool_segments(&self.config, &["plugins", "newengine", "startup_window", "graphics_backend", "debug_renderer_tools", "renderdoc_capture"], false));
        self.fields.bool("graphics.debug.phase_viewer", value_bool_segments(&self.config, &["plugins", "newengine", "startup_window", "graphics_backend", "debug_renderer_tools", "phase_viewer"], false));
        self.fields.bool("graphics.debug.target_viewer", value_bool_segments(&self.config, &["plugins", "newengine", "startup_window", "graphics_backend", "debug_renderer_tools", "target_viewer"], false));
        self.fields.bool("graphics.debug.shadow_cascade_viewer", value_bool_segments(&self.config, &["plugins", "newengine", "startup_window", "graphics_backend", "debug_renderer_tools", "shadow_cascade_viewer"], false));
        self.fields.bool("graphics.debug.gbuffer_viewer", value_bool_segments(&self.config, &["plugins", "newengine", "startup_window", "graphics_backend", "debug_renderer_tools", "gbuffer_viewer"], false));
        self.fields.bool("graphics.debug.gpu_timing", value_bool_segments(&self.config, &["plugins", "newengine", "startup_window", "graphics_backend", "debug_renderer_tools", "gpu_timing"], false));
    }

    pub(super) fn seed_plugin_fields(&mut self) {
        for tab in &self.plugin_tabs {
            if tab.plugin_id == "newengine.engine" {
                continue;
            }
            for field in &tab.fields {
                let key = plugin_field_key(&tab.plugin_id, &field.path);
                let current = plugin_field_current(&self.config, &tab.plugin_id, field)
                    .unwrap_or_else(|| Value::String(String::new()));
                match field.kind.as_str() {
                    "bool" => {
                        self.fields.bool(&key, current.as_bool().unwrap_or(false));
                    }
                    "select" => {
                        let value = current_to_string(&current);
                        self.fields.select(&key, value);
                    }
                    _ => {
                        self.fields.string(&key, current_to_string(&current));
                    }
                }
            }
        }
    }

    pub(super) fn set_display_window_mode(&mut self, mode: String) {
        let mode = normalize_window_mode(mode);
        self.fields.selects.insert("display.window_mode".to_owned(), mode.clone());
        self.fields.bools.insert("display.fullscreen".to_owned(), mode != "windowed");
        self.fields.bools.insert("display.borderless_fullscreen".to_owned(), mode == "borderless");
    }
}
