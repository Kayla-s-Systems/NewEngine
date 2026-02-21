#![forbid(unsafe_op_in_unsafe_fn)]

use crate::error::{EngineError, EngineResult};
use crate::startup::config::{StartupPluginOverride, UiBackend};
use crate::startup::{
    ConfigPaths, StartupConfig, StartupConfigSource, StartupLoadReport,
    StartupOverride, StartupResolvedFrom, WindowPlacement,
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

        crate::startup::set_last_load_report(report.clone());
        crate::startup::set_last_startup_config(cfg.clone());
        Ok((cfg, report))
    }
}

#[derive(Deserialize)]
struct RootJson {
    window: Option<WindowJson>,
    /// Legacy (pre-plugin). Mapped to plugins["newengine.logging"] for backward compatibility.
    logging: Option<LoggingJson>,
    engine: Option<EngineJson>,
    render: Option<RenderJson>,
    ui: Option<UiJson>,
    plugins: Option<std::collections::HashMap<String, serde_json::Value>>,
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
        // Backward compatibility: old top-level `logging` maps to plugins["newengine.logging"].
        let mut o = serde_json::Map::new();

        if let Some(level) = logging.level {
            o.insert("level".to_owned(), serde_json::Value::String(level));
        }
        if let Some(filter) = logging.filter {
            o.insert("filter".to_owned(), serde_json::Value::String(filter));
        }
        if let Some(style) = logging.style {
            o.insert("style".to_owned(), serde_json::Value::String(style));
        }
        if let Some(t) = logging.target {
            o.insert("console_target".to_owned(), serde_json::Value::String(t));
        }
        if let Some(fp) = logging.file {
            o.insert("file_path".to_owned(), serde_json::Value::String(fp));
        }
        if let Some(tee) = logging.tee {
            o.insert("tee".to_owned(), serde_json::Value::Bool(tee));
        }
        if let Some(colors) = logging.colors {
            o.insert("colors".to_owned(), serde_json::Value::Bool(colors));
        }

        // Legacy include_module -> include_module_path
        if let Some(inc_mod) = logging.include_module {
            o.insert("include_module_path".to_owned(), serde_json::Value::Bool(inc_mod));
        }
        if let Some(inc) = logging.include {
            if let Some(v) = inc.module_path {
                o.insert("include_module_path".to_owned(), serde_json::Value::Bool(v));
            }
            if let Some(v) = inc.target {
                o.insert("include_target".to_owned(), serde_json::Value::Bool(v));
            }
            if let Some(v) = inc.file {
                o.insert("include_file".to_owned(), serde_json::Value::Bool(v));
            }
            if let Some(v) = inc.line {
                o.insert("include_line_number".to_owned(), serde_json::Value::Bool(v));
            }
        }

        if let Some(ts) = logging.timestamp {
            o.insert("timestamp".to_owned(), serde_json::Value::String(ts));
        }
        if let Some(indent) = logging.indent {
            o.insert("indent".to_owned(), serde_json::Value::Number(serde_json::Number::from(indent as u64)));
        }
        if let Some(rolling) = logging.rolling {
            if let Some(v) = rolling.max_bytes {
                o.insert("roll_max_bytes".to_owned(), serde_json::Value::Number(serde_json::Number::from(v)));
            }
            if let Some(v) = rolling.max_files {
                o.insert("roll_max_files".to_owned(), serde_json::Value::Number(serde_json::Number::from(v as u64)));
            }
            if let Some(v) = rolling.keep_days {
                o.insert("roll_keep_days".to_owned(), serde_json::Value::Number(serde_json::Number::from(v as u64)));
            }
        }

        cfg.plugins
            .entry("newengine.logging".to_owned())
            .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()))
            .as_object_mut()
            .map(|dst| dst.extend(o));

        report.overrides.push(StartupOverride {
            key: "logging",
            from: "legacy".to_owned(),
            to: "plugins.newengine.logging".to_owned(),
        });
    }

    if let Some(mut plugins) = src.plugins {
        // Deterministic merge: config.json plugins override defaults (plugin-owned).
        // Also emit a report entry per plugin id so the logging plugin can print it later.
        let mut ids: Vec<String> = plugins.keys().cloned().collect();
        ids.sort();

        for id in ids {
            if let Some(v) = plugins.remove(&id) {
                let to = summarize_json(&v);
                cfg.plugins.insert(id.clone(), v);
                report.plugin_overrides.push(StartupPluginOverride {
                    plugin_id: id,
                    key: "plugins.*",
                    from: "<plugin defaults>".to_owned(),
                    to,
                });
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
        // Legacy engine-side asset settings are translated into plugin overrides.
        // NewEngine vNext: AssetManager is a plugin with its own config contract.
        let mut legacy_assets = serde_json::Map::new();

        if let Some(root) = engine.assets_root {
            legacy_assets.insert("assets_root".to_string(), serde_json::Value::String(root));
        }
        if let Some(steps) = engine.asset_pump_steps {
            legacy_assets.insert("pump_steps".to_string(), serde_json::Value::Number((steps as u64).into()));
        }
        if let Some(enabled) = engine.asset_filesystem_source {
            legacy_assets.insert("filesystem".to_string(), serde_json::Value::Bool(enabled));
        }

        if !legacy_assets.is_empty() {
            let pid = "newengine.assets".to_string();
            let entry = cfg
                .plugins
                .entry(pid.clone())
                .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));

            let obj = entry.as_object_mut().unwrap();
            for (k, v) in legacy_assets {
                obj.insert(k, v);
        }

            report.plugin_overrides.push(StartupPluginOverride {
                plugin_id: pid,
                key: "engine.* (legacy assets config)",
                from: "legacy".to_string(),
                to: "plugins.newengine.assets (translated)".to_string(),
            });
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

fn summarize_json(v: &serde_json::Value) -> String {
    // Compact representation with a hard cap to avoid log spam.
    // (Plugins should keep their base config inside the DLL; config.json carries overrides only.)
    const MAX: usize = 512;
    match serde_json::to_string(v) {
        Ok(s) if s.len() <= MAX => s,
        Ok(s) => {
            let mut out = s;
            out.truncate(MAX);
            out.push_str("…");
            out
        }
        Err(_) => "<invalid json>".to_owned(),
    }
}