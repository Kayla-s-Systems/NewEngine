use std::path::{Path, PathBuf};

use abi_stable::std_types::{ROption, RString};
use newengine_asset_bootstrap_runtime::try_load_window_icon_best_effort;
use newengine_core::{Engine, EngineError, EngineResult, StartupConfig};
use newengine_runtime_host::app_launcher::{
    RuntimeHostAppProfile, RuntimeHostFrontend, RuntimeHostFrontendContext, RuntimeHostLaunchSpec,
};
use newengine_ui::{UiBuildFn, UiProviderKind};

use crate::platform_runtime::{
    HostPlatformRuntime, ResolvedPlatformRuntimeConfig, detect_platform_runtime_path,
    resolve_platform_runtime_config,
};

/// UI/window-specific extension of the generic runtime-host product profile.
/// None of these types leak downward into `newengine-runtime-host`.
pub trait WindowedRuntimeHostProfile: RuntimeHostAppProfile {
    #[inline]
    fn ui_build_from_startup(&self, _startup: &StartupConfig) -> Option<Box<dyn UiBuildFn>> {
        None
    }

    #[inline]
    fn ui_provider_kind_from_startup(&self, _startup: &StartupConfig) -> UiProviderKind {
        UiProviderKind::Null
    }
}

#[derive(Clone, Debug)]
pub struct WindowedHostFrontend {
    window_title: String,
}

impl WindowedHostFrontend {
    #[inline]
    pub fn new(window_title: impl Into<String>) -> Self {
        Self {
            window_title: window_title.into(),
        }
    }

    fn detect_platform_runtime(
        &self,
        app_name: &str,
        startup: &StartupConfig,
    ) -> EngineResult<PathBuf> {
        crate::platform_early_log!(
            "platform.detect.begin app={} modules_dir={}",
            app_name,
            startup.modules_dir.display()
        );
        let path = detect_platform_runtime_path(&startup.modules_dir)?;
        crate::platform_early_log!(
            "platform.detect.ok app={} path={}",
            app_name,
            newengine_runtime_host::path_display::display_abs_path(&path)
        );
        newengine_core::crash::record_breadcrumb(format!(
            "{app_name} launcher: platform runtime detected path='{}'",
            newengine_runtime_host::path_display::display_abs_path(&path)
        ));
        Ok(path)
    }

    fn resolve_platform_runtime(
        &self,
        app_name: &str,
        startup: &StartupConfig,
        runtime_path: &Path,
    ) -> EngineResult<ResolvedPlatformRuntimeConfig> {
        crate::platform_early_log!("platform.config.resolve.begin app={app_name}");
        let resolved = resolve_platform_runtime_config(startup, runtime_path)?;
        crate::platform_early_log!(
            "platform.config.resolve.ok app={} id={} name={} version={}",
            app_name,
            resolved.plugin_id,
            resolved.plugin_name,
            resolved.plugin_version
        );
        newengine_core::crash::record_breadcrumb(format!(
            "{app_name} launcher: platform runtime resolved id='{}'",
            resolved.plugin_id
        ));
        Ok(resolved)
    }

    #[inline]
    fn headless_mode_requested(&self) -> bool {
        !platform_required() && env_bool("NEWENGINE_HEADLESS", false)
    }

    fn platform_missing_can_fallback_to_headless(&self, err: &EngineError) -> bool {
        let headless_requested = env_bool("NEWENGINE_HEADLESS", false);
        if platform_required() {
            return false;
        }
        if newengine_plugin_host::current_host_context()
            .environment_var_os("NEWENGINE_PLATFORM_RUNTIME")
            .is_some()
            && !headless_requested
        {
            return false;
        }
        if headless_requested {
            return true;
        }
        let text = err.to_string();
        [
            "platform runtime DLL not found",
            "No platform runtime",
            "platform runtime unavailable",
            "platform runtime metadata load failed",
            "platform config defaults failed",
            "platform config apply failed",
            "platform config decode failed",
            "platform runtime symbol missing",
            "platform runtime load failed",
        ]
        .iter()
        .any(|needle| text.contains(needle))
    }
}

impl<P> RuntimeHostFrontend<P> for WindowedHostFrontend
where
    P: WindowedRuntimeHostProfile,
{
    fn prepare_startup(&self, _profile: &P, spec: &RuntimeHostLaunchSpec) -> EngineResult<()> {
        #[cfg(feature = "prestart-window-egui")]
        {
            let first_install = newengine_startup_window_egui::install();
            crate::platform_early_log!(
                "startup.presenter.egui app={} registered={} first_install={}",
                spec.app_name,
                newengine_core::startup_window::startup_window_presenter_registered(),
                first_install,
            );
        }
        #[cfg(not(feature = "prestart-window-egui"))]
        let _ = spec;
        Ok(())
    }

    fn launch(
        &self,
        profile: &P,
        engine: Engine<()>,
        context: RuntimeHostFrontendContext<'_>,
    ) -> EngineResult<()> {
        let app_name = context.launch_spec.app_name;
        if self.headless_mode_requested() {
            newengine_ulog_api::ulog::warn!(
                "{} launcher: headless mode requested; windowed frontend delegates to generic CLI host",
                app_name
            );
            return newengine_runtime_host::HeadlessCliRuntime::new(
                engine,
                context.launch_spec.fixed_dt_ms,
            )
            .run("NEWENGINE_HEADLESS requested");
        }

        let runtime_path = match self.detect_platform_runtime(app_name, context.startup) {
            Ok(path) => path,
            Err(err) if self.platform_missing_can_fallback_to_headless(&err) => {
                newengine_ulog_api::ulog::warn!(
                    "{} launcher: platform runtime unavailable; falling back to generic headless frontend: {}",
                    app_name,
                    err
                );
                return newengine_runtime_host::HeadlessCliRuntime::new(
                    engine,
                    context.launch_spec.fixed_dt_ms,
                )
                .run(err.to_string());
            }
            Err(err) => return Err(err),
        };
        let mut resolved_platform = match self.resolve_platform_runtime(
            app_name,
            context.startup,
            &runtime_path,
        ) {
            Ok(resolved) => resolved,
            Err(err) if self.platform_missing_can_fallback_to_headless(&err) => {
                newengine_ulog_api::ulog::warn!(
                    "{} launcher: platform runtime could not be resolved; falling back to generic headless frontend: {}",
                    app_name,
                    err
                );
                return newengine_runtime_host::HeadlessCliRuntime::new(
                    engine,
                    context.launch_spec.fixed_dt_ms,
                )
                .run(err.to_string());
            }
            Err(err) => return Err(err),
        };

        newengine_ulog_api::ulog::info!(
            "{} launcher: windowed host runtime plugin id='{}' path='{}'",
            app_name,
            resolved_platform.plugin_id,
            newengine_runtime_host::path_display::display_abs_path(&runtime_path)
        );

        let icon = try_load_window_icon_best_effort(
            resolved_platform.icon_path.as_deref(),
            context.assets_available.then_some(context.assets),
            context.asset_roots,
        );
        let mut platform_cfg = resolved_platform.config.clone();
        platform_cfg.title = RString::from(self.window_title.as_str());
        platform_cfg.icon = icon.map_or(ROption::RNone, ROption::RSome);
        resolved_platform.config = platform_cfg;

        let ui_build = profile.ui_build_from_startup(context.startup);
        let ui_kind = profile.ui_provider_kind_from_startup(context.startup);
        crate::platform_early_log!("windowed_host.new.begin app={app_name}");
        let runtime = HostPlatformRuntime::new(engine, ui_kind, ui_build);
        crate::platform_early_log!("windowed_host.new.ok app={app_name}");

        newengine_core::crash::record_breadcrumb(format!(
            "{app_name} launcher: entering windowed host runtime"
        ));
        crate::platform_early_log!(
            "windowed_host.run.begin app={} path={} id={}",
            app_name,
            newengine_runtime_host::path_display::display_abs_path(&runtime_path),
            resolved_platform.plugin_id
        );
        runtime.run(&runtime_path, &resolved_platform)?;
        crate::platform_early_log!("windowed_host.run.returned app={app_name}");
        newengine_ulog_api::ulog::info!("{} stopped", app_name);
        Ok(())
    }
}

#[inline]
fn platform_required() -> bool {
    env_bool("NEWENGINE_REQUIRE_PLATFORM", false)
        || env_bool("NEWENGINE_REQUIRE_PLATFORM_BACKEND", false)
}

#[inline]
fn env_bool(name: &str, default: bool) -> bool {
    newengine_plugin_host::current_host_context()
        .environment_var(name)
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}
