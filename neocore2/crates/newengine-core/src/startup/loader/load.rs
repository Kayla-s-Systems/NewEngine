pub struct StartupLoader;

impl StartupLoader {
    pub fn load_json(paths: &ConfigPaths) -> EngineResult<(StartupConfig, StartupLoadReport)> {
        let t0 = Instant::now();
        let mut cfg = StartupConfig::default();
        let mut report = StartupLoadReport::new();

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

        // Browser/CLI launch graphics overrides are applied after persisted config
        // but before any optional PreStart presenter and before the active settings
        // snapshot is published. This keeps one authoritative core settings path.
        apply_graphics_process_overrides(&mut cfg, &mut report);

        // Present/record PreStart loading only after config.json is known, so
        // consumer-owned `plugins.engine.loading` manifests can assign bg/logo/spinner.
        let startup_window = crate::startup_window::present_before_startup_if_needed(paths, &cfg);
        if let Some(selection) = startup_window.selection.as_ref() {
            apply_size(
                &mut report,
                "window_size.prestart",
                &mut cfg.window_size,
                selection.window_size,
            );
            apply_placement(
                &mut report,
                "window_placement.prestart",
                &mut cfg.window_placement,
                selection.window_placement,
            );
            let from = format!("{:?}", cfg.launch_settings);
            cfg.launch_settings = selection.launch_settings.clone();
            cfg.launch_settings_explicit = true;
            report.overrides.push(StartupOverride {
                key: "startup_settings.prestart",
                from,
                to: format!("{:?}", cfg.launch_settings),
            });
        }
        if let Some(loading_assignment) = startup_window.loading_assignment.as_ref() {
            report.overrides.push(StartupOverride {
                key: "engine.loading",
                from: "core default / consumer manifest".to_owned(),
                to: loading_assignment.override_summary(),
            });
        }
        if let Some(boot_frame) = startup_window.boot_frame.as_ref() {
            report.overrides.push(StartupOverride {
                key: "engine.loading.boot_frame",
                from: "<none>".to_owned(),
                to: boot_frame.diagnostic_summary(),
            });
        }
        report.overrides.push(StartupOverride {
            key: "startup_window",
            from: "core default".to_owned(),
            to: format!(
                "decision={:?}; details={}; disabled_by={}; warnings={}",
                startup_window.decision,
                startup_window.details,
                startup_window.disabled_by.as_deref().unwrap_or("<none>"),
                startup_window.warnings.len()
            ),
        });

        if matches!(
            startup_window.decision,
            crate::startup_window::StartupWindowDecision::Cancelled
        ) {
            return Err(EngineError::ExitRequested);
        }

        // Publish the core-owned launch snapshot before platform and renderer
        // creation. Rust consumers use `startup_launch_settings()` while plugin
        // and FFI consumers can use the mirrored process variables.
        crate::startup_window::set_startup_launch_settings(cfg.launch_settings.clone());
        report.overrides.push(StartupOverride {
            key: "startup_settings.variables",
            from: "core defaults / saved config".to_owned(),
            to: format!(
                "preset={} msaa={} fxaa={} taa={} ssao={} bloom={} shadows={} window_mode={} vsync={}",
                cfg.launch_settings.graphics.preset.as_str(),
                cfg.launch_settings.graphics.msaa_samples,
                cfg.launch_settings.graphics.fxaa_enabled,
                cfg.launch_settings.graphics.taa_enabled,
                cfg.launch_settings.graphics.ssao_enabled,
                cfg.launch_settings.graphics.bloom_enabled,
                cfg.launch_settings.graphics.shadows_enabled,
                cfg.launch_settings.display.window_mode.as_str(),
                cfg.launch_settings.display.vsync,
            ),
        });

        // Publish engine-level roots as soon as startup config is resolved.
        // CACHE_FILES is disposable generated data. CONFIG is durable user settings.
        publish_startup_storage_roots(&cfg, &mut report);

        report.total_ms = Some(t0.elapsed().as_millis().min(u128::from(u32::MAX)) as u32);
        crate::startup::set_last_load_report(report.clone());
        Ok((cfg, report))
    }

    /// Loads only the persisted startup launch settings without presenting the
    /// PreStart UI or publishing process/global state. Used by the editor-owned
    /// Project Browser to seed its Settings tab before a project is selected.
    pub fn load_launch_settings_preview(
        paths: &ConfigPaths,
    ) -> EngineResult<crate::startup_window::StartupLaunchSettings> {
        let mut settings = crate::startup_window::StartupLaunchSettings::default();
        if let Some((resolved, _)) = resolve_startup_file_optional(paths, paths.startup_path())? {
            let data = fs::read_to_string(&resolved).map_err(|e| {
                EngineError::Other(format!(
                    "startup config preview read failed: path={:?} err={}",
                    resolved, e
                ))
            })?;
            let parsed: RootJson = serde_json::from_str(&data).map_err(|e| {
                EngineError::Other(format!(
                    "startup config preview parse failed (json): path={:?} err={}",
                    resolved, e
                ))
            })?;
            if let Some(mut persisted) = parsed.startup_settings {
                persisted.normalize();
                settings = persisted;
            }
        }
        apply_graphics_process_overrides_to_settings(&mut settings, None);
        Ok(settings)
    }
}
