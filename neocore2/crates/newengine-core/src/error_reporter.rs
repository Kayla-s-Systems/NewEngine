#![forbid(unsafe_op_in_unsafe_fn)]

use crate::crash;
use crate::error::EngineError;
use crate::startup;

use std::fmt::Write as _;
use std::path::PathBuf;

use crate::path_fmt::display_clean;
use std::sync::OnceLock;

/// High-level engine error reporting facade.
///
/// Responsibilities:
/// - consistent formatting of `EngineError` chains
/// - optional inclusion of startup diagnostics
/// - delegating persistence + crash UI spawning to `crate::crash`
#[derive(Debug, Clone)]
pub struct EngineErrorReporterConfig {
    pub crash: crash::CrashReporterConfig,

    /// When enabled, the reporter appends a snapshot of the last startup load report and config.
    pub include_startup_snapshot: bool,
}

impl Default for EngineErrorReporterConfig {
    #[inline]
    fn default() -> Self {
        Self {
            crash: crash::CrashReporterConfig::default(),
            include_startup_snapshot: true,
        }
    }
}

static CFG: OnceLock<EngineErrorReporterConfig> = OnceLock::new();

pub struct EngineErrorReporter;

impl EngineErrorReporter {
    /// Installs the process-wide panic hook and configures crash reporting.
    ///
    /// Idempotent: first call wins.
    #[inline]
    pub fn install(cfg: EngineErrorReporterConfig) {
        let _ = CFG.set(cfg.clone());
        crash::install_panic_hook(cfg.crash);
    }

    /// Writes a crash report for a fatal `EngineError`.
    ///
    /// Returns the report file path when a report was successfully written.
    #[inline]
    pub fn report_fatal_engine_error(err: &EngineError) -> Option<PathBuf> {
        if matches!(err, EngineError::ExitRequested) {
            return None;
        }

        let cfg = CFG.get().cloned().unwrap_or_default();

        let mut details = String::new();
        let _ = writeln!(details, "EngineError:");
        write_engine_error_tree(&mut details, err);

        if cfg.include_startup_snapshot {
            write_startup_snapshot(&mut details);
        }

        crash::report_fatal("Fatal Engine Error", &details)
    }

    /// Writes a crash report for a fatal message.
    #[inline]
    pub fn report_fatal_message(title: &str, details: &str) -> Option<PathBuf> {
        crash::report_fatal(title, details)
    }
}

fn write_engine_error_tree(out: &mut String, err: &EngineError) {
    fn rec(out: &mut String, err: &EngineError, depth: usize) {
        let indent = "  ".repeat(depth);

        match err {
            EngineError::ExitRequested => {
                let _ = writeln!(out, "{indent}- ExitRequested");
            }
            EngineError::Other(s) => {
                let _ = writeln!(out, "{indent}- {s}");
            }
            EngineError::Module {
                module_id,
                stage,
                cause,
            } => {
                let _ = writeln!(
                    out,
                    "{indent}- ModuleError module='{module_id}' stage={stage:?}"
                );
                rec(out, cause.as_ref(), depth + 1);
            }
        }
    }

    rec(out, err, 0);
}

fn write_startup_snapshot(out: &mut String) {
    let report = startup::last_load_report();
    let cfg = startup::last_startup_config();

    if report.is_none() && cfg.is_none() {
        return;
    }

    let _ = writeln!(out);
    let _ = writeln!(out, "Startup:");

    if let Some(r) = report {
        write_startup_load_report(out, r);
    }

    if let Some(c) = cfg {
        write_startup_config(out, c);
    }
}

fn write_startup_load_report(out: &mut String, report: &startup::StartupLoadReport) {
    let src = match &report.source {
        startup::StartupConfigSource::Defaults => "Defaults".to_owned(),
        startup::StartupConfigSource::File { path } => format!("File({})", path.display()),
    };

    let file = report
        .file
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "<none>".to_owned());

    let _ = writeln!(out, "- LoadReport:");
    let _ = writeln!(out, "  - source: {src}");
    let _ = writeln!(out, "  - file: {file}");
    let _ = writeln!(out, "  - resolved_from: {:?}", report.resolved_from);
    let _ = writeln!(out, "  - file_bytes: {:?}", report.file_bytes);
    let _ = writeln!(out, "  - total_ms: {:?}", report.total_ms);
    let _ = writeln!(out, "  - overrides: {}", report.overrides.len());
    let _ = writeln!(out, "  - plugin_overrides: {}", report.plugin_overrides.len());

    if !report.overrides.is_empty() {
        let _ = writeln!(out, "  - override_list:");
        for o in &report.overrides {
            let _ = writeln!(out, "    - {}: '{}' -> '{}'", o.key, o.from, o.to);
        }
    }

    if !report.plugin_overrides.is_empty() {
        let _ = writeln!(out, "  - plugin_override_list:");
        for o in &report.plugin_overrides {
            let _ = writeln!(
                out,
                "    - {} {}: '{}' -> '{}'",
                o.plugin_id,
                o.key,
                o.from,
                o.to
            );
        }
    }
}

fn write_startup_config(out: &mut String, cfg: &startup::StartupConfig) {
    let _ = writeln!(out, "- ConfigSnapshot:");
    let _ = writeln!(out, "  - window_title: {}", cfg.window_title);
    let _ = writeln!(out, "  - window_size: {}x{}", cfg.window_size.0, cfg.window_size.1);
    let _ = writeln!(out, "  - window_placement: {:?}", cfg.window_placement);
    let _ = writeln!(out, "  - window_icon_path: {:?}", cfg.window_icon_path);
    let _ = writeln!(out, "  - modules_dir: {}", display_clean(&cfg.modules_dir));
    let _ = writeln!(out, "  - cache_files: {}", display_clean(&cfg.resolved_cache_files_dir()));
    let _ = writeln!(out, "  - config: {}", display_clean(&cfg.resolved_config_dir()));
    let _ = writeln!(out, "  - plugins: {}", cfg.plugins.len());
    let _ = writeln!(out, "  - extra: {}", cfg.extra.len());

    if !cfg.extra.is_empty() {
        let _ = writeln!(out, "  - extra_list:");
        let mut keys: Vec<&String> = cfg.extra.keys().collect();
        keys.sort();
        for k in keys {
            let v = &cfg.extra[k];
            let _ = writeln!(out, "    - {k}: {v}");
        }
    }
}
