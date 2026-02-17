#![forbid(unsafe_op_in_unsafe_fn)]

use crate::error::{EngineError, EngineResult};
use crate::startup::config::UiBackend;
use crate::startup::{
    ConfigPaths, StartupConfig, StartupConfigSource, StartupLoadReport, StartupOverride,
    StartupResolvedFrom, WindowPlacement,
};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

pub struct StartupLoader;

impl StartupLoader {
    pub fn load_json(paths: &ConfigPaths) -> EngineResult<(StartupConfig, StartupLoadReport)> {
        let mut cfg = StartupConfig::default();
        let mut report = StartupLoadReport::new();

        let raw_path = paths.startup_path();

        match resolve_startup_file_optional(paths, raw_path) {
            Ok(Some((resolved, from))) => {
                report.file = Some(resolved.clone());
                report.resolved_from = from;

                let data = fs::read_to_string(&resolved).map_err(|e| {
                    EngineError::Other(format!(
                        "startup config read failed: path={:?} err={}",
                        resolved, e
                    ))
                })?;

                let parsed: RootJson = serde_json::from_str(&data).map_err(|e| {
                    EngineError::Other(format!(
                        "startup config parse failed (json): path={:?} err={}",
                        resolved, e
                    ))
                })?;

                apply_root(&mut cfg, &mut report, parsed);

                cfg.source = StartupConfigSource::File {
                    path: resolved.clone(),
                };
                report.source = cfg.source.clone();
            }
            Ok(None) => {
                report.source = StartupConfigSource::Defaults;
                report.file = None;
                report.resolved_from = StartupResolvedFrom::NotProvided;
            }
            Err(e) => return Err(e),
        }

        Ok((cfg, report))
    }
}

#[derive(Deserialize)]
struct RootJson {
    window: Option<WindowJson>,
    logging: Option<LoggingJson>,
    engine: Option<EngineJson>,
    render: Option<RenderJson>,
    ui: Option<UiJson>,
}

#[derive(Deserialize)]
struct LoggingJson {
    // Legacy:
    level: Option<String>,
    #[allow(dead_code)]
    colors: Option<bool>,
    #[allow(dead_code)]
    include_module: Option<bool>,

    // Extended:
    filter: Option<String>,
    style: Option<String>,
    target: Option<String>,
    file: Option<String>,
    tee: Option<bool>,

    include: Option<LoggingIncludeJson>,
    rolling: Option<LoggingRollingJson>,
    timestamp: Option<String>,
    indent: Option<usize>,
}

#[derive(Deserialize)]
struct LoggingIncludeJson {
    module_path: Option<bool>,
    target: Option<bool>,
    file: Option<bool>,
    line: Option<bool>,
}

#[derive(Deserialize)]
struct LoggingRollingJson {
    max_bytes: Option<u64>,
    max_files: Option<usize>,
    keep_days: Option<usize>,
}

#[derive(Deserialize)]
struct WindowJson {
    title: Option<String>,

    size: Option<[u32; 2]>,
    width: Option<u32>,
    height: Option<u32>,

    placement: Option<WindowPlacementJson>,

    /// Logical path inside assets, e.g. "ui/icon.png"
    icon: Option<String>,
}

#[derive(Deserialize)]
struct WindowPlacementJson {
    #[serde(rename = "type")]
    kind: Option<String>,
    offset: Option<[i32; 2]>,
}

#[derive(Deserialize)]
struct EngineJson {
    assets_root: Option<String>,
    asset_pump_steps: Option<u32>,
    asset_filesystem_source: Option<bool>,
    modules_dir: Option<String>,
}

#[derive(Deserialize)]
struct RenderJson {
    backend: Option<String>,
    clear_color: Option<[f32; 4]>,
    debug_text: Option<String>,
}

#[derive(Deserialize)]
struct UiJson {
    backend: Option<String>,
}

fn apply_root(cfg: &mut StartupConfig, report: &mut StartupLoadReport, src: RootJson) {
    if let Some(logging) = src.logging {
        // Legacy: "logging.level" mirrors old "log_level" field.
        if let Some(level) = logging.level.clone() {
            apply_string(report, "logging.level", &mut cfg.logging.level, level.clone());
            apply_string(report, "log_level", &mut cfg.log_level, level);
        }

        if let Some(filter) = logging.filter {
            apply_opt_string(report, "logging.filter", &mut cfg.logging.filter, filter);
        }

        if let Some(style) = logging.style {
            apply_opt_string(report, "logging.style", &mut cfg.logging.style, style);
        }

        // target: stdout|stderr
        if let Some(t) = logging.target {
            apply_opt_string(report, "logging.target", &mut cfg.logging.console_target, t);
        }

        // file path + tee behavior
        if let Some(fp) = logging.file {
            apply_opt_string(report, "logging.file", &mut cfg.logging.file_path, fp);
            // If tee not explicitly specified, default to true when file is set.
            if logging.tee.is_none() && !cfg.logging.tee {
                apply_bool(report, "logging.tee", &mut cfg.logging.tee, true);
            }
        }

        if let Some(tee) = logging.tee {
            apply_bool(report, "logging.tee", &mut cfg.logging.tee, tee);
        }

        // Legacy color + include_module mapping
        if let Some(colors) = logging.colors {
            apply_bool(report, "logging.colors", &mut cfg.logging.colors, colors);
        }
        if let Some(inc_mod) = logging.include_module {
            apply_bool(report, "logging.include.module_path", &mut cfg.logging.include_module_path, inc_mod);
        }

        if let Some(inc) = logging.include {
            if let Some(v) = inc.module_path {
                apply_bool(report, "logging.include.module_path", &mut cfg.logging.include_module_path, v);
            }
            if let Some(v) = inc.target {
                apply_bool(report, "logging.include.target", &mut cfg.logging.include_target, v);
            }
            if let Some(v) = inc.file {
                apply_bool(report, "logging.include.file", &mut cfg.logging.include_file, v);
            }
            if let Some(v) = inc.line {
                apply_bool(report, "logging.include.line", &mut cfg.logging.include_line_number, v);
            }
        }

        if let Some(ts) = logging.timestamp {
            // "none" means explicit disable
            if ts.trim().eq_ignore_ascii_case("none") {
                let from = cfg.logging.timestamp.clone().unwrap_or_else(|| "null".to_owned());
                let to = "null".to_owned();
                if cfg.logging.timestamp.is_some() {
                    cfg.logging.timestamp = None;
                    report.overrides.push(StartupOverride {
                        key: "logging.timestamp",
                        from,
                        to,
                    });
                }
            } else {
                apply_opt_string(report, "logging.timestamp", &mut cfg.logging.timestamp, ts);
            }
        }

        if let Some(indent) = logging.indent {
            let from = cfg.logging.indent.map(|v| v.to_string()).unwrap_or_else(|| "null".to_owned());
            let to = indent.to_string();
            if cfg.logging.indent != Some(indent) {
                cfg.logging.indent = Some(indent);
                report.overrides.push(StartupOverride {
                    key: "logging.indent",
                    from,
                    to,
                });
            }
        }

        if let Some(rolling) = logging.rolling {
            if let Some(v) = rolling.max_bytes {
                apply_opt_u64(report, "logging.rolling.max_bytes", &mut cfg.logging.roll_max_bytes, v);
            }
            if let Some(v) = rolling.max_files {
                apply_usize(report, "logging.rolling.max_files", &mut cfg.logging.roll_max_files, v);
            }
            if let Some(v) = rolling.keep_days {
                apply_opt_usize(report, "logging.rolling.keep_days", &mut cfg.logging.roll_keep_days, v);
            }
        }
    }

    if let Some(w) = src.window {
        if let Some(t) = w.title {
            apply_string(report, "window_title", &mut cfg.window_title, t);
        }

        if let Some([ww, hh]) = w.size {
            apply_size(report, "window_size", &mut cfg.window_size, (ww, hh));
        } else {
            match (w.width, w.height) {
                (Some(ww), Some(hh)) => {
                    apply_size(report, "window_size", &mut cfg.window_size, (ww, hh));
                }
                (Some(_), None) | (None, Some(_)) => report.overrides.push(StartupOverride {
                    key: "window_size",
                    from: format_size(cfg.window_size),
                    to: "ignored (width/height must both be present)".to_owned(),
                }),
                (None, None) => {}
            }
        }

        if let Some(p) = w.placement {
            if let Some(pl) = parse_placement(p) {
                apply_placement(report, "window_placement", &mut cfg.window_placement, pl);
            }
        }

        if let Some(icon) = w.icon {
            apply_opt_string(report, "window_icon", &mut cfg.window_icon_path, icon);
        }
    }

    if let Some(engine) = src.engine {
        if let Some(root) = engine.assets_root {
            apply_path(report, "assets_root", &mut cfg.assets_root, root);
        }
        if let Some(steps) = engine.asset_pump_steps {
            apply_u32(report, "asset_pump_steps", &mut cfg.asset_pump_steps, steps);
        }
        if let Some(enabled) = engine.asset_filesystem_source {
            apply_bool(
                report,
                "asset_filesystem_source",
                &mut cfg.asset_filesystem_source,
                enabled,
            );
        }
        if let Some(dir) = engine.modules_dir {
            apply_path(report, "modules_dir", &mut cfg.modules_dir, dir);
        }
    }

    if let Some(render) = src.render {
        if let Some(backend) = render.backend {
            apply_string(report, "render_backend", &mut cfg.render_backend, backend);
        }
        if let Some(color) = render.clear_color {
            apply_color(report, "render_clear_color", &mut cfg.render_clear_color, color);
        }
        if let Some(text) = render.debug_text {
            apply_string(report, "render_debug_text", &mut cfg.render_debug_text, text);
        }
    }

    if let Some(ui) = src.ui {
        if let Some(backend) = ui.backend {
            let parsed = parse_ui_backend(&backend);
            apply_ui_backend(report, "ui_backend", &mut cfg.ui_backend, parsed);
        }
    }
}

fn parse_placement(p: WindowPlacementJson) -> Option<WindowPlacement> {
    let kind = p
        .kind
        .unwrap_or_else(|| "default".to_owned())
        .to_ascii_lowercase();

    match kind.as_str() {
        "centered" => {
            let off = p.offset.unwrap_or([0, 0]);
            Some(WindowPlacement::Centered {
                offset: (off[0], off[1]),
            })
        }
        "default" => Some(WindowPlacement::Default),
        _ => None,
    }
}

fn parse_ui_backend(s: &str) -> UiBackend {
    let v = s.trim().to_ascii_lowercase();
    match v.as_str() {
        "egui" => UiBackend::Egui,
        "none" | "null" | "off" | "disabled" => UiBackend::Disabled,
        _ => UiBackend::Custom(s.trim().to_owned()),
    }
}

#[inline]
fn apply_string(report: &mut StartupLoadReport, key: &'static str, dst: &mut String, v: String) {
    let from = dst.clone();
    if from != v {
        *dst = v.clone();
        report.overrides.push(StartupOverride {
            key,
            from,
            to: v,
        });
    }
}

#[inline]
fn apply_opt_string(
    report: &mut StartupLoadReport,
    key: &'static str,
    dst: &mut Option<String>,
    v: String,
) {
    let from = dst.clone().unwrap_or_else(|| "null".to_owned());
    let to = v.clone();

    let changed = match dst {
        Some(cur) => cur != &v,
        None => true,
    };

    if changed {
        *dst = Some(v);
        report.overrides.push(StartupOverride { key, from, to });
    }
}

#[inline]
fn apply_u32(report: &mut StartupLoadReport, key: &'static str, dst: &mut u32, v: u32) {
    let from = dst.to_string();
    let to = v.to_string();
    if *dst != v {
        *dst = v;
        report.overrides.push(StartupOverride { key, from, to });
    }
}

#[inline]
fn apply_usize(report: &mut StartupLoadReport, key: &'static str, dst: &mut usize, v: usize) {
    let from = dst.to_string();
    let to = v.to_string();
    if *dst != v {
        *dst = v;
        report.overrides.push(StartupOverride { key, from, to });
    }
}

#[inline]
fn apply_opt_u64(report: &mut StartupLoadReport, key: &'static str, dst: &mut Option<u64>, v: u64) {
    let from = dst.map(|x| x.to_string()).unwrap_or_else(|| "null".to_owned());
    let to = v.to_string();
    if *dst != Some(v) {
        *dst = Some(v);
        report.overrides.push(StartupOverride { key, from, to });
    }
}

#[inline]
fn apply_opt_usize(
    report: &mut StartupLoadReport,
    key: &'static str,
    dst: &mut Option<usize>,
    v: usize,
) {
    let from = dst.map(|x| x.to_string()).unwrap_or_else(|| "null".to_owned());
    let to = v.to_string();
    if *dst != Some(v) {
        *dst = Some(v);
        report.overrides.push(StartupOverride { key, from, to });
    }
}
fn apply_bool(report: &mut StartupLoadReport, key: &'static str, dst: &mut bool, v: bool) {
    let from = dst.to_string();
    let to = v.to_string();
    if *dst != v {
        *dst = v;
        report.overrides.push(StartupOverride { key, from, to });
    }
}

#[inline]
fn apply_size(
    report: &mut StartupLoadReport,
    key: &'static str,
    dst: &mut (u32, u32),
    v: (u32, u32),
) {
    let from = format_size(*dst);
    let to = format_size(v);
    if *dst != v {
        *dst = v;
        report.overrides.push(StartupOverride { key, from, to });
    }
}

#[inline]
fn apply_placement(
    report: &mut StartupLoadReport,
    key: &'static str,
    dst: &mut WindowPlacement,
    v: WindowPlacement,
) {
    let from = format!("{:?}", dst);
    let to = format!("{:?}", v);
    if *dst != v {
        *dst = v;
        report.overrides.push(StartupOverride { key, from, to });
    }
}

#[inline]
fn apply_ui_backend(report: &mut StartupLoadReport, key: &'static str, dst: &mut UiBackend, v: UiBackend) {
    let from = format!("{:?}", dst);
    let to = format!("{:?}", v);
    if *dst != v {
        *dst = v;
        report.overrides.push(StartupOverride { key, from, to });
    }
}

#[inline]
fn apply_path(report: &mut StartupLoadReport, key: &'static str, dst: &mut PathBuf, v: String) {
    let from = dst.display().to_string();
    let pb = PathBuf::from(v);
    let to = pb.display().to_string();
    if *dst != pb {
        *dst = pb;
        report.overrides.push(StartupOverride { key, from, to });
    }
}

#[inline]
fn apply_color(
    report: &mut StartupLoadReport,
    key: &'static str,
    dst: &mut [f32; 4],
    v: [f32; 4],
) {
    let from = format!("{:.3},{:.3},{:.3},{:.3}", dst[0], dst[1], dst[2], dst[3]);
    let to = format!("{:.3},{:.3},{:.3},{:.3}", v[0], v[1], v[2], v[3]);
    if *dst != v {
        *dst = v;
        report.overrides.push(StartupOverride { key, from, to });
    }
}

#[inline]
fn format_size(s: (u32, u32)) -> String {
    format!("{}x{}", s.0, s.1)
}

fn resolve_startup_file_optional(
    _paths: &ConfigPaths,
    raw: &str,
) -> EngineResult<Option<(PathBuf, StartupResolvedFrom)>> {
    let p = Path::new(raw);

    if p.is_absolute() {
        if p.exists() {
            return Ok(Some((p.to_path_buf(), StartupResolvedFrom::Absolute)));
        }
        return Ok(None);
    }

    // CWD
    let cwd = std::env::current_dir().map_err(|e| {
        EngineError::Other(format!("startup: current_dir failed err={}", e))
    })?;
    let in_cwd = cwd.join(p);
    if in_cwd.exists() {
        return Ok(Some((in_cwd, StartupResolvedFrom::Cwd)));
    }

    Ok(None)
}