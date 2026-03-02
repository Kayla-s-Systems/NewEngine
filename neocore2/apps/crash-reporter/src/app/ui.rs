#![forbid(unsafe_op_in_unsafe_fn)]

use crate::platform::open::reveal_in_file_manager;
use eframe::egui;

use super::model::CrashReporterApp;
use super::style::{apply_once, card_frame, Theme};

impl eframe::App for CrashReporterApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let theme = Theme::default();

        if !self.visuals_set {
            apply_once(ctx, theme);
            self.visuals_set = true;
        }

        top_bar(ctx, theme, &self.title, &self.subtitle, self.report_path.as_ref());

        egui::SidePanel::left("sidebar")
            .resizable(false)
            .default_width(360.0)
            .min_width(320.0)
            .show(ctx, |ui| {
                sidebar(ui, ctx, theme, self);
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            report_view(ui, theme, &mut self.report_text);
        });

        bottom_bar(ctx, theme, self);
    }
}

fn clamp_w(ui: &egui::Ui) -> f32 {
    let w = ui.available_width();
    if w.is_finite() && w > 1.0 { w } else { 1.0 }
}

fn top_bar(
    ctx: &egui::Context,
    theme: Theme,
    title: &str,
    subtitle: &str,
    report_path: Option<&std::path::PathBuf>,
) {
    egui::TopBottomPanel::top("top_bar")
        .resizable(false)
        .show(ctx, |ui| {
            ui.add_space(6.0);
            card_frame(theme).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.heading(title);
                        ui.label(
                            egui::RichText::new(subtitle)
                                .color(theme.accent.linear_multiply(0.95))
                                .strong(),
                        );
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let t = if let Some(p) = report_path {
                            format!("Report: {}", p.display())
                        } else {
                            "Report: <none>".to_owned()
                        };
                        ui.label(
                            egui::RichText::new(t).color(egui::Color32::from_rgb(170, 170, 170)),
                        );
                    });
                });
            });
            ui.add_space(6.0);
        });
}

fn sidebar(ui: &mut egui::Ui, ctx: &egui::Context, theme: Theme, app: &mut CrashReporterApp) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            card_frame(theme).show(ui, |ui| {
                ui.label(egui::RichText::new("Summary").strong());
                ui.add_space(6.0);

                if let Some(p) = app.report_path.as_ref() {
                    ui.label(egui::RichText::new("File").strong());
                    ui.monospace(p.display().to_string());

                    if let Ok(meta) = std::fs::metadata(p) {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label("Size:");
                            ui.label(format!("{} bytes", meta.len()));
                        });
                    }
                } else {
                    ui.label("No report file path provided.");
                }

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);

                ui.label(egui::RichText::new("User Notes").strong());
                ui.add_space(6.0);

                ui.add(
                    egui::TextEdit::multiline(&mut app.user_notes)
                        .desired_width(clamp_w(ui))
                        .desired_rows(6)
                        .hint_text("What were you doing when it crashed? Steps to reproduce, etc."),
                );

                ui.add_space(8.0);
                ui.checkbox(&mut app.include_env, "Include environment info");
            });

            ui.add_space(10.0);

            card_frame(theme).show(ui, |ui| {
                ui.label(egui::RichText::new("Actions").strong());
                ui.add_space(8.0);

                let w = clamp_w(ui);

                let primary = egui::Button::new(
                    egui::RichText::new("Copy All")
                        .strong()
                        .color(egui::Color32::WHITE),
                )
                    .fill(theme.accent.linear_multiply(0.85));

                if ui.add_sized([w, 36.0], primary).clicked() {
                    ctx.copy_text(app.build_clipboard_payload());
                }

                ui.add_space(6.0);

                if let Some(p) = app.report_path.as_ref() {
                    if ui.add_sized([w, 32.0], egui::Button::new("Reveal in Folder")).clicked() {
                        reveal_in_file_manager(p);
                    }
                }

                ui.add_space(6.0);

                if ui.add_sized([w, 32.0], egui::Button::new("Close")).clicked() {
                    std::process::exit(0);
                }
            });
        });
}

fn report_view(ui: &mut egui::Ui, theme: Theme, report_text: &mut String) {
    card_frame(theme).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Report").strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new("Editable (redact if needed)")
                        .color(egui::Color32::from_rgb(150, 150, 150)),
                );
            });
        });

        ui.add_space(8.0);

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(report_text)
                        .code_editor()
                        .desired_width(clamp_w(ui)),
                );
            });
    });
}

fn bottom_bar(ctx: &egui::Context, theme: Theme, app: &mut CrashReporterApp) {
    egui::TopBottomPanel::bottom("bottom_bar")
        .resizable(false)
        .show(ctx, |ui| {
            ui.add_space(6.0);

            card_frame(theme).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Tip:")
                            .color(egui::Color32::from_rgb(170, 170, 170))
                            .strong(),
                    );
                    ui.label(
                        egui::RichText::new("Attach this report when filing a bug. Include reproduction steps.")
                            .color(egui::Color32::from_rgb(170, 170, 170)),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Copy Report Only").clicked() {
                            ctx.copy_text(app.report_text.clone());
                        }
                    });
                });
            });

            ui.add_space(6.0);
        });
}
