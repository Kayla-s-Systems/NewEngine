#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::{Path, PathBuf};

use newengine_plugin_api::HostApiV1;

use crate::path_fmt::{canonicalize_if_exists, display_clean};
use crate::plugins::install_forward_logger_once;
use crate::plugins::paths::{default_plugins_dir, is_dynamic_lib, resolve_plugins_dir};

use super::types::PluginLoadError;
use super::PluginManager;

impl PluginManager {
    #[inline]
    pub fn load_default(&mut self, host: HostApiV1) -> Result<(), PluginLoadError> {
        self.load_default_with_policy(host, false)
    }

    #[inline]
    pub fn load_default_with_policy(
        &mut self,
        host: HostApiV1,
        strict: bool,
    ) -> Result<(), PluginLoadError> {
        let dir = default_plugins_dir()?;
        self.load_from_dir_with_policy(&dir, host, strict)
    }

    #[inline]
    pub fn load_from_dir(&mut self, dir: &Path, host: HostApiV1) -> Result<(), PluginLoadError> {
        self.load_from_dir_with_policy(dir, host, false)
    }

    pub fn load_from_dir_with_policy(
        &mut self,
        dir: &Path,
        host: HostApiV1,
        strict: bool,
    ) -> Result<(), PluginLoadError> {
        let dir = resolve_plugins_dir(dir)?;

        if let Err(e) = std::fs::create_dir_all(&dir) {
            return Err(PluginLoadError {
                path: dir.clone(),
                message: format!("create_dir_all failed: {e}"),
            });
        }

        // Now that it exists, canonicalize to eliminate `..` / `.` segments.
        let dir = canonicalize_if_exists(&dir);

        let rd = std::fs::read_dir(&dir).map_err(|e| PluginLoadError {
            path: dir.clone(),
            message: format!("read_dir failed: {e}"),
        })?;

        let mut candidates: Vec<PathBuf> = Vec::new();
        for ent in rd {
            let ent = ent.map_err(|e| PluginLoadError {
                path: dir.clone(),
                message: format!("read_dir entry failed: {e}"),
            })?;

            let p = ent.path();
            if !is_dynamic_lib(&p) {
                continue;
            }
            candidates.push(p);
        }

        fn is_logging_candidate(p: &Path) -> bool {
            p.file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.to_ascii_lowercase())
                .is_some_and(|s| s.contains("logging"))
        }

        candidates.sort();
        let (mut loggers, mut rest): (Vec<PathBuf>, Vec<PathBuf>) = candidates
            .into_iter()
            .partition(|p| is_logging_candidate(p));

        loggers.sort();
        rest.sort();

        log::debug!(
            "plugins: candidates loggers={} rest={} (logger-first load)",
            loggers.len(),
            rest.len()
        );
        if log::log_enabled!(log::Level::Debug) {
            for p in loggers.iter().chain(rest.iter()) {
                log::debug!("plugins: candidate '{}'", p.display());
            }
        }

        let mut load_errors: Vec<PluginLoadError> = Vec::new();

        // 1) Try to load logging plugins first.
        for path in &loggers {
            if let Err(e) = self.load_one(path, host.clone()) {
                log::warn!("plugins: failed to load '{}': {}", display_clean(path), e);
                load_errors.push(e);
            }
        }

        // If startup config specifies overrides for the logging plugin but we found no logging candidate,
        // it's usually a packaging/layout issue (wrong modules_dir, missing DLL, wrong name, etc.).
        if loggers.is_empty() {
            if let Some(r) = crate::startup::last_load_report() {
                let has_logging_override = r
                    .plugin_overrides
                    .iter()
                    .any(|o| o.plugin_id == "newengine.logging");
                if has_logging_override {
                    log::warn!(
                        "plugins: no logging plugin candidate found in '{}' but startup has overrides for 'newengine.logging'",
                        display_clean(&dir)
                    );
                }
            }
        }

        // 2) Install forward logger after potential logging plugin init().
        install_forward_logger_once(host.clone());

        // 3) Emit deferred startup diagnostics (now log backend can exist).
        crate::startup::SystemProbe::probe().emit_table("startup");

        if let Some(r) = crate::startup::last_load_report() {
            r.emit_logs();
        }

        log::info!("plugins: scanning directory '{}'", display_clean(&dir));
        log::info!(
            "plugins: found {} candidate(s) in '{}'",
            loggers.len() + rest.len(),
            display_clean(&dir)
        );

        // 4) Load the rest.
        for path in rest {
            match self.load_one(&path, host.clone()) {
                Ok(()) => {}
                Err(e) => {
                    log::warn!("plugins: failed to load '{}': {}", display_clean(&path), e);
                    load_errors.push(e);
                }
            }
        }

        log::info!("plugins: load complete loaded_count={}", self.loaded.len());

        // Enforce declared capability dependencies before starting plugins.
        self.validate_required_capabilities();
        if log::log_enabled!(log::Level::Debug) {
            for p in self.loaded.iter() {
                log::debug!(
                    "plugins: loaded '{}' ver='{}' path='{}'",
                    p.info.id,
                    p.info.version,
                    display_clean(&p.path)
                );
            }
        }

        if strict && !load_errors.is_empty() {
            let mut msg = String::new();
            use std::fmt::Write as _;
            let _ = writeln!(
                msg,
                "one or more plugins failed to load (count={}):",
                load_errors.len()
            );
            for e in load_errors.iter() {
                let _ = writeln!(msg, "- path='{}' err='{}'", display_clean(&e.path), e.message);
            }

            return Err(PluginLoadError {
                path: dir.clone(),
                message: msg,
            });
        }

        Ok(())
    }

    #[inline]
    pub fn load_path(&mut self, path: &Path, host: HostApiV1) -> Result<(), PluginLoadError> {
        let res = self.load_one(path, host.clone());
        install_forward_logger_once(host);
        res
    }
}
