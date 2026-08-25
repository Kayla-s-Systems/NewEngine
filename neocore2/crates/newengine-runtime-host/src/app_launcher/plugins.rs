use newengine_core::{Engine, EngineResult, StartupConfig};

use super::boot_options::{boot_option_enabled, RuntimeHostBootOption};
use super::types::{RuntimeHostAppProfile, RuntimeHostLauncher};

impl<P> RuntimeHostLauncher<P>
where
    P: RuntimeHostAppProfile,
{
    pub(super) fn initialize_profile_and_plugins(
        &self,
        engine: &mut Engine<()>,
        startup: &StartupConfig,
        boot_options: Option<&'static [RuntimeHostBootOption]>,
    ) -> EngineResult<()> {
        if let Some(composition) = self.profile.composition_spec() {
            super::runtime_units::materialize_declared_runtime_units(engine, startup, composition)?;
        }
        self.profile.register_modules(engine, startup)?;
        if boot_option_enabled(boot_options, RuntimeHostBootOption::RuntimePlugins) {
            engine.preload_bootstrap_plugins()?;
        }
        self.profile.register_engine_provider_routes_best_effort();
        self.profile.bootstrap_content_best_effort();
        newengine_core::crash::record_breadcrumb(format!(
            "{} launcher: profile registered and bootstrap plugin phase evaluated",
            self.spec.app_name
        ));
        Ok(())
    }
}
