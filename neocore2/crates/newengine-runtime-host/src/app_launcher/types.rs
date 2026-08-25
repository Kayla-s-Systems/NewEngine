use std::path::PathBuf;

use newengine_assets::AssetServiceClient;
use newengine_core::{Engine, EngineResult, StartupConfig};
use newengine_project_runtime::RuntimeCompositionContext;

use super::boot_options::RuntimeHostBootOption;

/// Generic process/control-plane launch declaration.
///
/// Window titles, UI toolkit hooks and platform runtime policy deliberately live
/// above this contract in `newengine-windowed-host-runtime`.
#[derive(Clone, Debug)]
pub struct RuntimeHostLaunchSpec {
    pub product_name: &'static str,
    pub app_name: &'static str,
    pub app_version: &'static str,
    pub startup_config_path: &'static str,
    pub fixed_dt_ms: u32,
    pub app_dir_name: &'static str,
    pub app_assets_env: &'static str,
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
    /// Pure declarative engine shape requested by this product/profile.
    /// The generic host declares the requirements and materializes matching
    /// runtime bridge units; profiles must not instantiate backend adapters.
    #[inline]
    fn composition_spec(&self) -> Option<newengine_service_api::EngineCompositionSpec> {
        None
    }

    fn register_modules(
        &self,
        engine: &mut Engine<()>,
        startup: &StartupConfig,
    ) -> EngineResult<()>;

    #[inline]
    fn boot_options(&self) -> Option<&'static [RuntimeHostBootOption]> {
        None
    }

    /// Initializes services owned by the selected runtime composition.
    ///
    /// The generic host deliberately does not construct gameplay, replication,
    /// networking, rendering or other engine-domain services. Concrete profiles
    /// opt into those domains through this hook or, preferably, plugin routes.
    #[inline]
    fn initialize_composition_services(
        &self,
        _engine: &mut Engine<()>,
        _host_preinit: &crate::HostPreInitSnapshot,
        _runtime: Option<&RuntimeCompositionContext>,
    ) -> EngineResult<()> {
        Ok(())
    }

    /// Registers providers that must be selectable before Engine construction.
    ///
    /// Implementations may supply an alternative `engine.host.capabilities`
    /// route here. The runtime host installs its native provider only when no
    /// route is present, so product and platform compositions can replace
    /// hardware discovery without patching the Host.
    #[inline]
    fn register_preinit_provider_routes_best_effort(&self) {}

    #[inline]
    fn register_engine_provider_routes_best_effort(&self) {}

    #[inline]
    fn bootstrap_content_best_effort(&self) {}
}

/// Neutral handoff from process/control-plane bootstrap to a concrete runtime
/// frontend. This is the only point where a windowed host, headless host, remote
/// host or future console host takes ownership of the already-composed Engine.
pub struct RuntimeHostFrontendContext<'a> {
    pub launch_spec: &'a RuntimeHostLaunchSpec,
    pub startup: &'a StartupConfig,
    pub assets_available: bool,
    pub assets: &'a AssetServiceClient,
    pub asset_roots: &'a [PathBuf],
}

pub trait RuntimeHostFrontend<P: RuntimeHostAppProfile> {
    /// Called before startup config is loaded. Concrete frontends may install a
    /// startup presenter, but the runtime host itself remains toolkit-agnostic.
    #[inline]
    fn prepare_startup(&self, _profile: &P, _spec: &RuntimeHostLaunchSpec) -> EngineResult<()> {
        Ok(())
    }

    fn launch(
        &self,
        profile: &P,
        engine: Engine<()>,
        context: RuntimeHostFrontendContext<'_>,
    ) -> EngineResult<()>;
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

    #[inline]
    pub fn launch_spec(&self) -> &RuntimeHostLaunchSpec {
        &self.spec
    }

    #[inline]
    pub fn profile(&self) -> &P {
        &self.profile
    }
}
