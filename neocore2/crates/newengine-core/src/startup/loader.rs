#![forbid(unsafe_op_in_unsafe_fn)]

use crate::error::{EngineError, EngineResult};
use crate::startup::config::StartupPluginOverride;
use crate::startup::{
    ConfigPaths, StartupConfig, StartupConfigSource, StartupLoadReport, StartupOverride,
    StartupResolvedFrom, StartupStorageRootKind, WindowPlacement,
};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub struct StartupLoader;

impl StartupLoader {
    pub fn load_json(paths: &ConfigPaths) -> EngineResult<(StartupConfig, StartupLoadReport)> {
        let t0 = Instant::now();
        let mut cfg = StartupConfig::default();
        let mut report = StartupLoadReport::new();

        let startup_window = crate::startup_window::present_before_startup_if_needed(paths);
        report.overrides.push(StartupOverride {
            key: "startup_window",
            from: "core default".to_owned(),
            to: format!(
                "decision={:?}; details={}; disabled_by={}; warnings={}",
                startup_window.decision,
                startup_window.details,
                startup_window
                    .disabled_by
                    .as_deref()
                    .unwrap_or("<none>"),
                startup_window.warnings.len()
            ),
        });

        if matches!(startup_window.decision, crate::startup_window::StartupWindowDecision::Cancelled) {
            return Err(EngineError::ExitRequested);
        }

        let raw_path = paths.startup_path();

        match resolve_startup_file_optional(paths, raw_path) {
            Ok(Some((resolved, from))) => {
                report.file = Some(resolved.clone());
                report.resolved_from = from;

                let meta_len = fs::metadata(&resolved).ok().map(|m| m.len() as usize);
                report.file_bytes = meta_len;

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

        // Publish engine-level roots as soon as startup config is resolved.
        // CACHE_FILES is disposable generated data. CONFIG is durable user settings.
        publish_startup_storage_roots(&cfg, &mut report);

        report.total_ms = Some(t0.elapsed().as_millis().min(u128::from(u32::MAX)) as u32);
        crate::startup::set_last_load_report(report.clone());
        crate::startup::set_last_startup_config(cfg.clone());
        Ok((cfg, report))
    }
}

#[derive(Deserialize)]
struct RootJson {
    window: Option<WindowJson>,
    engine: Option<EngineJson>,
    plugins: Option<newengine_math::collections_prelude::NeHashMap<String, serde_json::Value>>,

    #[serde(flatten)]
    extra: newengine_math::collections_prelude::NeHashMap<String, serde_json::Value>,
}

#[derive(Deserialize)]
struct WindowJson {
    title: Option<String>,

    size: Option<[u32; 2]>,
    width: Option<u32>,
    height: Option<u32>,

    placement: Option<WindowPlacementJson>,

    /// Logical path inside assets, e.g. "ui/icons/builtin_icons.ytd@app_logo"
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
    modules_dir: Option<String>,
    cache_files: Option<String>,
    #[serde(rename = "CACHE_FILES")]
    cache_files_upper: Option<String>,
    config: Option<String>,
    #[serde(rename = "CONFIG")]
    config_upper: Option<String>,

    /// Unknown keys are preserved to produce deterministic diagnostics.
    ///
    /// Engine-side asset settings are intentionally NOT supported anymore:
    /// assets must be configured via the AssetManager plugin (`plugins.newengine.assets`).
    #[serde(flatten)]
    extra: newengine_math::collections_prelude::NeHashMap<String, serde_json::Value>,
}


fn apply_root(cfg: &mut StartupConfig, report: &mut StartupLoadReport, mut src: RootJson) {
    apply_root_level_storage_paths(cfg, report, &mut src.extra);

    if !src.extra.is_empty() {
        let mut keys: Vec<String> = src.extra.keys().cloned().collect();
        keys.sort();
        report.overrides.push(StartupOverride {
            key: "root.*",
            from: "provided".to_owned(),
            to: format!("ignored unknown startup keys: {}", keys.join(", ")),
        });
    }

    if let Some(mut plugins) = src.plugins {
        // Deterministic merge: config.json plugins override defaults (plugin-owned).
        // Raw roots are preserved so the config service can resolve either:
        // - flat ids: plugins["newengine.logging"]
        // - wrapped domains: plugins.newengine.logging
        let mut ids: Vec<String> = plugins.keys().cloned().collect();
        ids.sort();

        for id in ids {
            if let Some(v) = plugins.remove(&id) {
                collect_plugin_override_report_entries(&id, &v, &mut report.plugin_overrides);
                cfg.plugins.insert(id, v);
            }
        }

        dedup_plugin_override_report_entries(&mut report.plugin_overrides);
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
        if let Some(dir) = engine.modules_dir {
            apply_path(report, "modules_dir", &mut cfg.modules_dir, dir);
        }

        apply_engine_storage_path(
            report,
            cfg,
            StartupStorageRootKind::CacheFiles,
            engine.cache_files.or(engine.cache_files_upper),
        );
        apply_engine_storage_path(
            report,
            cfg,
            StartupStorageRootKind::Config,
            engine.config.or(engine.config_upper),
        );

        if !engine.extra.is_empty() {
            let mut keys: Vec<String> = engine.extra.keys().cloned().collect();
            keys.sort();
            report.overrides.push(StartupOverride {
                key: "engine.*",
                from: "provided".to_owned(),
                to: format!(
                    "ignored unknown engine keys: {} (configure plugins via `plugins.*`)",
                    keys.join(", ")
                ),
            });
        }
    }


}

fn publish_startup_storage_roots(cfg: &StartupConfig, report: &mut StartupLoadReport) {
    for kind in StartupStorageRootKind::ALL {
        let root = cfg.publish_storage_root_env(kind);
        report.overrides.push(StartupOverride {
            key: kind.key(),
            from: "<resolved>".to_owned(),
            to: display_storage_root(kind, &root),
        });
    }
}

fn apply_root_level_storage_paths(
    cfg: &mut StartupConfig,
    report: &mut StartupLoadReport,
    extra: &mut newengine_math::collections_prelude::NeHashMap<String, serde_json::Value>,
) {
    for kind in StartupStorageRootKind::ALL {
        for key in kind.config_keys() {
            let Some(v) = extra.remove(*key) else { continue; };
            let Some(path) = v.as_str().map(str::trim).filter(|s| !s.is_empty()) else { continue; };
            apply_storage_path(report, cfg, kind, path.to_owned());
        }
    }
}

fn apply_engine_storage_path(
    report: &mut StartupLoadReport,
    cfg: &mut StartupConfig,
    kind: StartupStorageRootKind,
    path: Option<String>,
) {
    if let Some(path) = path {
        apply_storage_path(report, cfg, kind, path);
    }
}

fn apply_storage_path(
    report: &mut StartupLoadReport,
    cfg: &mut StartupConfig,
    kind: StartupStorageRootKind,
    value: String,
) {
    apply_path(report, kind.key(), cfg.storage_root_mut(kind), value);
}

fn display_storage_root(kind: StartupStorageRootKind, path: &Path) -> String {
    match kind {
        StartupStorageRootKind::CacheFiles => crate::cache_files::display_cache_path(path),
        StartupStorageRootKind::Config => crate::config_root::display_config_path(path),
    }
}

fn collect_plugin_override_report_entries(
    root_key: &str,
    value: &serde_json::Value,
    out: &mut Vec<StartupPluginOverride>,
) {
    fn visit(
        path: String,
        value: &serde_json::Value,
        out: &mut Vec<StartupPluginOverride>,
        is_root: bool,
    ) {
        match value {
            serde_json::Value::Object(map) => {
                let is_leaf_like =
                    map.is_empty() || map.keys().any(|key| key.contains('.')) || map.values().any(|v| !v.is_object());

                if is_leaf_like {
                    out.push(StartupPluginOverride {
                        plugin_id: path,
                        key: "plugins.*",
                        from: "<plugin defaults>".to_owned(),
                        to: summarize_json(value),
                    });
                    return;
                }

                if is_root && map.is_empty() {
                    out.push(StartupPluginOverride {
                        plugin_id: path,
                        key: "plugins.*",
                        from: "<plugin defaults>".to_owned(),
                        to: summarize_json(value),
                    });
                    return;
                }

                let mut keys: Vec<&String> = map.keys().collect();
                keys.sort();
                for key in keys {
                    if let Some(child) = map.get(key) {
                        visit(format!("{path}.{key}"), child, out, false);
                    }
                }
            }
            _ => {
                out.push(StartupPluginOverride {
                    plugin_id: path,
                    key: "plugins.*",
                    from: "<plugin defaults>".to_owned(),
                    to: summarize_json(value),
                });
            }
        }
    }

    visit(root_key.to_owned(), value, out, true);
}

fn dedup_plugin_override_report_entries(entries: &mut Vec<StartupPluginOverride>) {
    let mut by_id = newengine_math::collections_prelude::NeBTreeMap::<String, StartupPluginOverride>::new();
    for entry in entries.drain(..) {
        by_id.insert(entry.plugin_id.clone(), entry);
    }
    *entries = by_id.into_values().collect();
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


#[inline]
fn apply_string(report: &mut StartupLoadReport, key: &'static str, dst: &mut String, v: String) {
    let from = dst.clone();
    if from != v {
        *dst = v.clone();
        report.overrides.push(StartupOverride { key, from, to: v });
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

    let roots = startup_search_roots()?;
    for root in &roots {
        let in_root = root.join(p);
        if in_root.exists() {
            return Ok(Some((in_root, StartupResolvedFrom::Cwd)));
        }

        // IDE launches from the outer repository root should still find
        // NewEngine/neocore2/config.json when the app spec says "config.json".
        let in_nested_neocore = root.join("NewEngine").join("neocore2").join(p);
        if in_nested_neocore.exists() {
            return Ok(Some((in_nested_neocore, StartupResolvedFrom::Cwd)));
        }
    }

    Ok(None)
}

fn startup_search_roots() -> EngineResult<Vec<PathBuf>> {
    let mut roots = Vec::new();
    let cwd = std::env::current_dir()
        .map_err(|e| EngineError::Other(format!("startup: current_dir failed err={}", e)))?;
    push_root_with_ancestors(&mut roots, cwd, 8);

    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            push_root_with_ancestors(&mut roots, parent.to_path_buf(), 10);
        }
    }

    Ok(roots)
}

fn push_root_with_ancestors(out: &mut Vec<PathBuf>, mut root: PathBuf, max_up: usize) {
    for _ in 0..=max_up {
        if !out.iter().any(|existing| existing == &root) {
            out.push(root.clone());
        }
        if !root.pop() {
            break;
        }
    }
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
            out.push_str("...");
            out
        }
        Err(_) => "<invalid json>".to_owned(),
    }
}
