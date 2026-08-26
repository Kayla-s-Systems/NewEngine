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
            let runtime = engine
                .resources_mut()
                .get::<newengine_project_runtime::RuntimeCompositionContext>()
                .cloned();
            let extra_runtime_unit_requirements = self
                .profile
                .runtime_unit_requirements_for_runtime(runtime.as_ref())
                .map_err(newengine_core::EngineError::Other)?;
            let report = super::runtime_units::materialize_runtime_units(
                engine,
                startup,
                composition,
                self.profile.runtime_unit_registrations(),
                &extra_runtime_unit_requirements,
                boot_option_enabled(boot_options, RuntimeHostBootOption::RuntimePlugins),
            )?;
            engine.resources_mut().insert(report);
        }
        self.profile.register_modules(engine, startup)?;
        // Host/profile-owned routes are composition inputs and must exist before
        // the authoritative provider plan is frozen.
        self.profile.register_engine_provider_routes_best_effort();
        if boot_option_enabled(boot_options, RuntimeHostBootOption::RuntimePlugins) {
            engine.preload_bootstrap_plugins()?;
        }
        self.profile.bootstrap_content_best_effort();
        newengine_core::crash::record_breadcrumb(format!(
            "{} launcher: profile registered and bootstrap plugin phase evaluated",
            self.spec.app_name
        ));
        Ok(())
    }
}
