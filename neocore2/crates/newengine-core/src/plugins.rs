#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Instant;
use abi_stable::library::RootModule;
use abi_stable::std_types::RString;

use newengine_plugin_api::{HostApiV1, PluginInfo, PluginModule_TO, PluginRootV1_Ref};

use crate::sync::ShutdownToken;

pub struct LoadedPlugin {
    _path: PathBuf,
    module: PluginModule_TO<'static, abi_stable::std_types::RBox<()>>,
    info: PluginInfo,
}

impl LoadedPlugin {
    #[inline]
    pub fn info(&self) -> &PluginInfo {
        &self.info
    }
}

pub struct PluginManager {
    plugins: Vec<LoadedPlugin>,
    started: bool,
}

impl PluginManager {
    #[inline]
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
            started: false,
        }
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &LoadedPlugin> {
        self.plugins.iter()
    }

    pub fn load_default(&mut self, host: HostApiV1) -> Result<(), String> {
        let dir = modules_dir_near_exe()?;
        self.load_dir(&dir, host)
    }

    pub fn load_dir(&mut self, dir: &Path, host: HostApiV1) -> Result<(), String> {
        (host.log_info)(RString::from(format!(
            "plugins: scanning directory '{}'",
            dir.display()
        )));

        if let Err(e) = std::fs::create_dir_all(dir) {
            (host.log_error)(RString::from(format!(
                "plugins: create_dir_all('{}') failed: {e}",
                dir.display()
            )));
            return Err(format!("plugins: create_dir_all('{}') failed: {e}", dir.display()));
        }

        let mut libs: Vec<PathBuf> = Vec::new();
        let rd = std::fs::read_dir(dir)
            .map_err(|e| format!("plugins: read_dir('{}') failed: {e}", dir.display()))?;

        for ent in rd {
            let ent = ent.map_err(|e| format!("plugins: read_dir entry failed: {e}"))?;
            let p = ent.path();
            if is_dynlib(&p) {
                libs.push(p);
            }
        }

        libs.sort();

        (host.log_info)(RString::from(format!(
            "plugins: found {} candidate(s) in '{}'",
            libs.len(),
            dir.display()
        )));

        for path in libs {
            (host.log_info)(RString::from(format!(
                "plugins: loading '{}'",
                path.display()
            )));

            let root = match PluginRootV1_Ref::load_from_file(&path) {
                Ok(r) => r,
                Err(e) => {
                    (host.log_error)(RString::from(format!(
                        "plugins: load_from_file('{}') failed: {e}",
                        path.display()
                    )));
                    return Err(format!(
                        "plugins: load_from_file('{}') failed: {e}",
                        path.display()
                    ));
                }
            };

            let mut module = (root.create())();
            let info = module.info();

            if let Err(e) = module.init(host.clone()).into_result() {
                (host.log_error)(RString::from(format!(
                    "plugins: init failed for id='{}' ver='{}' file='{}': {}",
                    info.id,
                    info.version,
                    path.display(),
                    e
                )));
                return Err(format!(
                    "plugins: init failed for id='{}' ver='{}': {}",
                    info.id, info.version, e
                ));
            }

            (host.log_info)(RString::from(format!(
                "plugins: loaded id='{}' ver='{}' from '{}'",
                info.id,
                info.version,
                path.display()
            )));

            self.plugins.push(LoadedPlugin {
                _path: path,
                module,
                info,
            });
        }

        Ok(())
    }

    pub fn start_all(&mut self) -> Result<(), String> {
        if self.started {
            return Ok(());
        }

        for p in self.plugins.iter_mut() {
            p.module
                .start()
                .into_result()
                .map_err(|e| format!("plugins: start failed for id='{}': {}", p.info.id, e))?;
        }

        self.started = true;
        Ok(())
    }

    pub fn fixed_update_all(&mut self, dt: f32) -> Result<(), String> {
        for p in self.plugins.iter_mut() {
            p.module
                .fixed_update(dt)
                .into_result()
                .map_err(|e| format!("plugins: fixed_update failed for id='{}': {}", p.info.id, e))?;
        }
        Ok(())
    }

    pub fn update_all(&mut self, dt: f32) -> Result<(), String> {
        for p in self.plugins.iter_mut() {
            p.module
                .update(dt)
                .into_result()
                .map_err(|e| format!("plugins: update failed for id='{}': {}", p.info.id, e))?;
        }
        Ok(())
    }

    pub fn render_all(&mut self, dt: f32) -> Result<(), String> {
        for p in self.plugins.iter_mut() {
            p.module
                .render(dt)
                .into_result()
                .map_err(|e| format!("plugins: render failed for id='{}': {}", p.info.id, e))?;
        }
        Ok(())
    }

    pub fn shutdown(&mut self) {
        for p in self.plugins.iter_mut().rev() {
            p.module.shutdown();
        }
        self.plugins.clear();
        self.started = false;
    }
}

#[inline]
fn is_dynlib(p: &Path) -> bool {
    let Some(ext) = p.extension().and_then(|s| s.to_str()) else {
        return false;
    };
    matches!(ext.to_ascii_lowercase().as_str(), "dll" | "so" | "dylib")
}

pub fn modules_dir_near_exe() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let base = exe
        .parent()
        .ok_or_else(|| "current_exe has no parent directory".to_string())?;
    Ok(base.join("modules"))
}

pub fn default_host_api() -> HostApiV1 {
    extern "C" fn log_info(msg: RString) {
        log::info!("{}", msg);
    }
    extern "C" fn log_warn(msg: RString) {
        log::warn!("{}", msg);
    }
    extern "C" fn log_error(msg: RString) {
        log::error!("{}", msg);
    }
    extern "C" fn request_exit() {
        ShutdownToken::global_request();
    }
    extern "C" fn monotonic_time_ns() -> u64 {
        static START: OnceLock<Instant> = OnceLock::new();
        let start = *START.get_or_init(Instant::now);
        start.elapsed().as_nanos() as u64
    }

    HostApiV1 {
        log_info,
        log_warn,
        log_error,
        request_exit,
        monotonic_time_ns,
    }
}