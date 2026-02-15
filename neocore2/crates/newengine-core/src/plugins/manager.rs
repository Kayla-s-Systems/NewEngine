#![forbid(unsafe_op_in_unsafe_fn)]

use libloading::Library;
use newengine_plugin_api::{
    CapabilityDesc, HostApiV1, PluginDescriptor, PluginInfo, PluginKind, PluginModuleDyn,
    PluginModuleV2Dyn, PluginRootV1Ref,
};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

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

#[derive(Clone, Debug)]
pub struct PluginSnapshotEntry {
    pub path: PathBuf,
    pub id: String,
    pub name: String,
    pub version: String,
    pub kind: Option<PluginKind>,
    pub capabilities: Vec<CapabilityDesc>,
    pub state: String,
    pub disabled_reason: Option<String>,
}

enum PluginModuleAny {
    V1(PluginModuleDyn<'static>),
    V2(PluginModuleV2Dyn<'static>),
}

struct LoadedPlugin {
    path: PathBuf,
    _lib: Library,
    module: PluginModuleAny,
    info: PluginInfo,
    descriptor: Option<PluginDescriptor>,
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
        self.loaded.iter().filter_map(|p| match &p.module {
            PluginModuleAny::V1(m) => Some(m),
            PluginModuleAny::V2(_) => None,
        })
    }

    #[inline]
    pub fn snapshot(&self) -> Vec<PluginSnapshotEntry> {
        let mut out = Vec::with_capacity(self.loaded.len());
        for p in self.loaded.iter() {
            let (kind, caps) = match &p.descriptor {
                Some(d) => (Some(d.kind), d.capabilities.iter().cloned().collect()),
                None => (None, Vec::new()),
            };

            out.push(PluginSnapshotEntry {
                path: p.path.clone(),
                id: p.info.id.to_string(),
                name: p.info.name.to_string(),
                version: p.info.version.to_string(),
                kind,
                capabilities: caps,
                state: match p.state {
                    PluginState::Registered => "registered".to_string(),
                    PluginState::Running => "running".to_string(),
                    PluginState::Stopped => "stopped".to_string(),
                    PluginState::Disabled => "disabled".to_string(),
                },
                disabled_reason: p.disabled_reason.clone(),
            });
        }

        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    pub fn load_default(&mut self, host: HostApiV1) -> Result<(), PluginLoadError> {
        let dir = default_plugins_dir()?;
        self.load_from_dir(&dir, host)
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
            self.call_plugin(i, "start", |m| match m {
                PluginModuleAny::V1(m) => Self::rresult_to_string(m.start()),
                PluginModuleAny::V2(m) => Self::rresult_to_string(m.start()),
            });
        }
        Ok(())
    }

    pub fn fixed_update_all(&mut self, dt: f32) -> Result<(), String> {
        for i in 0..self.loaded.len() {
            if self.loaded[i].state != PluginState::Running {
                continue;
            }
            self.call_plugin(i, "fixed_update", |m| match m {
                PluginModuleAny::V1(m) => Self::rresult_to_string(m.fixed_update(dt)),
                PluginModuleAny::V2(m) => Self::rresult_to_string(m.fixed_update(dt)),
            });
        }
        Ok(())
    }

    pub fn update_all(&mut self, dt: f32) -> Result<(), String> {
        for i in 0..self.loaded.len() {
            if self.loaded[i].state != PluginState::Running {
                continue;
            }
            self.call_plugin(i, "update", |m| match m {
                PluginModuleAny::V1(m) => Self::rresult_to_string(m.update(dt)),
                PluginModuleAny::V2(m) => Self::rresult_to_string(m.update(dt)),
            });
        }
        Ok(())
    }

    pub fn render_all(&mut self, dt: f32) -> Result<(), String> {
        for i in 0..self.loaded.len() {
            if self.loaded[i].state != PluginState::Running {
                continue;
            }
            self.call_plugin(i, "render", |m| match m {
                PluginModuleAny::V1(m) => Self::rresult_to_string(m.render(dt)),
                PluginModuleAny::V2(m) => Self::rresult_to_string(m.render(dt)),
            });
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
        f: impl FnOnce(&mut PluginModuleAny) -> Result<(), String>,
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
            with_current_plugin_id(&id, || match &mut self.loaded[idx].module {
                PluginModuleAny::V1(m) => m.shutdown(),
                PluginModuleAny::V2(m) => m.shutdown(),
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

        // Prefer V2 when available: it provides kind + capabilities for tooling.
        // Older plugins will not have this field; treat it as optional.
        let (mut module_any, info, descriptor) = if let Some(create_v2) = root.create_v2() {
            let m2 = create_v2();
            let d = m2.descriptor();
            let info = PluginInfo {
                id: d.id.clone(),
                name: d.name.clone(),
                version: d.version.clone(),
            };
            (PluginModuleAny::V2(m2), info, Some(d))
        } else {
            let m1 = root.create()();
            let info = m1.info();
            (PluginModuleAny::V1(m1), info, None)
        };
        let id_str = info.id.to_string();

        if id_str.trim().is_empty() {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match &mut module_any {
                PluginModuleAny::V1(m) => m.shutdown(),
                PluginModuleAny::V2(m) => m.shutdown(),
            }));
            return Err(PluginLoadError {
                path: path.to_path_buf(),
                message: "plugin id is empty".to_string(),
            });
        }

        if info.name.to_string().trim().is_empty() {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match &mut module_any {
                PluginModuleAny::V1(m) => m.shutdown(),
                PluginModuleAny::V2(m) => m.shutdown(),
            }));
            return Err(PluginLoadError {
                path: path.to_path_buf(),
                message: "plugin name is empty".to_string(),
            });
        }

        if info.version.to_string().trim().is_empty() {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match &mut module_any {
                PluginModuleAny::V1(m) => m.shutdown(),
                PluginModuleAny::V2(m) => m.shutdown(),
            }));
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
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match &mut module_any {
                PluginModuleAny::V1(m) => m.shutdown(),
                PluginModuleAny::V2(m) => m.shutdown(),
            }));
            return Ok(());
        }

        let init_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_current_plugin_id(&id_str, || match &mut module_any {
                PluginModuleAny::V1(m) => m.init(host).into_result(),
                PluginModuleAny::V2(m) => m.init(host).into_result(),
            })
        }));

        match init_res {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                unregister_by_owner(&id_str);
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    with_current_plugin_id(&id_str, || match &mut module_any {
                        PluginModuleAny::V1(m) => m.shutdown(),
                        PluginModuleAny::V2(m) => m.shutdown(),
                    });
                }));
                return Err(PluginLoadError {
                    path: path.to_path_buf(),
                    message: format!("init failed: {e}"),
                });
            }
            Err(_) => {
                unregister_by_owner(&id_str);
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    with_current_plugin_id(&id_str, || match &mut module_any {
                        PluginModuleAny::V1(m) => m.shutdown(),
                        PluginModuleAny::V2(m) => m.shutdown(),
                    });
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
            path: path.to_path_buf(),
            _lib: lib,
            module: module_any,
            info,
            descriptor,
            state: PluginState::Registered,
            disabled_reason: None,
        });

        Ok(())
    }
}