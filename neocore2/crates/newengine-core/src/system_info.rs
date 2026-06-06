#![forbid(unsafe_op_in_unsafe_fn)]
#![allow(dead_code)]

use std::path::PathBuf;

/// Runtime system/environment information, suitable for startup logs.
#[derive(Debug, Clone)]
pub struct SystemInfo {
    pub exe: Option<PathBuf>,
    pub cwd: Option<PathBuf>,
    pub pid: u32,

    pub os: &'static str,
    pub arch: &'static str,
    pub family: &'static str,

    pub logical_cpus: Option<usize>,
}

impl SystemInfo {
    #[inline]
    pub fn collect() -> Self {
        let exe = std::env::current_exe().ok();
        let cwd = std::env::current_dir().ok();
        let pid = std::process::id();

        let logical_cpus = std::thread::available_parallelism().ok().map(|n| n.get());

        Self {
            exe,
            cwd,
            pid,
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
            family: std::env::consts::FAMILY,
            logical_cpus,
        }
    }

    #[inline]
    pub fn log(&self) {
        newengine_ulog_api::ulog::info!(
            "system: os='{}' arch='{}' family='{}' pid={}",
            self.os,
            self.arch,
            self.family,
            self.pid
        );

        if let Some(n) = self.logical_cpus {
            newengine_ulog_api::ulog::info!("system: logical_cpus={}", n);
        }

        if let Some(exe) = &self.exe {
            newengine_ulog_api::ulog::info!("system: exe='{}'", exe.display());
        }

        if let Some(cwd) = &self.cwd {
            newengine_ulog_api::ulog::info!("system: cwd='{}'", cwd.display());
        }
    }
}
