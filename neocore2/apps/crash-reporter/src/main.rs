#![forbid(unsafe_op_in_unsafe_fn)]

mod app;
mod cli;
mod platform;

use app::CrashReporterApp;
use cli::Args;

fn main() -> eframe::Result<()> {
    std::env::set_var("NEWENGINE_CRASH_REPORTER_CHILD", "1");

    let args = Args::parse_env();
    let app = CrashReporterApp::new(args);

    let mut opts = eframe::NativeOptions::default();
    opts.viewport = opts
        .viewport
        .with_inner_size(egui::vec2(1040.0, 720.0))
        .with_resizable(true);

    eframe::run_native(
        "NewEngine Crash Reporter",
        opts,
        Box::new(|_cc| Ok(Box::new(app))),
    )
}
