use newengine_core::{Engine, EngineResult, StartupConfig};
use newengine_ui::{UiBuildFn, UiProviderKind};

use super::boot_options::RuntimeHostBootOption;

/// Product/application declaration for a standalone runtime-host launch.
#[derive(Clone, Debug)]
pub struct RuntimeHostLaunchSpec {
    pub product_name: &'static str,
    pub app_name: &'static str,
    pub app_version: &'static str,
    pub startup_config_path: &'static str,
    pub fixed_dt_ms: u32,
    pub app_dir_name: &'static str,
    pub app_assets_env: &'static str,
    pub window_title: &'static str,
    pub early_log_file_name: &'static str,
    pub default_profile_env: Option<(&'static str, &'static str)>,
    pub env_defaults: &'static [(&'static str, &'static str)],
}

impl RuntimeHostLaunchSpec {
    #[inline]
    pub fn apply_env_defaults(&self) {
        for &(key, value) in self.env_defaults {
            if std::env::var_os(key).is_none() {
                std::env::set_var(key, value);
            }
        }
        if let Some((key, value)) = self.default_profile_env {
            if std::env::var_os(key).is_none() {
                std::env::set_var(key, value);
            }
        }
    }
}

/// Game/profile-specific hooks used by the generic runtime-host launcher.
pub trait RuntimeHostAppProfile {
    fn register_modules(
        &self,
        engine: &mut Engine<()>,
        startup: &StartupConfig,
    ) -> EngineResult<()>;

    #[inline]
    fn boot_options(&self) -> Option<&'static [RuntimeHostBootOption]> {
        None
    }

    #[inline]
    fn register_engine_provider_routes_best_effort(&self) {}

    #[inline]
    fn bootstrap_content_best_effort(&self) {}

    #[inline]
    fn ui_build_from_startup(&self, _startup: &StartupConfig) -> Option<Box<dyn UiBuildFn>> {
        None
    }

    #[inline]
    fn ui_provider_kind_from_startup(&self, startup: &StartupConfig) -> UiProviderKind {
        crate::engine_factory::ui_provider_kind_from_startup(startup)
    }
}

pub struct RuntimeHostLauncher<P> {
    pub(super) spec: RuntimeHostLaunchSpec,
    pub(super) profile: P,
}

impl<P> RuntimeHostLauncher<P>
where
    P: RuntimeHostAppProfile,
{
    #[inline]
    pub fn new(spec: RuntimeHostLaunchSpec, profile: P) -> Self {
        Self { spec, profile }
    }
}
