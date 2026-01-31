use crate::error::{EngineError, EngineResult};
use crate::startup_config::{
    ConfigPaths, StartupConfig, StartupConfigSource, StartupDefaults, StartupLoadReport,
    StartupOverride, StartupResolvedFrom, WindowPlacement,
};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

pub struct StartupLoader;

impl StartupLoader {
    /// Optional startup config:
    /// - if file missing => fall back to defaults (no error)
    /// - if file exists but invalid/unreadable => return error
    pub fn load_json(
        paths: &ConfigPaths,
        defaults: &StartupDefaults,
    ) -> EngineResult<(StartupConfig, StartupLoadReport)> {
        let mut cfg = StartupConfig::default();
        let mut report = StartupLoadReport {
            source: StartupConfigSource::Defaults,
            file: None,
            resolved_from: StartupResolvedFrom::NotProvided,
            overrides: Vec::new(),
        };

        // Defaults
        cfg.log_level = defaults.log_level.clone();
        cfg.window_title = defaults.window_title.clone();
        cfg.window_size = defaults.window_size;
        cfg.window_placement = defaults.window_placement.clone();
        cfg.modules_dir = defaults.modules_dir.clone();
        cfg.source = StartupConfigSource::Defaults;

        // Optional file
        if let Some(raw_path) = paths.startup_path() {
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
                    // Not found => keep defaults, no overrides, no error.
                    report.source = StartupConfigSource::Defaults;
                    report.file = None;
                    report.resolved_from = StartupResolvedFrom::NotProvided;
                }
                Err(e) => return Err(e),
            }
        }

        Ok((cfg, report))
    }
}

#[derive(Deserialize)]
struct RootJson {
    window: Option<WindowJson>,
    logging: Option<LoggingJson>,
    #[allow(dead_code)]
    engine: Option<serde_json::Value>,
    #[allow(dead_code)]
    render: Option<serde_json::Value>,
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
                (Some(ww), Some(hh)) => apply_size(report, "window_size", &mut cfg.window_size, (ww, hh)),
                (Some(_), None) | (None, Some(_)) => report.overrides.push(StartupOverride {
                    key: "window_size",
                    from: format_opt_size(cfg.window_size),
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
}

fn parse_placement(p: WindowPlacementJson) -> Option<WindowPlacement> {
    let kind = p.kind.unwrap_or_else(|| "default".to_owned()).to_ascii_lowercase();
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

fn apply_string(
    report: &mut StartupLoadReport,
    key: &'static str,
    slot: &mut Option<String>,
    to: String,
) {
    let from = slot.clone().unwrap_or_else(|| "<unset>".to_owned());
    *slot = Some(to.clone());
    report.overrides.push(StartupOverride { key, from, to });
}

fn apply_size(
    report: &mut StartupLoadReport,
    key: &'static str,
    slot: &mut Option<(u32, u32)>,
    to: (u32, u32),
) {
    let from = format_opt_size(*slot);
    *slot = Some(to);
    report.overrides.push(StartupOverride {
        key,
        from,
        to: format!("{}x{}", to.0, to.1),
    });
}

fn apply_placement(
    report: &mut StartupLoadReport,
    key: &'static str,
    slot: &mut Option<WindowPlacement>,
    to: WindowPlacement,
) {
    let from = slot
        .as_ref()
        .map(format_placement)
        .unwrap_or_else(|| "<unset>".to_owned());
    *slot = Some(to.clone());
    report.overrides.push(StartupOverride {
        key,
        from,
        to: format_placement(&to),
    });
}

fn format_placement(p: &WindowPlacement) -> String {
    match p {
        WindowPlacement::Centered { offset } => format!("centered(offset={} {})", offset.0, offset.1),
        WindowPlacement::Default => "default".to_owned(),
    }
}

fn format_opt_size(v: Option<(u32, u32)>) -> String {
    v.map(|(w, h)| format!("{}x{}", w, h))
        .unwrap_or_else(|| "<unset>".to_owned())
}

/// Optional resolver:
/// - returns Ok(Some(...)) when found
/// - returns Ok(None) when not found
/// - returns Err only on real unexpected errors
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