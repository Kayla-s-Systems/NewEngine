#![forbid(unsafe_op_in_unsafe_fn)]

use libloading::Library;
use newengine_plugin_api::{HostApiV1, PluginInfo, PluginModuleDyn, PluginRootV1Ref, ServiceV1Dyn};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::plugins::host_api::{
    host_register_service_impl, with_importer_load_state, ImporterLoadState,
};
use crate::plugins::host_context::{unregister_by_owner, with_current_plugin_id};
use crate::plugins::paths::{default_plugins_dir, is_dynamic_lib, resolve_plugins_dir};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum PluginState {
    Registered,
    Running,
    Stopped,
    Disabled,
}

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
    state: PluginState,
    disabled_reason: Option<String>,
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

    #[inline]
    fn rresult_to_string(
        r: abi_stable::std_types::RResult<(), abi_stable::std_types::RString>,
    ) -> Result<(), String> {
        r.into_result().map_err(|e| e.to_string())
    }

    pub fn start_all(&mut self) -> Result<(), String> {
        for i in 0..self.loaded.len() {
            if self.loaded[i].state != PluginState::Registered {
                continue;
            }
            self.call_plugin(i, "start", |m| Self::rresult_to_string(m.start()));
        }
        Ok(())
    }

    pub fn fixed_update_all(&mut self, dt: f32) -> Result<(), String> {
        for i in 0..self.loaded.len() {
            if self.loaded[i].state != PluginState::Running {
                continue;
            }
            self.call_plugin(i, "fixed_update", |m| {
                Self::rresult_to_string(m.fixed_update(dt))
            });
        }
        Ok(())
    }

    pub fn update_all(&mut self, dt: f32) -> Result<(), String> {
        for i in 0..self.loaded.len() {
            if self.loaded[i].state != PluginState::Running {
                continue;
            }
            self.call_plugin(i, "update", |m| Self::rresult_to_string(m.update(dt)));
        }
        Ok(())
    }

    pub fn render_all(&mut self, dt: f32) -> Result<(), String> {
        for i in 0..self.loaded.len() {
            if self.loaded[i].state != PluginState::Running {
                continue;
            }
            self.call_plugin(i, "render", |m| Self::rresult_to_string(m.render(dt)));
        }
        Ok(())
    }

    pub fn shutdown(&mut self) {
        for i in (0..self.loaded.len()).rev() {
            let id = self.loaded[i].info.id.to_string();
            self.safe_shutdown_one(i);
            self.loaded[i].state = PluginState::Stopped;
            unregister_by_owner(&id);
        }
        self.loaded.clear();
        self.loaded_ids.clear();
    }

    fn call_plugin(
        &mut self,
        idx: usize,
        op: &str,
        f: impl FnOnce(&mut PluginModuleDyn<'static>) -> Result<(), String>,
    ) {
        if idx >= self.loaded.len() {
            return;
        }

        if self.loaded[idx].state == PluginState::Disabled {
            return;
        }

        let id = self.loaded[idx].info.id.to_string();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_current_plugin_id(&id, || f(&mut self.loaded[idx].module))
        }));

        match result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                log::error!("plugins: op '{}' failed for id='{}': {}", op, id, e);
                self.disable_plugin(idx, &id, format!("op '{op}' failed: {e}"));
            }
            Err(_) => {
                log::error!(
                    "plugins: panic during op '{}' for id='{}' (plugin disabled)",
                    op,
                    id
                );
                self.disable_plugin(idx, &id, format!("panic during op '{op}'"));
            }
        }

        if idx < self.loaded.len() {
            if op == "start" && self.loaded[idx].state == PluginState::Registered {
                self.loaded[idx].state = PluginState::Running;
            }
        }
    }

    fn disable_plugin(&mut self, idx: usize, id: &str, reason: String) {
        if idx >= self.loaded.len() || self.loaded[idx].state == PluginState::Disabled {
            return;
        }

        self.loaded[idx].state = PluginState::Disabled;
        self.loaded[idx].disabled_reason = Some(reason);

        self.safe_shutdown_one(idx);
        unregister_by_owner(id);
    }

    fn safe_shutdown_one(&mut self, idx: usize) {
        if idx >= self.loaded.len() {
            return;
        }

        let id = self.loaded[idx].info.id.to_string();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_current_plugin_id(&id, || {
                self.loaded[idx].module.shutdown();
            })
        }));
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

        if id_str.trim().is_empty() {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| module.shutdown()));
            return Err(PluginLoadError {
                path: path.to_path_buf(),
                message: "plugin id is empty".to_string(),
            });
        }

        if info.name.to_string().trim().is_empty() {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| module.shutdown()));
            return Err(PluginLoadError {
                path: path.to_path_buf(),
                message: "plugin name is empty".to_string(),
            });
        }

        if info.version.to_string().trim().is_empty() {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| module.shutdown()));
            return Err(PluginLoadError {
                path: path.to_path_buf(),
                message: "plugin version is empty".to_string(),
            });
        }

        if self.loaded_ids.contains(&id_str) {
            log::warn!(
                "plugins: duplicate id='{}' from '{}' ignored (already loaded)",
                id_str,
                path.display()
            );
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| module.shutdown()));
            return Ok(());
        }

        let init_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_current_plugin_id(&id_str, || module.init(host).into_result())
        }));

        match init_res {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                unregister_by_owner(&id_str);
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    with_current_plugin_id(&id_str, || module.shutdown());
                }));
                return Err(PluginLoadError {
                    path: path.to_path_buf(),
                    message: format!("init failed: {e}"),
                });
            }
            Err(_) => {
                unregister_by_owner(&id_str);
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    with_current_plugin_id(&id_str, || module.shutdown());
                }));
                return Err(PluginLoadError {
                    path: path.to_path_buf(),
                    message: "init panicked".to_string(),
                });
            }
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
            state: PluginState::Registered,
            disabled_reason: None,
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

        let info_pre = module.info();
        let id_pre = info_pre.id.to_string();

        let mut state = ImporterLoadState {
            saw_importer: false,
            staged: Vec::<ServiceV1Dyn<'static>>::new(),
        };

        let init_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_current_plugin_id(&id_pre, || {
                with_importer_load_state(&mut state, || module.init(host).into_result())
            })
        }));

        let init_outcome: Result<(), String> = match init_result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(e.to_string()),
            Err(_) => Err("init panicked".to_string()),
        };

        if let Err(e) = init_outcome {
            unregister_by_owner(&id_pre);
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                with_current_plugin_id(&id_pre, || module.shutdown());
            }));
            return Err(PluginLoadError {
                path: path.to_path_buf(),
                message: format!("init failed: {e}"),
            });
        }

        if !state.saw_importer {
            unregister_by_owner(&id_pre);
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                with_current_plugin_id(&id_pre, || module.shutdown());
            }));
            drop(module);
            drop(lib);
            return Ok(ImporterLoadOutcome::SkippedNotImporter);
        }

        for svc in state.staged.drain(..) {
            let reg = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                with_current_plugin_id(&id_pre, || {
                    host_register_service_impl(svc, true).into_result()
                })
            }));

            match reg {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    unregister_by_owner(&id_pre);
                    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        with_current_plugin_id(&id_pre, || module.shutdown());
                    }));
                    return Err(PluginLoadError {
                        path: path.to_path_buf(),
                        message: format!("register_service_v1 failed: {e}"),
                    });
                }
                Err(_) => {
                    unregister_by_owner(&id_pre);
                    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        with_current_plugin_id(&id_pre, || module.shutdown());
                    }));
                    return Err(PluginLoadError {
                        path: path.to_path_buf(),
                        message: "register_service_v1 panicked".to_string(),
                    });
                }
            }
        }

        let info = module.info();
        let id_str = info.id.to_string();

        if self.loaded_ids.contains(&id_str) {
            log::warn!(
                target: "assets",
                "importers: duplicate id='{}' from '{}' ignored (already loaded)",
                id_str,
                path.display()
            );
            unregister_by_owner(&id_str);
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                with_current_plugin_id(&id_str, || module.shutdown());
            }));
            return Ok(ImporterLoadOutcome::SkippedNotImporter);
        }

        self.loaded_ids.insert(id_str);

        self.loaded.push(LoadedPlugin {
            _lib: lib,
            module,
            info: info.clone(),
            state: PluginState::Registered,
            disabled_reason: None,
        });

        Ok(ImporterLoadOutcome::Loaded(info))
    }
}

enum ImporterLoadOutcome {
    Loaded(PluginInfo),
    SkippedNotImporter,
}