#![forbid(unsafe_op_in_unsafe_fn)]

use eframe::egui;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone)]
struct Args {
    report: Option<PathBuf>,
    product: Option<String>,
    app: Option<String>,
    version: Option<String>,
}

impl Args {
    fn parse() -> Self {
        let mut out = Args::default();
        let mut it = std::env::args().skip(1);
        while let Some(a) = it.next() {
            match a.as_str() {
                "--report" => out.report = it.next().map(PathBuf::from),
                "--product" => out.product = it.next(),
                "--app" => out.app = it.next(),
                "--version" => out.version = it.next(),
                _ => {}
            }
        }
        out
    }
}

struct CrashReporterApp {
    title: String,
    subtitle: String,
    report_path: Option<PathBuf>,
    report_text: String,
    visuals_set: bool,
}

impl CrashReporterApp {
    fn new(args: Args) -> Self {
        let product = args.product.unwrap_or_else(|| "NewEngine".to_owned());
        let app = args.app.unwrap_or_else(|| "app".to_owned());
        let version = args.version.unwrap_or_else(|| "0.0.0".to_owned());

        let title = format!("{product} Crash Reporter");
        let subtitle = format!("{app} ({version})");

        let (report_path, report_text) = match args.report {
            Some(p) => match std::fs::read_to_string(&p) {
                Ok(t) => (Some(p), t),
                Err(e) => (
                    Some(p),
                    format!(
                        "Failed to read crash report: {e}\n\nTip: ensure the report file exists and is readable."
                    ),
                ),
            },
            None => (
                None,
                "No crash report path was provided.\n\nUsage:\n  newengine-crash-reporter --report <path>"
                    .to_owned(),
            ),
        };

        Self {
            title,
            subtitle,
            report_path,
            report_text,
            visuals_set: false,
        }
    }

    fn open_in_file_manager(path: &Path) {
        #[cfg(windows)]
        {
            let _ = std::process::Command::new("explorer")
                .arg("/select,")
                .arg(path)
                .spawn();
        }

        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("open").arg("-R").arg(path).spawn();
        }

        #[cfg(all(unix, not(target_os = "macos")))]
        {
            let dir = path.parent().unwrap_or_else(|| Path::new("."));
            let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
        }
    }
}

impl eframe::App for CrashReporterApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.visuals_set {
            ctx.set_visuals(egui::Visuals::dark());
            self.visuals_set = true;
        }

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.heading(&self.title);
                ui.add_space(12.0);
                ui.label(egui::RichText::new(&self.subtitle).strong());
            });

            if let Some(p) = self.report_path.as_ref() {
                ui.label(format!("Report: {}", p.display()));
            }

            ui.add_space(4.0);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut self.report_text)
                            .code_editor()
                            .desired_width(f32::INFINITY)
                            .interactive(false),
                    );
                });
        });

        egui::TopBottomPanel::bottom("bottom").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button("Copy report").clicked() {
                    ctx.copy_text(self.report_text.clone());
                }

                if let Some(p) = self.report_path.as_ref() {
                    if ui.button("Open folder").clicked() {
                        Self::open_in_file_manager(p);
                    }
                }

                if ui.button("Close").clicked() {
                    std::process::exit(0);
                }
            });
            ui.add_space(6.0);
        });
    }
}

fn main() -> eframe::Result<()> {
    std::env::set_var("NEWENGINE_CRASH_REPORTER_CHILD", "1");

    let args = Args::parse();
    let app = CrashReporterApp::new(args);

    let mut opts = eframe::NativeOptions::default();
    opts.viewport = opts
        .viewport
        .with_inner_size(egui::vec2(980.0, 720.0))
        .with_resizable(true);

    eframe::run_native(
        "NewEngine Crash Reporter",
        opts,
        Box::new(|_cc| Ok(Box::new(app))),
    )
}
