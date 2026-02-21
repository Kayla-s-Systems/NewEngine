#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::{Path, PathBuf};

use newengine_plugin_api::HostApiV1;

use crate::plugins::install_forward_logger_once;
use crate::plugins::paths::{default_plugins_dir, is_dynamic_lib, resolve_plugins_dir};

use super::types::PluginLoadError;
use super::PluginManager;

impl PluginManager {
    pub fn load_default(&mut self, host: HostApiV1) -> Result<(), PluginLoadError> {
        let dir = default_plugins_dir()?;
        self.load_from_dir(&dir, host)
    }

    pub fn load_from_dir(&mut self, dir: &Path, host: HostApiV1) -> Result<(), PluginLoadError> {
        let dir = resolve_plugins_dir(dir)?;

        if let Err(e) = std::fs::create_dir_all(&dir) {
            return Err(PluginLoadError {
                path: dir.clone(),
                message: format!("create_dir_all failed: {e}"),
            });
        }

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
        let (mut loggers, mut rest): (Vec<PathBuf>, Vec<PathBuf>) =
            candidates.into_iter().partition(|p| is_logging_candidate(p));

        loggers.sort();
        rest.sort();

        // 1) Try to load logging plugins first (best-effort).
        for path in &loggers {
            let _ = self.load_one(path, host.clone());
        }

        // 2) Install forward logger after potential logging plugin init().
        install_forward_logger_once(host.clone());

        // 3) Emit deferred startup diagnostics (now log backend can exist).
        if let Some(r) = crate::startup::last_load_report() {
            r.emit_logs();
        }

        log::info!("plugins: scanning directory '{}'", dir.display());
        log::info!(
            "plugins: found {} candidate(s) in '{}'",
            loggers.len() + rest.len(),
            dir.display()
        );

        // 4) Load the rest.
        for path in rest {
            match self.load_one(&path, host.clone()) {
                Ok(()) => {}
                Err(e) => {
                    log::warn!("plugins: failed to load '{}': {}", path.display(), e);
                }
            }
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