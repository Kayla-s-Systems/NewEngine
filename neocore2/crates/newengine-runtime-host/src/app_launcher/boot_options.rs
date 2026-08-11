/// Runtime-host boot switches declared by a standalone app/profile.
///
/// A profile that does not declare boot options keeps the historical full boot:
/// plugin loading and optional startup phases remain enabled. A profile that
/// returns `Some(&[...])` owns its startup contract explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuntimeHostBootOption {
    /// Show/edit the pre-start configuration window before loading config.
    PreStartConfigWindow,
    /// Draw the staged platform/runtime bootstrap loading overlay.
    RuntimeBootstrapOverlay,
    /// Allow plugin loading/preload phases through the configured plugin dir.
    RuntimePlugins,
    /// Require an active platform/window backend.
    PlatformWindow,
    /// Require an active render backend.
    RenderBackend,
    /// Require an active `engine.ui` backend.
    UiBackend,
}

#[inline]
pub(crate) fn boot_option_enabled(
    options: Option<&'static [RuntimeHostBootOption]>,
    option: RuntimeHostBootOption,
) -> bool {
    options
        .map(|declared| declared.contains(&option))
        .unwrap_or(true)
}

#[inline]
fn backend_required(declared: bool, headless_requested: bool) -> bool {
    declared && !headless_requested
}

pub(crate) fn apply_declared_boot_options_env(
    app_name: &str,
    options: Option<&'static [RuntimeHostBootOption]>,
) {
    let Some(options) = options else {
        newengine_ulog_api::ulog::debug!(
            "{} launcher: boot options not declared; full runtime-host boot remains enabled",
            app_name
        );
        return;
    };

    let has = |option| options.contains(&option);
    let headless_requested = super::env_bool("NEWENGINE_HEADLESS", false);

    if has(RuntimeHostBootOption::PreStartConfigWindow) && !headless_requested {
        std::env::remove_var("NEWENGINE_STARTUP_WINDOW_DISABLED");
        std::env::remove_var("NEWENGINE_STARTUP_WINDOW_SKIP");
    } else {
        std::env::set_var("NEWENGINE_STARTUP_WINDOW_DISABLED", "1");
    }

    if has(RuntimeHostBootOption::RuntimeBootstrapOverlay) && !headless_requested {
        std::env::remove_var("NEWENGINE_RUNTIME_BOOTSTRAP_OVERLAY_DISABLED");
    } else {
        std::env::set_var("NEWENGINE_RUNTIME_BOOTSTRAP_OVERLAY_DISABLED", "1");
    }

    let require_platform = backend_required(
        has(RuntimeHostBootOption::PlatformWindow),
        headless_requested,
    );
    let require_render = backend_required(
        has(RuntimeHostBootOption::RenderBackend),
        headless_requested,
    );
    let require_ui = backend_required(has(RuntimeHostBootOption::UiBackend), headless_requested);

    std::env::set_var(
        "NEWENGINE_REQUIRE_PLATFORM_BACKEND",
        if require_platform { "1" } else { "0" },
    );
    std::env::set_var(
        "NEWENGINE_REQUIRE_RENDER_BACKEND",
        if require_render { "1" } else { "0" },
    );
    std::env::set_var(
        "NEWENGINE_REQUIRE_UI_BACKEND",
        if require_ui { "1" } else { "0" },
    );

    newengine_ulog_api::ulog::info!(
        "{} launcher: boot options declared headless={} pre_start_window={} bootstrap_overlay={} runtime_plugins={} platform={} render={} ui={}",
        app_name,
        headless_requested,
        has(RuntimeHostBootOption::PreStartConfigWindow) && !headless_requested,
        has(RuntimeHostBootOption::RuntimeBootstrapOverlay) && !headless_requested,
        has(RuntimeHostBootOption::RuntimePlugins),
        require_platform,
        require_render,
        require_ui,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_headless_mode_disables_visual_backend_requirements() {
        assert!(!backend_required(true, true));
        assert!(!backend_required(false, true));
    }

    #[test]
    fn normal_launch_preserves_declared_backend_requirements() {
        assert!(backend_required(true, false));
        assert!(!backend_required(false, false));
    }
}
