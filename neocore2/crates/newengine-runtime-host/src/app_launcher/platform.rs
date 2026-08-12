use std::path::{Path, PathBuf};

use abi_stable::std_types::{ROption, RString};
use newengine_assets::AssetServiceClient;
use newengine_core::{Engine, EngineError, EngineResult, StartupConfig};

use crate::{
    asset_bootstrap::try_load_window_icon_best_effort,
    headless_cli::HeadlessCliRuntime,
    path_display::display_abs_path,
    platform_runtime::{
        detect_platform_runtime_path, resolve_platform_runtime_config, HostPlatformRuntime,
        ResolvedPlatformRuntimeConfig,
    },
};

use super::env_bool;
use super::types::{RuntimeHostAppProfile, RuntimeHostLauncher};

impl<P> RuntimeHostLauncher<P>
where
    P: RuntimeHostAppProfile,
{
    pub(super) fn launch_runtime(
        &self,
        engine: Engine<()>,
        startup: &StartupConfig,
        assets_available: bool,
        assets: &AssetServiceClient,
        asset_roots: &[PathBuf],
    ) -> EngineResult<()> {
        if self.headless_mode_requested() {
            newengine_ulog_api::ulog::warn!(
                "{} launcher: headless mode requested; skipping platform runtime discovery and entering CLI host",
                self.spec.app_name
            );
            newengine_core::crash::record_breadcrumb(format!(
                "{} launcher: explicit headless mode requested",
                self.spec.app_name
            ));
            return HeadlessCliRuntime::new(engine, self.spec.fixed_dt_ms)
                .run("NEWENGINE_HEADLESS requested");
        }

        let runtime_path = match self.detect_platform_runtime(startup) {
            Ok(path) => path,
            Err(err) if self.platform_missing_can_fallback_to_headless(&err) => {
                newengine_ulog_api::ulog::warn!(
                    "{} launcher: platform runtime unavailable; falling back to headless CLI mode: {}",
                    self.spec.app_name,
                    err
                );
                newengine_core::crash::record_breadcrumb(format!(
                    "{} launcher: platform unavailable; entering headless CLI fallback",
                    self.spec.app_name
                ));
                return HeadlessCliRuntime::new(engine, self.spec.fixed_dt_ms).run(err.to_string());
            }
            Err(err) => return Err(err),
        };
        let mut resolved_platform = match self.resolve_platform_runtime(startup, &runtime_path) {
            Ok(resolved) => resolved,
            Err(err) if self.platform_missing_can_fallback_to_headless(&err) => {
                newengine_ulog_api::ulog::warn!(
                    "{} launcher: platform runtime could not be resolved; falling back to headless CLI mode: {}",
                    self.spec.app_name,
                    err
                );
                newengine_core::crash::record_breadcrumb(format!(
                    "{} launcher: platform metadata/ABI unavailable; entering headless CLI fallback",
                    self.spec.app_name
                ));
                return HeadlessCliRuntime::new(engine, self.spec.fixed_dt_ms).run(err.to_string());
            }
            Err(err) => return Err(err),
        };

        newengine_ulog_api::ulog::info!(
            "{} launcher: platform runtime plugin id='{}' path='{}'",
            self.spec.app_name,
            resolved_platform.plugin_id,
            display_abs_path(&runtime_path)
        );

        let icon = try_load_window_icon_best_effort(
            resolved_platform.icon_path.as_deref(),
            assets_available.then_some(assets),
            asset_roots,
        );
        let mut platform_cfg = resolved_platform.config.clone();
        platform_cfg.title = RString::from(self.spec.window_title);
        platform_cfg.icon = icon.map_or(ROption::RNone, ROption::RSome);
        resolved_platform.config = platform_cfg;

        let ui_build = self.profile.ui_build_from_startup(startup);
        let ui_kind = self.profile.ui_provider_kind_from_startup(startup);
        self.early_log(format_args!("host_runtime.new.begin"));
        let runtime = HostPlatformRuntime::new(engine, ui_kind, ui_build);
        self.early_log(format_args!("host_runtime.new.ok"));

        newengine_core::crash::record_breadcrumb(format!(
            "{} launcher: entering host platform runtime",
            self.spec.app_name
        ));
        self.early_log(format_args!(
            "runtime.run.begin path={} id={}",
            display_abs_path(&runtime_path),
            resolved_platform.plugin_id
        ));
        runtime.run(&runtime_path, &resolved_platform)?;
        self.early_log(format_args!("runtime.run.returned"));
        newengine_ulog_api::ulog::info!("{} stopped", self.spec.app_name);
        Ok(())
    }

    fn detect_platform_runtime(&self, startup: &StartupConfig) -> EngineResult<PathBuf> {
        self.early_log(format_args!(
            "platform.detect.begin modules_dir={}",
            startup.modules_dir.display()
        ));
        let path = detect_platform_runtime_path(&startup.modules_dir)?;
        self.early_log(format_args!(
            "platform.detect.ok path={}",
            display_abs_path(&path)
        ));
        newengine_core::crash::record_breadcrumb(format!(
            "{} launcher: platform runtime detected path='{}'",
            self.spec.app_name,
            display_abs_path(&path)
        ));
        Ok(path)
    }

    fn resolve_platform_runtime(
        &self,
        startup: &StartupConfig,
        runtime_path: &Path,
    ) -> EngineResult<ResolvedPlatformRuntimeConfig> {
        self.early_log(format_args!("platform.config.resolve.begin"));
        let resolved = resolve_platform_runtime_config(startup, runtime_path)?;
        self.early_log(format_args!(
            "platform.config.resolve.ok id={} name={} version={}",
            resolved.plugin_id, resolved.plugin_name, resolved.plugin_version
        ));
        newengine_core::crash::record_breadcrumb(format!(
            "{} launcher: platform runtime resolved id='{}'",
            self.spec.app_name, resolved.plugin_id
        ));
        Ok(resolved)
    }

    fn headless_mode_requested(&self) -> bool {
        !platform_required() && env_bool("NEWENGINE_HEADLESS", false)
    }

    fn platform_missing_can_fallback_to_headless(&self, err: &EngineError) -> bool {
        let headless_requested = env_bool("NEWENGINE_HEADLESS", false);
        if platform_required() {
            return false;
        }
        if std::env::var_os("NEWENGINE_PLATFORM_RUNTIME").is_some() && !headless_requested {
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

#[inline]
fn platform_required() -> bool {
    env_bool("NEWENGINE_REQUIRE_PLATFORM", false)
        || env_bool("NEWENGINE_REQUIRE_PLATFORM_BACKEND", false)
}
