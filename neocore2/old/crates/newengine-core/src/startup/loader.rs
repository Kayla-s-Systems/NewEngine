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
        let mut report = StartupLoadReport {
            source: StartupConfigSource::Defaults,
            file: None,
            resolved_from: StartupResolvedFrom::NotProvided,
            overrides: Vec::new(),
        };

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
    level: Option<String>,
    #[allow(dead_code)]
    colors: Option<bool>,
    #[allow(dead_code)]
    include_module: Option<bool>,
}

#[derive(Deserialize)]
struct WindowJson {
    title: Option<String>,

    size: Option<[u32; 2]>,
    width: Option<u32>,
    height: Option<u32>,

    placement: Option<WindowPlacementJson>,
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
        if let Some(level) = logging.level {
            apply_string(report, "log_level", &mut cfg.log_level, level);
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

fn apply_string(report: &mut StartupLoadReport, key: &'static str, slot: &mut String, to: String) {
    let from = slot.clone();
    *slot = to.clone();
    report.overrides.push(StartupOverride { key, from, to });
}

fn apply_size(
    report: &mut StartupLoadReport,
    key: &'static str,
    slot: &mut (u32, u32),
    to: (u32, u32),
) {
    let from = format_size(*slot);
    *slot = to;
    report.overrides.push(StartupOverride {
        key,
        from,
        to: format_size(to),
    });
}

fn apply_placement(
    report: &mut StartupLoadReport,
    key: &'static str,
    slot: &mut WindowPlacement,
    to: WindowPlacement,
) {
    let from = format_placement(slot);
    *slot = to.clone();
    report.overrides.push(StartupOverride {
        key,
        from,
        to: format_placement(&to),
    });
}

fn apply_path(report: &mut StartupLoadReport, key: &'static str, slot: &mut PathBuf, to: String) {
    let from = slot.display().to_string();
    *slot = PathBuf::from(&to);
    report.overrides.push(StartupOverride { key, from, to });
}

fn apply_u32(report: &mut StartupLoadReport, key: &'static str, slot: &mut u32, to: u32) {
    let from = slot.to_string();
    *slot = to;
    report.overrides.push(StartupOverride {
        key,
        from,
        to: to.to_string(),
    });
}

fn apply_bool(report: &mut StartupLoadReport, key: &'static str, slot: &mut bool, to: bool) {
    let from = slot.to_string();
    *slot = to;
    report.overrides.push(StartupOverride {
        key,
        from,
        to: to.to_string(),
    });
}

fn apply_color(
    report: &mut StartupLoadReport,
    key: &'static str,
    slot: &mut [f32; 4],
    to: [f32; 4],
) {
    let from = format_color(*slot);
    *slot = to;
    report.overrides.push(StartupOverride {
        key,
        from,
        to: format_color(to),
    });
}

fn apply_ui_backend(
    report: &mut StartupLoadReport,
    key: &'static str,
    slot: &mut UiBackend,
    to: UiBackend,
) {
    let from = slot.as_str().to_owned();
    let to_s = to.as_str().to_owned();
    *slot = to;
    report.overrides.push(StartupOverride { key, from, to: to_s });
}

fn format_placement(p: &WindowPlacement) -> String {
    match p {
        WindowPlacement::Centered { offset } => {
            format!("centered(offset={} {})", offset.0, offset.1)
        }
        WindowPlacement::Default => "default".to_owned(),
    }
}

fn format_size(v: (u32, u32)) -> String {
    format!("{}x{}", v.0, v.1)
}

fn format_color(v: [f32; 4]) -> String {
    format!("{:.3},{:.3},{:.3},{:.3}", v[0], v[1], v[2], v[3])
}

fn resolve_startup_file_optional(
    paths: &ConfigPaths,
    raw: &Path,
) -> EngineResult<Option<(PathBuf, StartupResolvedFrom)>> {
    if raw.is_absolute() {
        return Ok(if raw.is_file() {
            Some((raw.to_path_buf(), StartupResolvedFrom::Absolute))
        } else {
            None
        });
    }

    if let Ok(cwd) = std::env::current_dir() {
        let p = cwd.join(raw);
        if p.is_file() {
            return Ok(Some((p, StartupResolvedFrom::Cwd)));
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let p = dir.join(raw);
            if p.is_file() {
                return Ok(Some((p, StartupResolvedFrom::ExeDir)));
            }
        }
    }

    if let Some(root) = paths.root_dir.as_deref() {
        let p = root.join(raw);
        if p.is_file() {
            return Ok(Some((p, StartupResolvedFrom::RootDir)));
        }
    }

    let as_is = raw.to_path_buf();
    if as_is.is_file() {
        return Ok(Some((as_is, StartupResolvedFrom::AsIs)));
    }

    Ok(None)
}