fn apply_root(cfg: &mut StartupConfig, report: &mut StartupLoadReport, mut src: RootJson) {
    apply_root_level_storage_paths(cfg, report, &mut src.extra);

    if let Some(mut settings) = src.startup_settings.take() {
        settings.normalize();
        let from = format!("{:?}", cfg.launch_settings);
        cfg.launch_settings = settings;
        cfg.launch_settings_explicit = true;
        report.overrides.push(StartupOverride {
            key: "startup_settings",
            from,
            to: format!("{:?}", cfg.launch_settings),
        });
    }

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
        // - flat ids: plugins["engine.logging.chronicle"]
        // - wrapped domains: plugins.engine.logging.chronicle
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
