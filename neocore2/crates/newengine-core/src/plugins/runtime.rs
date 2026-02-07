#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_plugin_api::HostApiV1;
use std::path::PathBuf;

/// Deterministic plugin loading specification.
/// The kernel never derives paths implicitly (no current_exe/current_dir).
#[derive(Clone, Debug, Default)]
pub struct PluginLoadSpec {
    pub plugins_dir: Option<PathBuf>,
    pub importers_dir: Option<PathBuf>,
}

impl PluginLoadSpec {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn with_plugins_dir(mut self, dir: Option<PathBuf>) -> Self {
        self.plugins_dir = dir;
        self
    }

    #[inline]
    pub fn with_importers_dir(mut self, dir: Option<PathBuf>) -> Self {
        self.importers_dir = dir;
        self
    }
}

/// Runtime interface for loading and ticking plugins.
/// The kernel orchestrates only this interface.
pub trait PluginRuntime: Send {
    fn load_once(&mut self, spec: &PluginLoadSpec, host: HostApiV1) -> Result<(), String>;
    fn start_all(&mut self) -> Result<(), String>;
    fn fixed_update_all(&mut self, dt: f32) -> Result<(), String>;
    fn update_all(&mut self, dt: f32) -> Result<(), String>;
    fn render_all(&mut self, dt: f32) -> Result<(), String>;
    fn shutdown(&mut self);
}

#[derive(Default)]
pub struct NullPluginRuntime {
    loaded: bool,
}

impl NullPluginRuntime {
    #[inline]
    pub fn new() -> Self {
        Self { loaded: false }
    }
}

impl PluginRuntime for NullPluginRuntime {
    #[inline]
    fn load_once(&mut self, _spec: &PluginLoadSpec, _host: HostApiV1) -> Result<(), String> {
        if self.loaded {
            return Ok(());
        }
        self.loaded = true;
        Ok(())
    }

    #[inline]
    fn start_all(&mut self) -> Result<(), String> {
        Ok(())
    }

    #[inline]
    fn fixed_update_all(&mut self, _dt: f32) -> Result<(), String> {
        Ok(())
    }

    #[inline]
    fn update_all(&mut self, _dt: f32) -> Result<(), String> {
        Ok(())
    }

    #[inline]
    fn render_all(&mut self, _dt: f32) -> Result<(), String> {
        Ok(())
    }

    #[inline]
    fn shutdown(&mut self) {}
}