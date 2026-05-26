use super::*;

impl PreStartApp {
    pub(super) fn render_header(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        egui::Frame::new()
            .fill(egui::Color32::from_rgb(13, 17, 25))
            .corner_radius(egui::CornerRadius::same(22))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(42, 52, 70)))
            .inner_margin(egui::Margin::symmetric(22, 16))
            .show(ui, |ui| {
                let compact = ui.available_width() < 1040.0;
                if compact {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            icon(ui, IconKind::Logo, 56.0, egui::Color32::from_rgb(108, 181, 255));
                            ui.add_space(12.0);
                            ui.vertical(|ui| {
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(egui::RichText::new(APP_TITLE).size(28.0).strong().color(egui::Color32::from_rgb(241, 246, 255)));
                                    ui.label(egui::RichText::new(env!("CARGO_PKG_VERSION")).size(15.0).strong().color(egui::Color32::from_rgb(82, 169, 255)));
                                });
                                ui.label(egui::RichText::new(APP_SUBTITLE).size(13.0).color(egui::Color32::from_rgb(137, 148, 171)));
                            });
                        });
                        ui.add_space(10.0);
                        ui.horizontal_wrapped(|ui| {
                            ready_status_pill(ui);
                            if beveled_button(ui, IconKind::Folder, "Open Project Folder").clicked() {
                                self.open_project_folder();
                            }
                        });
                    });
                    return;
                }

                ui.horizontal(|ui| {
                    icon(ui, IconKind::Logo, 70.0, egui::Color32::from_rgb(108, 181, 255));
                    ui.add_space(14.0);
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(APP_TITLE).size(34.0).strong().color(egui::Color32::from_rgb(241, 246, 255)));
                        ui.label(egui::RichText::new("PreStart").size(19.0).color(egui::Color32::from_rgb(83, 159, 255)));
                    });
                    ui.add_space(30.0);
                    ui.separator();
                    ui.add_space(24.0);
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Version").size(16.0).color(egui::Color32::from_rgb(229, 235, 248)));
                            ui.label(egui::RichText::new(env!("CARGO_PKG_VERSION")).size(17.0).strong().color(egui::Color32::from_rgb(82, 169, 255)));
                            ui.label(egui::RichText::new("Alpha").size(16.0).color(egui::Color32::from_rgb(229, 235, 248)));
                        });
                        ui.label(egui::RichText::new(APP_SUBTITLE).size(14.0).color(egui::Color32::from_rgb(137, 148, 171)));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        egui::Frame::new()
                            .fill(egui::Color32::from_rgb(16, 21, 31))
                            .corner_radius(egui::CornerRadius::same(14))
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(39, 49, 66)))
                            .inner_margin(egui::Margin::symmetric(16, 12))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ready_status_pill(ui);
                                    ui.add_space(8.0);
                                    if beveled_button(ui, IconKind::Folder, "Open Project Folder").clicked() {
                                        self.open_project_folder();
                                    }
                                });
                            });
                    });
                });
            });
    }

    pub(super) fn render_sidebar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        nav_button(ui, &mut self.selected_tab, 0, "Engine", "Core settings and startup identity");
        nav_button(ui, &mut self.selected_tab, 1, "Display", "Monitor, resolution, fullscreen");
        nav_button(ui, &mut self.selected_tab, 2, "Graphics", "Renderer, GPU, profile, debug");
        ui.add_space(14.0);
        ui.label(egui::RichText::new("Plugin configuration").size(12.0).color(egui::Color32::from_rgb(126, 137, 158)));
        ui.separator();
        for (index, tab) in self.plugin_tabs.iter().enumerate() {
            if tab.plugin_id == "newengine.engine" {
                continue;
            }
            nav_button(ui, &mut self.selected_tab, 3 + index, &tab.title, &tab.category);
        }
    }

    pub(super) fn render_engine_tab(&mut self, ui: &mut egui::Ui) {
        section_header(ui, "Engine", "Host-owned launch roots. This tab writes to the root `engine` block, not `plugins.*`.");
        card(ui, |ui| {
            text_row(ui, "Modules directory", self.fields.string("engine.modules_dir", "plugins"));
            text_row(ui, "Cache files", self.fields.string("engine.cache_files", "cache"));
            text_row(ui, "Config directory", self.fields.string("engine.config", "config"));
        });
    }

    pub(super) fn render_display_tab(&mut self, ui: &mut egui::Ui) {
        section_header(ui, "Display", "Monitor, resolution and screen mode. Fullscreen is a quick control that maps to the window-mode enum.");
        card(ui, |ui| {
            text_row(ui, "Title", self.fields.string("display.title", "North Star Engine"));
            ui.horizontal(|ui| {
                ui.label(label("Resolution"));
                styled_text_edit(ui, self.fields.string("display.width", "1600"), 110.0, 28.0);
                ui.label(egui::RichText::new("×").color(TEXT_MUTED));
                styled_text_edit(ui, self.fields.string("display.height", "900"), 110.0, 28.0);
            });
            text_row(ui, "Monitor", self.fields.string("display.monitor", "primary"));
            ui.horizontal_wrapped(|ui| {
                ui.label(label("Window mode"));
                let mut display_mode = self.fields.select_value("display.window_mode", "windowed");
                if segmented_screen_mode(ui, &mut display_mode) {
                    self.set_display_window_mode(display_mode);
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.label(label("Quick controls"));
                let mut fullscreen = self.fields.bool_value("display.fullscreen", false);
                let mut borderless = self.fields.bool_value("display.borderless_fullscreen", false);
                if switch_chip_value(ui, "Fullscreen", &mut fullscreen) {
                    if fullscreen {
                        self.set_display_window_mode("exclusive_fullscreen".to_owned());
                    } else {
                        self.set_display_window_mode("windowed".to_owned());
                    }
                }
                if switch_chip_value(ui, "Borderless", &mut borderless) {
                    if borderless {
                        self.set_display_window_mode("borderless".to_owned());
                    } else if fullscreen {
                        self.set_display_window_mode("exclusive_fullscreen".to_owned());
                    } else {
                        self.set_display_window_mode("windowed".to_owned());
                    }
                }
                switch_chip(ui, "VSync", self.fields.bool("display.vsync", false));
            });
            text_row(ui, "Refresh rate", self.fields.string("display.refresh_rate", "auto"));
            text_row(ui, "Render scale", self.fields.string("display.render_scale", "1.0"));
            select_row(ui, "HDR", self.fields.select("display.hdr", "auto"), &[
                ("auto", "Auto"),
                ("enabled", "Enabled"),
                ("disabled", "Disabled"),
            ]);
        });
    }

    pub(super) fn render_graphics_tab(&mut self, ui: &mut egui::Ui) {
        section_header(ui, "Graphics Backend", "Human-facing options map to internal capability and profile IDs after save.");
        card(ui, |ui| {
            select_row(ui, "Renderer", self.fields.select("graphics.renderer_backend", "auto"), &[
                ("auto", "Auto"),
                ("vulkan", "Vulkan"),
                ("null", "NullRenderer"),
                ("dx12", "Future DX12"),
            ]);
            text_row(ui, "GPU / device", self.fields.string("graphics.gpu_device", "auto"));
            select_row(ui, "GPU profile", self.fields.select("graphics.graphics_profile", "auto"), &[
                ("auto", "Auto"),
                ("safe_mode", "Safe Mode"),
                ("legacy_gpu", "Legacy GPU"),
                ("modern_gpu", "Modern GPU"),
                ("rtx", "RTX / Raytracing-capable"),
                ("developer_diagnostics", "Developer Diagnostics"),
            ]);
            select_row(ui, "Shader cache", self.fields.select("graphics.shader_cache_mode", "auto"), &[
                ("auto", "Auto"),
                ("enabled", "Enabled"),
                ("disabled", "Disabled"),
                ("rebuild", "Rebuild now"),
            ]);
        });
        ui.add_space(10.0);
        section_header(ui, "Debug renderer tools", "Optional diagnostics exposed as explicit startup configuration.");
        card(ui, |ui| {
            switch_row(ui, "Enable debug renderer tools", "Expose renderer diagnostics in the launch profile.", self.fields.bool("graphics.debug.enabled", false));
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                switch_chip(ui, "RenderDoc capture", self.fields.bool("graphics.debug.renderdoc_capture", false));
                switch_chip(ui, "Phase viewer", self.fields.bool("graphics.debug.phase_viewer", false));
                switch_chip(ui, "Target viewer", self.fields.bool("graphics.debug.target_viewer", false));
                switch_chip(ui, "Shadow cascade viewer", self.fields.bool("graphics.debug.shadow_cascade_viewer", false));
                switch_chip(ui, "GBuffer viewer", self.fields.bool("graphics.debug.gbuffer_viewer", false));
                switch_chip(ui, "GPU timing", self.fields.bool("graphics.debug.gpu_timing", false));
            });
        });
    }

    pub(super) fn render_plugin_tab(&mut self, ui: &mut egui::Ui, index: usize) {
        let Some(tab) = self.plugin_tabs.get(index).cloned() else {
            section_header(ui, "Plugins", "No plugin configuration tab selected.");
            return;
        };
        if tab.plugin_id == "newengine.engine" {
            self.render_engine_tab(ui);
            return;
        }
        section_header(ui, &tab.title, &format!("{} · {}", tab.plugin_id, tab.source));
        card(ui, |ui| {
            if tab.fields.is_empty() {
                ui.label(egui::RichText::new("This plugin published a tab but no editable startup fields.").color(egui::Color32::from_rgb(164, 174, 195)));
                return;
            }
            for field in &tab.fields {
                self.render_schema_field(ui, &tab.plugin_id, field);
            }
        });
    }

    pub(super) fn render_schema_field(&mut self, ui: &mut egui::Ui, plugin_id: &str, field: &SchemaField) {
        let key = plugin_field_key(plugin_id, &field.path);
        let narrow = ui.available_width() < 390.0;
        match field.kind.as_str() {
            "bool" => {
                if narrow {
                    ui.vertical(|ui| {
                        ui.label(label(&field.label));
                        ui.horizontal(|ui| {
                            switch_only(ui, self.fields.bool(&key, false));
                            if let Some(default_label) = &field.default_label {
                                ui.label(egui::RichText::new(format!("default: {default_label}")).size(10.5).color(TEXT_MUTED));
                            }
                        });
                    });
                } else {
                    ui.horizontal(|ui| {
                        let label_width = if ui.available_width() < 430.0 { 112.0 } else { 138.0 };
                        ui.add_sized([label_width, 22.0], egui::Label::new(label(&field.label)));
                        switch_only(ui, self.fields.bool(&key, false));
                        if ui.available_width() > 130.0 {
                            if let Some(default_label) = &field.default_label {
                                ui.label(egui::RichText::new(format!("default: {default_label}")).size(10.5).color(TEXT_MUTED));
                            }
                        }
                    });
                }
            }
            "select" => {
                if narrow {
                    ui.vertical(|ui| {
                        ui.label(label(&field.label));
                        let width = ui.available_width().clamp(150.0, 320.0);
                        let selected = self.fields.select(&key, field.options.first().map(|o| o.value.as_str()).unwrap_or(""));
                        styled_select_dynamic(ui, key.clone(), selected, &field.options, width);
                    });
                } else {
                    ui.horizontal(|ui| {
                        let label_width = if ui.available_width() < 430.0 { 112.0 } else { 138.0 };
                        ui.add_sized([label_width, 22.0], egui::Label::new(label(&field.label)));
                        let selected = self.fields.select(&key, field.options.first().map(|o| o.value.as_str()).unwrap_or(""));
                        let width = (ui.available_width() - 4.0).clamp(140.0, 280.0);
                        styled_select_dynamic(ui, key.clone(), selected, &field.options, width);
                    });
                }
            }
            _ => {
                let editor = self.fields.string(&key, "");
                if narrow {
                    ui.vertical(|ui| {
                        ui.label(label(&field.label));
                        styled_text_edit(ui, editor, ui.available_width().clamp(150.0, 320.0), 28.0);
                    });
                } else {
                    text_row(ui, &field.label, editor);
                }
            }
        }
    }


    pub(super) fn render_dashboard(&mut self, ui: &mut egui::Ui) {
        if let Some(warning) = &self.parse_warning {
            ui.colored_label(egui::Color32::from_rgb(255, 184, 112), warning);
            ui.add_space(8.0);
        }

        let available_width = ui.available_width();
        let gap = 14.0;

        if available_width < 1220.0 {
            ui.vertical(|ui| {
                ui.set_width(available_width);
                self.render_left_launch_panel(ui);
                ui.add_space(gap);
                let panel_height = ui.available_height().max(340.0);
                self.render_right_modules_panel(ui, panel_height);
            });
            return;
        }

        let right_width = (available_width * 0.31).clamp(324.0, 408.0);
        let left_width = (available_width - right_width - gap).max(0.0);
        let panel_height = ui.available_height().max(400.0);

        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(left_width);
                self.render_left_launch_panel(ui);
            });
            ui.add_space(gap);
            ui.vertical(|ui| {
                ui.set_width(right_width);
                self.render_right_modules_panel(ui, panel_height);
            });
        });
    }

    pub(super) fn render_left_launch_panel(&mut self, ui: &mut egui::Ui) {
        launcher_card(ui, |ui| {
            card_title(ui, IconKind::Project, "PROJECT / PROFILE", None);
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                icon_box(ui, IconKind::Project);
                ui.vertical(|ui| {
                    styled_text_edit(ui, self.fields.string("project.name", "MyGameProject"), (ui.available_width() - 52.0).max(160.0), 30.0);
                    styled_text_edit(ui, self.fields.string("project.path", "."), (ui.available_width() - 52.0).max(160.0), 26.0);
                });
                icon_button_box(ui, IconKind::Settings);
            });
            ui.add_space(14.0);

            card_title(ui, IconKind::Terminal, "LAUNCH PARAMETERS", None);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                styled_text_edit(ui, self.fields.string("launch.parameters", "--windowed --devmode --log --console"), (ui.available_width() - 46.0).max(220.0), 34.0);
                icon_button_box(ui, IconKind::Settings);
            });
            ui.add_space(14.0);

            let compact_controls = ui.available_width() < 690.0;
            if compact_controls {
                self.render_launch_controls_column(ui);
            } else {
                ui.columns(2, |columns| {
                    columns[0].vertical(|ui| self.render_launch_controls_left(ui));
                    columns[1].vertical(|ui| self.render_launch_controls_right(ui));
                });
            }
        });

        ui.add_space(12.0);
        launcher_card(ui, |ui| {
            ui.horizontal(|ui| {
                icon(ui, IconKind::Bookmark, 26.0, egui::Color32::from_rgb(143, 164, 196));
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("REMEMBER LAST OPTIONS").size(13.0).strong().color(egui::Color32::from_rgb(162, 178, 203)));
                    ui.label(egui::RichText::new("Load the last used configuration on startup").size(12.0).color(egui::Color32::from_rgb(142, 154, 178)));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    switch_only(ui, self.fields.bool("launch.remember_last_options", true));
                });
            });
        });
    }


    pub(super) fn render_launch_controls_column(&mut self, ui: &mut egui::Ui) {
        self.render_launch_controls_left(ui);
        ui.add_space(12.0);
        ui.separator();
        ui.add_space(12.0);
        self.render_launch_controls_right(ui);
    }

    pub(super) fn render_launch_controls_left(&mut self, ui: &mut egui::Ui) {
        card_title(ui, IconKind::Chip, "RENDERER", None);
        select_row(ui, "", self.fields.select("graphics.renderer_backend", "auto"), &[
            ("auto", "Auto"),
            ("vulkan", "Vulkan"),
            ("null", "NullRenderer"),
            ("dx12", "Future DX12"),
        ]);
        ui.add_space(10.0);
        card_title(ui, IconKind::Monitor, "RESOLUTION", None);
        ui.horizontal_wrapped(|ui| {
            styled_text_edit(ui, self.fields.string("display.width", "1600"), 90.0, 30.0);
            ui.label("×");
            styled_text_edit(ui, self.fields.string("display.height", "900"), 90.0, 30.0);
        });
        ui.add_space(10.0);
        card_title(ui, IconKind::Check, "VSYNC", None);
        switch_row(ui, "Enabled", "Synchronize present rate with the selected display refresh. Disabled by default for max-FPS development runs; enable explicitly for stable presentation.", self.fields.bool("display.vsync", false));
    }

    pub(super) fn render_launch_controls_right(&mut self, ui: &mut egui::Ui) {
        card_title(ui, IconKind::ScreenMode, "SCREEN MODE", None);
        let mut display_mode = self.fields.select_value("display.window_mode", "windowed");
        if segmented_screen_mode(ui, &mut display_mode) {
            self.set_display_window_mode(display_mode);
        }
        ui.add_space(10.0);
        card_title(ui, IconKind::Check, "FULLSCREEN", None);
        let mut fullscreen = self.fields.bool_value("display.fullscreen", false);
        let mut borderless = self.fields.bool_value("display.borderless_fullscreen", false);
        if switch_row_change(ui, "Fullscreen", "Quick toggle between windowed and fullscreen-oriented presentation.", &mut fullscreen) {
            if fullscreen {
                self.set_display_window_mode("exclusive_fullscreen".to_owned());
            } else {
                self.set_display_window_mode("windowed".to_owned());
            }
        }
        if switch_row_change(ui, "Borderless fullscreen", "Use borderless fullscreen presentation instead of a bordered window.", &mut borderless) {
            if borderless {
                self.set_display_window_mode("borderless".to_owned());
            } else if fullscreen {
                self.set_display_window_mode("exclusive_fullscreen".to_owned());
            } else {
                self.set_display_window_mode("windowed".to_owned());
            }
        }
        ui.add_space(12.0);
        card_title(ui, IconKind::ScreenMode, "STARTUP SCENE / PROFILE", None);
        ui.horizontal(|ui| {
            styled_text_edit(ui, self.fields.string("launch.startup_scene", "MainMenu"), (ui.available_width() - 44.0).max(180.0), 32.0);
            icon_button_box(ui, IconKind::Settings);
        });
    }

    pub(super) fn render_right_modules_panel(&mut self, ui: &mut egui::Ui, available_height: f32) {
        let plugin_height = (available_height * 0.46).clamp(196.0, 320.0);
        let config_height = (available_height - plugin_height - 16.0).clamp(180.0, 420.0);
        self.render_plugins_modules_card(ui, plugin_height);
        ui.add_space(12.0);
        self.render_selected_plugin_config_card(ui, config_height);
    }

    pub(super) fn render_plugins_modules_card(&mut self, ui: &mut egui::Ui, max_height: f32) {
        launcher_card(ui, |ui| {
            let enabled_count = self.plugin_tabs.iter().filter(|tab| tab.plugin_id != "newengine.engine" && tab.enabled).count();
            ui.horizontal(|ui| {
                card_title(ui, IconKind::Puzzle, "PLUGINS / MODULES", Some(&format!("{enabled_count} enabled")));
            });
            ui.add_space(8.0);
            egui::ScrollArea::vertical()
                .id_salt("prestart-plugin-modules-scroll")
                .max_height(max_height)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for index in 0..self.plugin_tabs.len() {
                        if self.plugin_tabs[index].plugin_id == "newengine.engine" {
                            continue;
                        }
                        let selected = self.selected_plugin.as_deref() == Some(self.plugin_tabs[index].plugin_id.as_str());
                        match plugin_module_entry(ui, &self.plugin_tabs[index], selected) {
                            PluginEntryAction::OpenConfig => self.select_plugin(index),
                            PluginEntryAction::ToggleEnabled => self.toggle_plugin(index),
                            PluginEntryAction::None => {}
                        }
                    }
                });
            ui.add_space(8.0);
            let _response = beveled_button(ui, IconKind::Settings, "Manage Plugins...");
        });
    }

    pub(super) fn select_plugin(&mut self, index: usize) {
        let Some(tab) = self.plugin_tabs.get(index) else { return; };
        self.selected_plugin = Some(tab.plugin_id.clone());
        self.status = if tab.enabled {
            format!("Selected {}. Configuration is open.", tab.title)
        } else {
            format!("Selected {}. Plugin is disabled; status block toggles loading.", tab.title)
        };
    }

    pub(super) fn toggle_plugin(&mut self, index: usize) {
        let Some(tab) = self.plugin_tabs.get_mut(index) else { return; };
        tab.enabled = !tab.enabled;
        if tab.enabled {
            self.selected_plugin = Some(tab.plugin_id.clone());
            self.status = format!("Enabled {}. Plugin configuration is visible.", tab.title);
        } else {
            if self.selected_plugin.as_deref() == Some(tab.plugin_id.as_str()) {
                self.selected_plugin = None;
            }
            self.status = format!("Disabled {}. Its configuration block is hidden.", tab.title);
        }
    }

    pub(super) fn render_selected_plugin_config_card(&mut self, ui: &mut egui::Ui, max_height: f32) {
        let selected = self
            .selected_plugin
            .clone()
            .and_then(|selected_id| self.plugin_tabs.iter().find(|tab| tab.plugin_id == selected_id).cloned());

        launcher_card(ui, |ui| {
            match selected {
                Some(tab) => {
                    let pill = if tab.enabled { "enabled" } else { "disabled" };
                    card_title(ui, plugin_icon(&tab), &format!("{} CONFIG", tab.title.to_uppercase()), Some(pill));
                    ui.label(egui::RichText::new(format!("{} · {}", tab.plugin_id, tab.category)).size(11.0).color(egui::Color32::from_rgb(120, 132, 154)));
                    if !tab.enabled {
                        ui.label(egui::RichText::new("Disabled plugins keep editable config, but the loader will skip their DLL until re-enabled.").size(11.0).color(egui::Color32::from_rgb(255, 184, 112)));
                    }
                    ui.add_space(8.0);
                    if tab.fields.is_empty() {
                        ui.label(egui::RichText::new("No editable startup fields were published by this plugin.").color(egui::Color32::from_rgb(150, 162, 185)));
                    } else {
                        egui::ScrollArea::vertical()
                            .id_salt(format!("plugin-config-scroll-{}", tab.plugin_id))
                            .max_height(max_height)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                for field in &tab.fields {
                                    self.render_schema_field(ui, &tab.plugin_id, field);
                                    ui.add_space(4.0);
                                }
                            });
                    }
                }
                None => {
                    card_title(ui, IconKind::Settings, "MODULE CONFIG", Some("select module"));
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("Choose a module from the PLUGINS / MODULES card to edit its startup settings.").size(12.5).color(egui::Color32::from_rgb(150, 162, 185)));
                    ui.add_space(10.0);
                    ui.label(egui::RichText::new("This keeps the right column stable, avoids empty gaps, and matches the concept layout.").size(11.0).color(egui::Color32::from_rgb(118, 130, 152)));
                }
            }
        });
    }

    pub(super) fn render_recent_configs_card(&mut self, ui: &mut egui::Ui) {
        launcher_card(ui, |ui| {
            card_title(ui, IconKind::Clock, "RECENT CONFIGURATIONS", None);
            ui.add_space(8.0);
            for (title, meta) in [
                ("Current config", "canonical config.json"),
                ("Developer diagnostics", "renderer debug preset"),
            ] {
                ui.horizontal(|ui| {
                    icon(ui, IconKind::Clock, 18.0, egui::Color32::from_rgb(132, 149, 176));
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(title).size(13.0).color(egui::Color32::from_rgb(214, 222, 238)));
                        ui.label(egui::RichText::new(meta).size(10.5).color(egui::Color32::from_rgb(116, 128, 151)));
                    });
                });
                ui.separator();
            }
        });
    }
}
