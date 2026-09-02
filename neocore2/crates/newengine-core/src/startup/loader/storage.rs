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
            let Some(v) = extra.remove(*key) else {
                continue;
            };
            let Some(path) = v.as_str().map(str::trim).filter(|s| !s.is_empty()) else {
                continue;
            };
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
