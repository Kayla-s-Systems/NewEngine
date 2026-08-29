#![forbid(unsafe_op_in_unsafe_fn)]

use crate::startup::StartupLoggingConfig;

/// Applies resolved startup logging configuration to process environment variables.
///
/// Rationale: logging can be provided by an external runtime plugin (DLL),
/// but the engine must still enforce the *resolved* config.json overrides
/// deterministically (so defaults/env do not silently win).
///
/// The logging plugin is expected to read the NEWENGINE_LOG_* variables.
pub fn apply_startup_logging_env(cfg: &StartupLoggingConfig) {
    fn set_opt(key: &str, v: Option<&str>) {
        match v.map(str::trim).filter(|s| !s.is_empty()) {
            Some(val) => {
                newengine_plugin_host::current_host_context().set_environment_var(key, val)
            }
            None => newengine_plugin_host::current_host_context().remove_environment_var(key),
        }
    }

    fn set_bool(key: &str, v: bool) {
        newengine_plugin_host::current_host_context()
            .set_environment_var(key, if v { "true" } else { "false" });
    }

    fn set_u64_opt(key: &str, v: Option<u64>) {
        match v {
            Some(x) => newengine_plugin_host::current_host_context()
                .set_environment_var(key, x.to_string()),
            None => newengine_plugin_host::current_host_context().remove_environment_var(key),
        }
    }

    fn set_usize(key: &str, v: usize) {
        newengine_plugin_host::current_host_context().set_environment_var(key, v.to_string());
    }

    set_opt("NEWENGINE_LOG", cfg.filter.as_deref());
    newengine_plugin_host::current_host_context()
        .set_environment_var("NEWENGINE_LOG_LEVEL", cfg.level.trim());

    set_opt("NEWENGINE_LOG_STYLE", cfg.style.as_deref());
    set_bool("NEWENGINE_LOG_COLORS", cfg.colors);

    // Field selection.
    set_bool("NEWENGINE_LOG_MODULE", cfg.include_module_path);
    set_bool("NEWENGINE_LOG_TARGET_FIELD", cfg.include_target);
    set_bool("NEWENGINE_LOG_INCLUDE_FILE", cfg.include_file);
    set_bool("NEWENGINE_LOG_INCLUDE_LINE", cfg.include_line_number);

    set_opt("NEWENGINE_LOG_TIMESTAMP", cfg.timestamp.as_deref());

    match cfg.indent {
        Some(n) => newengine_plugin_host::current_host_context()
            .set_environment_var("NEWENGINE_LOG_INDENT", n.to_string()),
        None => newengine_plugin_host::current_host_context()
            .remove_environment_var("NEWENGINE_LOG_INDENT"),
    }

    set_opt("NEWENGINE_LOG_TARGET", cfg.console_target.as_deref());
    set_opt("NEWENGINE_LOG_FILE", cfg.file_path.as_deref());
    set_bool("NEWENGINE_LOG_TEE", cfg.tee);

    set_u64_opt("NEWENGINE_LOG_ROLL_MAX_BYTES", cfg.roll_max_bytes);
    set_usize("NEWENGINE_LOG_ROLL_MAX_FILES", cfg.roll_max_files);

    match cfg.roll_keep_days {
        Some(n) => newengine_plugin_host::current_host_context()
            .set_environment_var("NEWENGINE_LOG_ROLL_KEEP_DAYS", n.to_string()),
        None => newengine_plugin_host::current_host_context()
            .remove_environment_var("NEWENGINE_LOG_ROLL_KEEP_DAYS"),
    }
}
