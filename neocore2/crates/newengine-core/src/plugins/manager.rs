#![forbid(unsafe_op_in_unsafe_fn)]

use libloading::Library;
use newengine_plugin_api::{HostApiV1, PluginInfo, PluginModuleDyn, PluginRootV1Ref, ServiceV1Dyn};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::plugins::host_api::{
    host_register_service_impl, with_importer_load_state, ImporterLoadState,
};
use crate::plugins::paths::{default_plugins_dir, is_dynamic_lib, resolve_plugins_dir};

#[derive(Debug)]
pub struct PluginLoadError {
    pub path: PathBuf,
    pub message: String,
}

impl std::fmt::Display for PluginLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.message)
    }
}

impl std::error::Error for PluginLoadError {}

struct LoadedPlugin {
    _lib: Library,
    module: PluginModuleDyn<'static>,
    info: PluginInfo,
}

pub struct PluginManager {
    loaded: Vec<LoadedPlugin>,
    loaded_ids: HashSet<String>,
}

impl PluginManager {
    #[inline]
    pub fn new() -> Self {
        Self {
            loaded: Vec::new(),
            loaded_ids: HashSet::new(),
        }
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &PluginModuleDyn<'static>> {
        self.loaded.iter().map(|p| &p.module)
    }

    pub fn load_default(&mut self, host: HostApiV1) -> Result<(), PluginLoadError> {
        let dir = default_plugins_dir()?;
        self.load_from_dir(&dir, host)
    }

    pub fn load_importers_from_dir(
        &mut self,
        dir: &Path,
        host: HostApiV1,
    ) -> Result<(), PluginLoadError> {
        let dir = resolve_plugins_dir(dir)?;
        log::info!(target: "assets", "importers: scanning directory '{}'", dir.display());

        if let Err(e) = std::fs::create_dir_all(&dir) {
            return Err(PluginLoadError {
                path: dir.clone(),
                message: format!("create_dir_all failed: {e}"),
            });
        }

        let mut candidates = Vec::new();
        let rd = std::fs::read_dir(&dir).map_err(|e| PluginLoadError {
            path: dir.clone(),
            message: format!("read_dir failed: {e}"),
        })?;

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

        candidates.sort();

        log::info!(
            target: "assets",
            "importers: found {} candidate(s) in '{}'",
            candidates.len(),
            dir.display()
        );

        for path in candidates {
            match self.load_one_importer(&path, host.clone()) {
                Ok(ImporterLoadOutcome::Loaded(info)) => {
                    log::info!(
                        target: "assets",
                        "importers: loaded id='{}' ver='{}' from '{}'",
                        info.id,
                        info.version,
                        path.display()
                    );
                }
                Ok(ImporterLoadOutcome::SkippedNotImporter) => {
                    log::debug!(
                        target: "assets",
                        "importers: skipped (not an importer) '{}'",
                        path.display()
                    );
                }
                Err(e) => {
                    log::warn!(
                        target: "assets",
                        "importers: failed to load '{}': {}",
                        path.display(),
                        e
                    );
                }
            }
        }

        Ok(())
    }

    pub fn load_from_dir(&mut self, dir: &Path, host: HostApiV1) -> Result<(), PluginLoadError> {
        let dir = resolve_plugins_dir(dir)?;
        log::info!("plugins: scanning directory '{}'", dir.display());

        // Keep behavior symmetric with importer loading: create directory if missing.
        if let Err(e) = std::fs::create_dir_all(&dir) {
            return Err(PluginLoadError {
                path: dir.clone(),
                message: format!("create_dir_all failed: {e}"),
            });
        }

        let mut candidates = Vec::new();
        let rd = std::fs::read_dir(&dir).map_err(|e| PluginLoadError {
            path: dir.clone(),
            message: format!("read_dir failed: {e}"),
        })?;

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

        candidates.sort();

        log::info!(
            "plugins: found {} candidate(s) in '{}'",
            candidates.len(),
            dir.display()
        );

        for path in candidates {
            match self.load_one(&path, host.clone()) {
                Ok(()) => {}
                Err(e) => {
                    log::warn!("plugins: failed to load '{}': {}", path.display(), e);
                }
            }
        }

        Ok(())
    }

    pub fn start_all(&mut self) -> Result<(), String> {
        for p in self.loaded.iter_mut() {
            if let Err(e) = p.module.start().into_result() {
                return Err(format!("plugin '{}' start failed: {}", p.info.id, e));
            }
        }
        Ok(())
    }

    pub fn fixed_update_all(&mut self, dt: f32) -> Result<(), String> {
        for p in self.loaded.iter_mut() {
            if let Err(e) = p.module.fixed_update(dt).into_result() {
                return Err(format!("plugin '{}' fixed_update failed: {}", p.info.id, e));
            }
        }
        Ok(())
    }

    pub fn update_all(&mut self, dt: f32) -> Result<(), String> {
        for p in self.loaded.iter_mut() {
            if let Err(e) = p.module.update(dt).into_result() {
                return Err(format!("plugin '{}' update failed: {}", p.info.id, e));
            }
        }
        Ok(())
    }

    pub fn render_all(&mut self, dt: f32) -> Result<(), String> {
        for p in self.loaded.iter_mut() {
            if let Err(e) = p.module.render(dt).into_result() {
                return Err(format!("plugin '{}' render failed: {}", p.info.id, e));
            }
        }
        Ok(())
    }

    pub fn shutdown(&mut self) {
        for p in self.loaded.iter_mut().rev() {
            p.module.shutdown();
        }
        self.loaded.clear();
        self.loaded_ids.clear();
    }

    fn load_one(&mut self, path: &Path, host: HostApiV1) -> Result<(), PluginLoadError> {
        log::info!("plugins: loading '{}'", path.display());

        let lib = unsafe { Library::new(path) }.map_err(|e| PluginLoadError {
            path: path.to_path_buf(),
            message: format!("Library::new failed: {e}"),
        })?;

        let sym: libloading::Symbol<unsafe extern "C" fn() -> PluginRootV1Ref> =
            unsafe { lib.get(b"export_plugin_root\0") }.map_err(|e| PluginLoadError {
                path: path.to_path_buf(),
                message: format!("symbol export_plugin_root not found: {e}"),
            })?;

        let root = unsafe { sym() };
        let mut module = root.create()();

        let info = module.info();
        let id_str = info.id.to_string();

        // Prevent accidental double-load of the same plugin id (can cause duplicated services).
        if self.loaded_ids.contains(&id_str) {
            log::warn!(
            "plugins: duplicate id='{}' from '{}' ignored (already loaded)",
            id_str,
            path.display()
        );
            module.shutdown();
            return Ok(());
        }

        if let Err(e) = module.init(host).into_result() {
            return Err(PluginLoadError {
                path: path.to_path_buf(),
                message: format!("init failed: {e}"),
            });
        }

        log::info!(
        "plugins: loaded id='{}' ver='{}' from '{}'",
        info.id,
        info.version,
        path.display()
    );

        self.loaded_ids.insert(id_str);
        self.loaded.push(LoadedPlugin {
            _lib: lib,
            module,
            info,
        });

        Ok(())
    }

    fn load_one_importer(
        &mut self,
        path: &Path,
        host: HostApiV1,
    ) -> Result<ImporterLoadOutcome, PluginLoadError> {
        log::info!(target: "assets", "importers: loading '{}'", path.display());

        let lib = unsafe { Library::new(path) }.map_err(|e| PluginLoadError {
            path: path.to_path_buf(),
            message: format!("Library::new failed: {e}"),
        })?;

        let sym: libloading::Symbol<unsafe extern "C" fn() -> PluginRootV1Ref> =
            unsafe { lib.get(b"export_plugin_root\0") }.map_err(|e| PluginLoadError {
                path: path.to_path_buf(),
                message: format!("symbol export_plugin_root not found: {e}"),
            })?;

        let root = unsafe { sym() };
        let mut module = root.create()();

        let mut state = ImporterLoadState {
            saw_importer: false,
            staged: Vec::<ServiceV1Dyn<'static>>::new(),
        };

        let init_result = with_importer_load_state(&mut state, || module.init(host).into_result());

        if let Err(e) = init_result {
            return Err(PluginLoadError {
                path: path.to_path_buf(),
                message: format!("init failed: {e}"),
            });
        }

        if !state.saw_importer {
            module.shutdown();
            drop(module);
            drop(lib);
            return Ok(ImporterLoadOutcome::SkippedNotImporter);
        }

        for svc in state.staged.drain(..) {
            if let Err(e) = host_register_service_impl(svc, true).into_result() {
                module.shutdown();
                return Err(PluginLoadError {
                    path: path.to_path_buf(),
                    message: format!("register_service_v1 failed: {e}"),
                });
            }
        }

        let info = module.info();

        // Importers are regular plugins too, but are loaded in a separate phase. Prevent duplicates.
        let id_str = info.id.to_string();
        if self.loaded_ids.contains(&id_str) {
            log::warn!(
                target: "assets",
                "importers: duplicate id='{}' from '{}' ignored (already loaded)",
                id_str,
                path.display()
            );
            module.shutdown();
            return Ok(ImporterLoadOutcome::SkippedNotImporter);
        }

        self.loaded_ids.insert(id_str);

        self.loaded.push(LoadedPlugin {
            _lib: lib,
            module,
            info: info.clone(),
        });

        Ok(ImporterLoadOutcome::Loaded(info))
    }
}

enum ImporterLoadOutcome {
    Loaded(PluginInfo),
    SkippedNotImporter,
}
