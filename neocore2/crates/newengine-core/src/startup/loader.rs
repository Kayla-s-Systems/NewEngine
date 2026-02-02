use crate::error::{EngineError, EngineResult};
use crate::startup::{
    ConfigPaths, StartupConfig, StartupConfigSource, StartupDefaults, StartupLoadReport,
    StartupOverride, StartupOverrideSource, StartupOverrides, StartupResolvedFrom, WindowPlacement,
};

use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub struct StartupLoader;

impl StartupLoader {
    /// Loads startup config with layering:
    /// defaults -> file -> env -> programmatic.
    pub fn load_json(
        paths: &ConfigPaths,
        defaults: &StartupDefaults,
    ) -> EngineResult<(StartupConfig, StartupLoadReport)> {
        Self::load_json_with_overrides(paths, defaults, &StartupOverrides::empty())
    }

    /// Loads startup config with layering:
    /// defaults -> file -> env -> programmatic.
    pub fn load_json_with_overrides(
        paths: &ConfigPaths,
        defaults: &StartupDefaults,
        programmatic: &StartupOverrides,
    ) -> EngineResult<(StartupConfig, StartupLoadReport)> {
        // Effective config must be bootable out of the box.
        let mut cfg = StartupConfig::default();

        let mut report = StartupLoadReport {
            source: StartupConfigSource::Defaults,
            file: None,
            resolved_from: StartupResolvedFrom::NotProvided,
            overrides: Vec::new(),
        };

        // 1) Defaults layer (app-level defaults, optional overrides over StartupConfig::default()).
        apply_defaults(&mut cfg, &mut report, defaults);

        cfg.source = StartupConfigSource::Defaults;

        // 2) File layer (optional)
        let mut file_used = false;
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

                    apply_root(&mut cfg, &mut report, StartupOverrideSource::File, parsed);

                    cfg.source = StartupConfigSource::File {
                        path: resolved.clone(),
                    };
                    report.source = cfg.source.clone();
                    file_used = true;
                }
                Ok(None) => {
                    // Missing file is not an error.
                    report.source = StartupConfigSource::Defaults;
                    report.file = None;
                    report.resolved_from = StartupResolvedFrom::NotProvided;
                }
                Err(e) => return Err(e),
            }
        }

        // 3) Env layer
        let env = StartupOverrides::from_env();
        apply_overrides(&mut cfg, &mut report, StartupOverrideSource::Env, &env);

        // 4) Programmatic layer
        apply_overrides(
            &mut cfg,
            &mut report,
            StartupOverrideSource::Programmatic,
            programmatic,
        );

        // Mixed detection
        let mixed = report.overrides.iter().any(|o| {
            o.source == StartupOverrideSource::Env || o.source == StartupOverrideSource::Programmatic
        });

        if mixed {
            cfg.source = StartupConfigSource::Mixed;
            report.source = StartupConfigSource::Mixed;

            // Keep file path info if file existed.
            if !file_used {
                report.file = None;
                report.resolved_from = StartupResolvedFrom::NotProvided;
            }
        }

        Ok((cfg, report))
    }
}

#[derive(Deserialize)]
struct RootJson {
    window: Option<WindowJson>,
    logging: Option<LoggingJson>,
    engine: Option<EngineJson>,
    modules: Option<ModulesJson>,
    extra: Option<HashMap<String, String>>,
}

#[derive(Deserialize)]
struct LoggingJson {
    level: Option<String>,
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
    fixed_dt_ms: Option<u32>,
    assets_root: Option<String>,
    asset_budget: Option<u32>,
    init_host_context: Option<bool>,
}

#[derive(Deserialize)]
struct ModulesJson {
    dir: Option<String>,
}

fn apply_defaults(cfg: &mut StartupConfig, report: &mut StartupLoadReport, defaults: &StartupDefaults) {
    if let Some(v) = defaults.log_level.clone() {
        apply_string(report, StartupOverrideSource::Defaults, "log_level", &mut cfg.log_level, v);
    }

    if let Some(v) = defaults.window_title.clone() {
        apply_string(
            report,
            StartupOverrideSource::Defaults,
            "window_title",
            &mut cfg.window_title,
            v,
        );
    }

    if let Some(v) = defaults.window_size {
        apply_size(
            report,
            StartupOverrideSource::Defaults,
            "window_size",
            &mut cfg.window_size,
            v,
        );
    }

    if let Some(v) = defaults.window_placement.clone() {
        apply_placement(
            report,
            StartupOverrideSource::Defaults,
            "window_placement",
            &mut cfg.window_placement,
            v,
        );
    }

    if let Some(v) = defaults.fixed_dt_ms {
        apply_u32(
            report,
            StartupOverrideSource::Defaults,
            "fixed_dt_ms",
            &mut cfg.fixed_dt_ms,
            v,
        );
    }

    if let Some(v) = defaults.assets_root.clone() {
        apply_path(
            report,
            StartupOverrideSource::Defaults,
            "assets_root",
            &mut cfg.assets_root,
            v,
        );
    }

    if let Some(v) = defaults.asset_budget {
        apply_u32(
            report,
            StartupOverrideSource::Defaults,
            "asset_budget",
            &mut cfg.asset_budget,
            v,
        );
    }

    if let Some(v) = defaults.init_host_context {
        apply_bool(
            report,
            StartupOverrideSource::Defaults,
            "init_host_context",
            &mut cfg.init_host_context,
            v,
        );
    }

    if let Some(v) = defaults.modules_dir.clone() {
        apply_path(
            report,
            StartupOverrideSource::Defaults,
            "modules_dir",
            &mut cfg.modules_dir,
            v,
        );
    }
}

fn apply_root(
    cfg: &mut StartupConfig,
    report: &mut StartupLoadReport,
    source: StartupOverrideSource,
    src: RootJson,
) {
    if let Some(logging) = src.logging {
        if let Some(level) = logging.level {
            apply_string(report, source, "log_level", &mut cfg.log_level, level);
        }
    }

    if let Some(w) = src.window {
        if let Some(t) = w.title {
            apply_string(report, source, "window_title", &mut cfg.window_title, t);
        }

        if let Some([ww, hh]) = w.size {
            apply_size(report, source, "window_size", &mut cfg.window_size, (ww, hh));
        } else {
            match (w.width, w.height) {
                (Some(ww), Some(hh)) => {
                    apply_size(report, source, "window_size", &mut cfg.window_size, (ww, hh));
                }
                (Some(_), None) | (None, Some(_)) => {
                    report.overrides.push(StartupOverride {
                        key: "window_size",
                        source,
                        from: format_size(cfg.window_size),
                        to: "ignored (width/height must both be present)".to_owned(),
                    });
                }
                (None, None) => {}
            }
        }

        if let Some(p) = w.placement {
            if let Some(pl) = parse_placement(p) {
                apply_placement(report, source, "window_placement", &mut cfg.window_placement, pl);
            }
        }
    }

    if let Some(engine) = src.engine {
        if let Some(v) = engine.fixed_dt_ms {
            apply_u32(report, source, "fixed_dt_ms", &mut cfg.fixed_dt_ms, v);
        }
        if let Some(p) = engine.assets_root {
            apply_path(report, source, "assets_root", &mut cfg.assets_root, PathBuf::from(p));
        }
        if let Some(b) = engine.asset_budget {
            apply_u32(report, source, "asset_budget", &mut cfg.asset_budget, b);
        }
        if let Some(v) = engine.init_host_context {
            apply_bool(
                report,
                source,
                "init_host_context",
                &mut cfg.init_host_context,
                v,
            );
        }
    }

    if let Some(mods) = src.modules {
        if let Some(d) = mods.dir {
            apply_path(report, source, "modules_dir", &mut cfg.modules_dir, PathBuf::from(d));
        }
    }

    if let Some(extra) = src.extra {
        for (k, v) in extra {
            apply_extra(report, source, &mut cfg.extra, k, v);
        }
    }
}

fn apply_overrides(
    cfg: &mut StartupConfig,
    report: &mut StartupLoadReport,
    source: StartupOverrideSource,
    ov: &StartupOverrides,
) {
    if let Some(v) = ov.log_level.clone() {
        apply_string(report, source, "log_level", &mut cfg.log_level, v);
    }

    if let Some(v) = ov.window_title.clone() {
        apply_string(report, source, "window_title", &mut cfg.window_title, v);
    }

    if let Some(v) = ov.window_size {
        apply_size(report, source, "window_size", &mut cfg.window_size, v);
    }

    if let Some(v) = ov.window_placement.clone() {
        apply_placement(report, source, "window_placement", &mut cfg.window_placement, v);
    }

    if let Some(v) = ov.fixed_dt_ms {
        apply_u32(report, source, "fixed_dt_ms", &mut cfg.fixed_dt_ms, v);
    }

    if let Some(v) = ov.assets_root.clone() {
        apply_path(report, source, "assets_root", &mut cfg.assets_root, v);
    }

    if let Some(v) = ov.asset_budget {
        apply_u32(report, source, "asset_budget", &mut cfg.asset_budget, v);
    }

    if let Some(v) = ov.init_host_context {
        apply_bool(report, source, "init_host_context", &mut cfg.init_host_context, v);
    }

    if let Some(v) = ov.modules_dir.clone() {
        apply_path(report, source, "modules_dir", &mut cfg.modules_dir, v);
    }

    for (k, v) in &ov.extra {
        apply_extra(report, source, &mut cfg.extra, k.clone(), v.clone());
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

fn apply_string(
    report: &mut StartupLoadReport,
    source: StartupOverrideSource,
    key: &'static str,
    slot: &mut String,
    to: String,
) {
    let from = slot.clone();
    if from == to {
        return;
    }
    *slot = to.clone();
    report.overrides.push(StartupOverride {
        key,
        source,
        from,
        to,
    });
}

fn apply_u32(
    report: &mut StartupLoadReport,
    source: StartupOverrideSource,
    key: &'static str,
    slot: &mut u32,
    to: u32,
) {
    let from = slot.to_string();
    if *slot == to {
        return;
    }
    *slot = to;
    report.overrides.push(StartupOverride {
        key,
        source,
        from,
        to: to.to_string(),
    });
}

fn apply_bool(
    report: &mut StartupLoadReport,
    source: StartupOverrideSource,
    key: &'static str,
    slot: &mut bool,
    to: bool,
) {
    let from = slot.to_string();
    if *slot == to {
        return;
    }
    *slot = to;
    report.overrides.push(StartupOverride {
        key,
        source,
        from,
        to: to.to_string(),
    });
}

fn apply_path(
    report: &mut StartupLoadReport,
    source: StartupOverrideSource,
    key: &'static str,
    slot: &mut PathBuf,
    to: PathBuf,
) {
    let from = slot.display().to_string();
    let to_s = to.display().to_string();
    if from == to_s {
        return;
    }
    *slot = to;
    report.overrides.push(StartupOverride {
        key,
        source,
        from,
        to: to_s,
    });
}

fn apply_size(
    report: &mut StartupLoadReport,
    source: StartupOverrideSource,
    key: &'static str,
    slot: &mut (u32, u32),
    to: (u32, u32),
) {
    let from = format_size(*slot);
    let to_s = format_size(to);
    if from == to_s {
        return;
    }
    *slot = to;
    report.overrides.push(StartupOverride {
        key,
        source,
        from,
        to: to_s,
    });
}

fn apply_placement(
    report: &mut StartupLoadReport,
    source: StartupOverrideSource,
    key: &'static str,
    slot: &mut WindowPlacement,
    to: WindowPlacement,
) {
    let from = format_placement(slot);
    let to_s = format_placement(&to);
    if from == to_s {
        return;
    }
    *slot = to;
    report.overrides.push(StartupOverride {
        key,
        source,
        from,
        to: to_s,
    });
}

fn apply_extra(
    report: &mut StartupLoadReport,
    source: StartupOverrideSource,
    slot: &mut HashMap<String, String>,
    k: String,
    v: String,
) {
    let from_v = slot.get(&k).cloned().unwrap_or_else(|| "<unset>".to_owned());
    if from_v == v {
        return;
    }
    slot.insert(k.clone(), v.clone());
    report.overrides.push(StartupOverride {
        key: "extra",
        source,
        from: format!("{}={}", k, from_v),
        to: format!("{}={}", k, v),
    });
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