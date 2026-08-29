#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::OnceLock;

use super::config::StartupConfig;
use super::config::StartupLoadReport;

static LAST_REPORT: OnceLock<StartupLoadReport> = OnceLock::new();
static LAST_CONFIG: OnceLock<StartupConfig> = OnceLock::new();

#[inline]
pub fn set_last_load_report(report: StartupLoadReport) {
    let _ = LAST_REPORT.set(report);
}

#[inline]
pub fn last_load_report() -> Option<&'static StartupLoadReport> {
    LAST_REPORT.get()
}

#[inline]
pub fn set_last_startup_config(cfg: StartupConfig) {
    let _ = LAST_CONFIG.set(cfg);
}

#[inline]
pub fn last_startup_config() -> Option<&'static StartupConfig> {
    LAST_CONFIG.get()
}
