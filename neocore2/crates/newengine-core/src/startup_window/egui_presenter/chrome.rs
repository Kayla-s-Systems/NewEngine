use super::*;

impl eframe::App for PreStartApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.apply_style(ctx);
        self.center_window_on_startup(ctx);

        let viewport_width = ctx.input(|input| {
            let viewport = input.viewport();
            viewport
                .inner_rect
                .or(viewport.outer_rect)
                .map(|rect| rect.width())
                .unwrap_or(WINDOW_WIDTH)
        });
        let compact = viewport_width < 1180.0;
        let header_height = if compact { 156.0 } else { 132.0 };
        let footer_height = if compact { 124.0 } else { 82.0 };

        egui::TopBottomPanel::top("prestart_header")
            .exact_height(header_height)
            .frame(egui::Frame::new().fill(egui::Color32::from_rgb(3, 5, 9)).inner_margin(egui::Margin::symmetric(12, 8)))
            .show(ctx, |ui| self.render_header(ui));

        egui::TopBottomPanel::bottom("prestart_footer")
            .exact_height(footer_height)
            .frame(egui::Frame::new().fill(egui::Color32::from_rgb(6, 8, 13)).inner_margin(egui::Margin::symmetric(18, 10)))
            .show(ctx, |ui| {
                let compact_footer = ui.available_width() < 1180.0;
                if compact_footer {
                    ui.vertical(|ui| {
                        ui.horizontal_wrapped(|ui| {
                            icon(ui, IconKind::Settings, 22.0, egui::Color32::from_rgb(144, 160, 186));
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("CONFIG PATH").size(10.5).strong().color(egui::Color32::from_rgb(147, 158, 181)));
                                ui.label(egui::RichText::new(self.config_path.display().to_string()).size(12.0).color(egui::Color32::from_rgb(85, 161, 255)));
                            });
                            ui.separator();
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("ACTIVE PROFILE").size(10.5).strong().color(egui::Color32::from_rgb(147, 158, 181)));
                                ui.label(egui::RichText::new(self.fields.select_value("graphics.graphics_profile", "auto")).size(12.0).color(egui::Color32::from_rgb(210, 219, 238)));
                            });
                        });
                        ui.add_space(8.0);
                        ui.horizontal_wrapped(|ui| {
                            if action_button(ui, IconKind::Cancel, "CANCEL", 126.0, false).clicked() {
                                self.set_outcome(WindowOutcome::Cancelled);
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                            if action_button(ui, IconKind::Save, "SAVE", 126.0, false).clicked() {
                                if let Err(err) = self.save() {
                                    self.status = err;
                                }
                            }
                            let launch = action_button(ui, IconKind::Launch, "LAUNCH ENGINE", 214.0, true);
                            if launch.clicked() {
                                match self.save() {
                                    Ok(()) => {
                                        self.set_outcome(WindowOutcome::LaunchRequested);
                                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                    }
                                    Err(err) => self.status = err,
                                }
                            }
                        });
                    });
                } else {
                    ui.horizontal(|ui| {
                        icon(ui, IconKind::Settings, 22.0, egui::Color32::from_rgb(144, 160, 186));
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new("CONFIG PATH").size(10.5).strong().color(egui::Color32::from_rgb(147, 158, 181)));
                            ui.label(egui::RichText::new(self.config_path.display().to_string()).size(12.0).color(egui::Color32::from_rgb(85, 161, 255)));
                        });
                        ui.separator();
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new("ACTIVE PROFILE").size(10.5).strong().color(egui::Color32::from_rgb(147, 158, 181)));
                            ui.label(egui::RichText::new(self.fields.select_value("graphics.graphics_profile", "auto")).size(12.0).color(egui::Color32::from_rgb(210, 219, 238)));
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let launch = action_button(ui, IconKind::Launch, "LAUNCH ENGINE", 220.0, true);
                            if launch.clicked() {
                                match self.save() {
                                    Ok(()) => {
                                        self.set_outcome(WindowOutcome::LaunchRequested);
                                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                    }
                                    Err(err) => self.status = err,
                                }
                            }
                            if action_button(ui, IconKind::Save, "SAVE", 136.0, false).clicked() {
                                if let Err(err) = self.save() {
                                    self.status = err;
                                }
                            }
                            if action_button(ui, IconKind::Cancel, "CANCEL", 136.0, false).clicked() {
                                self.set_outcome(WindowOutcome::Cancelled);
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        });
                    });
                }
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(egui::Color32::from_rgb(8, 11, 17)).inner_margin(egui::Margin::symmetric(14, 12)))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("prestart-dashboard-scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.render_dashboard(ui));
            });
    }
}
