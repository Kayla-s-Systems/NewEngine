#![forbid(unsafe_op_in_unsafe_fn)]

use crate::cli::Args;
use std::path::PathBuf;

pub struct CrashReporterApp {
    pub title: String,
    pub subtitle: String,

    pub report_path: Option<PathBuf>,
    pub report_text: String,

    pub user_notes: String,
    pub include_env: bool,

    pub visuals_set: bool,
}

impl CrashReporterApp {
    pub fn new(args: Args) -> Self {
        let product = args.product.unwrap_or_else(|| "NewEngine".to_owned());
        let app = args.app.unwrap_or_else(|| "app".to_owned());
        let version = args.version.unwrap_or_else(|| "0.0.0".to_owned());

        let title = format!("{product} Crash Reporter");
        let subtitle = format!("{app} ({version})");

        let (report_path, report_text) = match args.report {
            Some(p) => match std::fs::read_to_string(&p) {
                Ok(t) => (Some(p), t),
                Err(e) => (Some(p), format!("Failed to read crash report: {e}")),
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
            user_notes: String::new(),
            include_env: true,
            visuals_set: false,
        }
    }

    pub fn build_clipboard_payload(&self) -> String {
        let mut out = String::new();

        out.push_str(&self.title);
        out.push('\n');
        out.push_str(&self.subtitle);
        out.push('\n');

        if let Some(p) = self.report_path.as_ref() {
            out.push_str("Report: ");
            out.push_str(&p.display().to_string());
            out.push('\n');
        }

        if !self.user_notes.trim().is_empty() {
            out.push_str("\n--- User Notes ---\n");
            out.push_str(self.user_notes.trim());
            out.push('\n');
        }

        if self.include_env {
            out.push_str("\n--- Environment ---\n");
            out.push_str(&format!("os: {}\n", std::env::consts::OS));
            out.push_str(&format!("arch: {}\n", std::env::consts::ARCH));
        }

        out.push_str("\n--- Report ---\n");
        out.push_str(&self.report_text);

        out
    }
}
