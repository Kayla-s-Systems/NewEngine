use super::*;

impl PreStartApp {
    pub(super) fn save(&mut self) -> Result<(), String> {
        let mut config = self.config.clone();
        self.apply_engine(&mut config);
        self.apply_display(&mut config);
        self.apply_graphics(&mut config);
        self.apply_launch_options(&mut config);
        self.apply_plugin_tabs(&mut config);
        self.apply_plugin_enabled_states(&mut config);
        let text = serde_json::to_string_pretty(&config)
            .map_err(|e| format!("config serialization failed: {e}"))?;
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("config directory create failed path='{}' err={e}", parent.display()))?;
        }
        fs::write(&self.config_path, format!("{text}\n"))
            .map_err(|e| format!("config write failed path='{}' err={e}", self.config_path.display()))?;
        self.config = config;
        self.status = format!("Saved {}", self.config_path.display());
        Ok(())
    }

    pub(super) fn apply_engine(&self, config: &mut Value) {
        set_segments(config, &["engine", "modules_dir"], Value::String(self.fields.string_value("engine.modules_dir", "plugins")));
        set_segments(config, &["engine", "cache_files"], Value::String(self.fields.string_value("engine.cache_files", "cache")));
        set_segments(config, &["engine", "config"], Value::String(self.fields.string_value("engine.config", "config")));
    }

    pub(super) fn apply_launch_options(&self, config: &mut Value) {
        set_segments(config, &["plugins", "newengine", "startup_window", "project", "name"], Value::String(self.fields.string_value("project.name", "MyGameProject")));
        set_segments(config, &["plugins", "newengine", "startup_window", "project", "path"], Value::String(self.fields.string_value("project.path", ".")));
        set_segments(config, &["plugins", "newengine", "startup_window", "launch_parameters"], Value::String(self.fields.string_value("launch.parameters", "")));
        set_segments(config, &["plugins", "newengine", "startup_window", "startup_scene"], Value::String(self.fields.string_value("launch.startup_scene", "MainMenu")));
        set_segments(config, &["plugins", "newengine", "startup_window", "remember_last_options"], Value::Bool(self.fields.bool_value("launch.remember_last_options", true)));
    }

    pub(super) fn apply_display(&self, config: &mut Value) {
        let title = self.fields.string_value("display.title", "Kayla's Editor");
        let width = parse_i64(&self.fields.string_value("display.width", "1600"), 1600).max(320);
        let height = parse_i64(&self.fields.string_value("display.height", "900"), 900).max(240);
        let mut mode = normalize_window_mode(self.fields.select_value("display.window_mode", "windowed"));
        let quick_fullscreen = self.fields.bool_value("display.fullscreen", false);
        let quick_borderless = self.fields.bool_value("display.borderless_fullscreen", false);
        if quick_borderless {
            mode = "borderless".to_owned();
        } else if quick_fullscreen && mode == "windowed" {
            mode = "exclusive_fullscreen".to_owned();
        }
        let fullscreen = mode != "windowed";
        let borderless = mode == "borderless";
        let vsync = self.fields.bool_value("display.vsync", true);
        let monitor = self.fields.string_value("display.monitor", "primary");
        let refresh_rate = self.fields.string_value("display.refresh_rate", "auto");
        let render_scale = parse_f64(&self.fields.string_value("display.render_scale", "1.0"), 1.0).clamp(0.25, 4.0);
        let hdr = self.fields.select_value("display.hdr", "auto");

        set_segments(config, &["window", "title"], Value::String(title.clone()));
        set_segments(config, &["window", "width"], Value::Number(width.into()));
        set_segments(config, &["window", "height"], Value::Number(height.into()));
        set_segments(config, &["window", "display", "window_mode"], Value::String(mode.clone()));
        set_segments(config, &["window", "display", "fullscreen"], Value::Bool(fullscreen));
        set_segments(config, &["window", "display", "borderless_fullscreen"], Value::Bool(borderless));
        set_segments(config, &["plugins", "newengine", "platform.winit", "title"], Value::String(title));
        set_segments(config, &["plugins", "newengine", "platform.winit", "width"], Value::Number(width.into()));
        set_segments(config, &["plugins", "newengine", "platform.winit", "height"], Value::Number(height.into()));

        set_segments(config, &["plugins", "newengine", "startup_window", "display", "monitor"], Value::String(monitor.clone()));
        set_segments(config, &["plugins", "newengine", "startup_window", "display", "resolution", "width"], Value::Number(width.into()));
        set_segments(config, &["plugins", "newengine", "startup_window", "display", "resolution", "height"], Value::Number(height.into()));
        set_segments(config, &["plugins", "newengine", "startup_window", "display", "window_mode"], Value::String(mode.clone()));
        set_segments(config, &["plugins", "newengine", "startup_window", "display", "fullscreen"], Value::Bool(fullscreen));
        set_segments(config, &["plugins", "newengine", "startup_window", "display", "borderless_fullscreen"], Value::Bool(borderless));
        set_segments(config, &["plugins", "newengine", "startup_window", "display", "vsync"], Value::Bool(vsync));
        set_segments(config, &["plugins", "newengine", "startup_window", "display", "refresh_rate"], Value::String(refresh_rate.clone()));
        set_segments(config, &["plugins", "newengine", "startup_window", "display", "render_scale"], number_value(render_scale));
        set_segments(config, &["plugins", "newengine", "startup_window", "display", "hdr"], Value::String(hdr.clone()));

        set_segments(config, &["plugins", "newengine", "platform.winit", "display", "monitor"], Value::String(monitor));
        set_segments(config, &["plugins", "newengine", "platform.winit", "display", "resolution", "width"], Value::Number(width.into()));
        set_segments(config, &["plugins", "newengine", "platform.winit", "display", "resolution", "height"], Value::Number(height.into()));
        set_segments(config, &["plugins", "newengine", "platform.winit", "display", "window_mode"], Value::String(mode));
        set_segments(config, &["plugins", "newengine", "platform.winit", "display", "fullscreen"], Value::Bool(fullscreen));
        set_segments(config, &["plugins", "newengine", "platform.winit", "display", "borderless_fullscreen"], Value::Bool(borderless));
        set_segments(config, &["plugins", "newengine", "platform.winit", "display", "vsync"], Value::Bool(vsync));
        set_segments(config, &["plugins", "newengine", "platform.winit", "display", "refresh_rate"], Value::String(refresh_rate));
        set_segments(config, &["plugins", "newengine", "platform.winit", "display", "render_scale"], number_value(render_scale));
        set_segments(config, &["plugins", "newengine", "platform.winit", "display", "hdr"], Value::String(hdr));
    }

    pub(super) fn apply_graphics(&self, config: &mut Value) {
        let backend = self.fields.select_value("graphics.renderer_backend", "auto");
        let gpu_device = self.fields.string_value("graphics.gpu_device", "auto");
        let profile = self.fields.select_value("graphics.graphics_profile", "auto");
        let shader_cache = self.fields.select_value("graphics.shader_cache_mode", "auto");
        let profile_id = graphics_profile_id(&profile);
        let gpu_safe = profile == "safe_mode";

        set_segments(config, &["plugins", "newengine", "startup_window", "graphics_backend", "renderer_backend"], Value::String(backend.clone()));
        set_segments(config, &["plugins", "newengine", "startup_window", "graphics_backend", "gpu_device"], Value::String(gpu_device.clone()));
        set_segments(config, &["plugins", "newengine", "startup_window", "graphics_backend", "graphics_profile"], Value::String(profile.clone()));
        set_segments(config, &["plugins", "newengine", "startup_window", "graphics_backend", "shader_cache_mode"], Value::String(shader_cache.clone()));

        set_segments(config, &["plugins", "newengine", "renderer", "selection", "backend"], Value::String(backend));
        set_segments(config, &["plugins", "newengine", "renderer", "selection", "gpu_device"], Value::String(gpu_device));
        set_segments(config, &["plugins", "newengine", "renderer", "selection", "graphics_profile"], Value::String(profile.clone()));
        set_segments(config, &["plugins", "newengine", "renderer", "selection", "graphics_profile_id"], Value::String(profile_id.to_owned()));
        set_segments(config, &["plugins", "newengine", "renderer", "selection", "shader_cache_mode"], Value::String(shader_cache.clone()));
        set_segments(config, &["plugins", "newengine", "renderer", "vulkan", "shader_cache", "mode"], Value::String(shader_cache));
        set_segments(config, &["plugins", "newengine", "engine_runtime", "render", "runtime_profile", "id"], Value::String(profile_id.to_owned()));
        set_segments(config, &["plugins", "newengine", "engine_runtime", "render", "runtime_profile", "gpu_safe"], Value::Bool(gpu_safe));

        for (key, path) in [
            ("graphics.debug.enabled", "enabled"),
            ("graphics.debug.renderdoc_capture", "renderdoc_capture"),
            ("graphics.debug.phase_viewer", "phase_viewer"),
            ("graphics.debug.target_viewer", "target_viewer"),
            ("graphics.debug.shadow_cascade_viewer", "shadow_cascade_viewer"),
            ("graphics.debug.gbuffer_viewer", "gbuffer_viewer"),
            ("graphics.debug.gpu_timing", "gpu_timing"),
        ] {
            let value = self.fields.bool_value(key, false);
            set_segments(config, &["plugins", "newengine", "startup_window", "graphics_backend", "debug_renderer_tools", path], Value::Bool(value));
            set_segments(config, &["plugins", "newengine", "renderer", "selection", "debug_renderer_tools", path], Value::Bool(value));
            set_segments(config, &["plugins", "newengine", "renderer", "vulkan", "debug_tools", path], Value::Bool(value));
        }
    }

    pub(super) fn apply_plugin_tabs(&self, config: &mut Value) {
        for tab in &self.plugin_tabs {
            if tab.plugin_id == "newengine.engine" {
                continue;
            }
            for field in &tab.fields {
                let key = plugin_field_key(&tab.plugin_id, &field.path);
                let value = match field.kind.as_str() {
                    "bool" => Value::Bool(self.fields.bool_value(&key, false)),
                    "integer" => Value::Number(parse_i64(&self.fields.string_value(&key, "0"), 0).into()),
                    "number" => number_value(parse_f64(&self.fields.string_value(&key, "0"), 0.0)),
                    "select" => Value::String(self.fields.select_value(&key, "")),
                    "array" | "object" => parse_json_or_string(&self.fields.string_value(&key, "")),
                    _ => Value::String(self.fields.string_value(&key, "")),
                };
                set_plugin_field(config, &tab.plugin_id, &field.path, value);
            }
        }
    }

    pub(super) fn apply_plugin_enabled_states(&self, config: &mut Value) {
        let mut raw_tabs = get_segments(config, &["plugins", "newengine", "startup_window", "plugin_tabs"])
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        for item in &mut raw_tabs {
            let Some(plugin_id) = item.get("plugin_id").and_then(Value::as_str).map(ToOwned::to_owned) else {
                continue;
            };
            let Some(tab) = self.plugin_tabs.iter().find(|tab| tab.plugin_id == plugin_id) else {
                continue;
            };
            if let Some(object) = item.as_object_mut() {
                object.insert("enabled".to_owned(), Value::Bool(tab.enabled));
            }
            set_plugin_field(config, &tab.plugin_id, "enabled", Value::Bool(tab.enabled));
            set_plugin_field(config, &tab.plugin_id, "host.enabled", Value::Bool(tab.enabled));
        }

        if !raw_tabs.is_empty() {
            set_segments(config, &["plugins", "newengine", "startup_window", "plugin_tabs"], Value::Array(raw_tabs));
        }
    }
}
