use newengine_core::{Engine, EngineError, EngineResult, StartupConfig};

use super::boot_options::{boot_option_enabled, RuntimeHostBootOption};
use super::types::{RuntimeHostAppProfile, RuntimeHostLauncher};

const ASSET_TYPES_HOST_OWNER: &str = "newengine-assets.file-type-registry";
const ASSET_TYPES_HOST_ROUTE: &str = "engine.assets.host.types";

/// Fundamental host bootstrap service required by StarVault format discovery.
///
/// The registry must exist before any first-party plugin `init()` callback can run. StarVault owns
/// descriptor discovery/publication only and must never publish `asset.types.api` itself.
pub(super) fn ensure_host_asset_types_registry() -> EngineResult<()> {
    if let Some(route) = newengine_plugin_host::active_engine_gateway_route(
        newengine_assets_api::ENGINE_ASSET_TYPES_SERVICE_ID,
    ) {
        let valid = route.provider_service_id == newengine_assets_api::ASSET_TYPES_SERVICE_ID
            && route.provider_owner_id == ASSET_TYPES_HOST_OWNER
            && route.provider_route_id.as_deref() == Some(ASSET_TYPES_HOST_ROUTE)
            && route.origin == "engine-runtime";
        if valid {
            return Ok(());
        }
        return Err(EngineError::Other(format!(
            "host bootstrap asset-types registry has unexpected owner/route: service='{}' owner='{}' route='{}' origin='{}'",
            route.provider_service_id,
            route.provider_owner_id,
            route.provider_route_id.as_deref().unwrap_or("<none>"),
            route.origin,
        )));
    }

    if !newengine_assets::register_asset_types_gateway_best_effort() {
        return Err(EngineError::Other(
            "host bootstrap failed to register asset.types.api before plugin initialization"
                .to_owned(),
        ));
    }

    let route = newengine_plugin_host::active_engine_gateway_route(
        newengine_assets_api::ENGINE_ASSET_TYPES_SERVICE_ID,
    )
    .ok_or_else(|| {
        EngineError::Other(
            "host bootstrap registered asset.types.api but engine.assets.types route is absent"
                .to_owned(),
        )
    })?;
    if route.provider_service_id != newengine_assets_api::ASSET_TYPES_SERVICE_ID
        || route.provider_owner_id != ASSET_TYPES_HOST_OWNER
        || route.provider_route_id.as_deref() != Some(ASSET_TYPES_HOST_ROUTE)
        || route.origin != "engine-runtime"
    {
        return Err(EngineError::Other(format!(
            "host bootstrap asset-types registry committed unexpected route: service='{}' owner='{}' route='{}' origin='{}'",
            route.provider_service_id,
            route.provider_owner_id,
            route.provider_route_id.as_deref().unwrap_or("<none>"),
            route.origin,
        )));
    }

    newengine_ulog_api::ulog::info!(
        "host bootstrap: asset type registry ready service='{}' gateway='{}' owner='<host>' provider_route='{}'",
        newengine_assets_api::ASSET_TYPES_SERVICE_ID,
        newengine_assets_api::ENGINE_ASSET_TYPES_SERVICE_ID,
        ASSET_TYPES_HOST_ROUTE,
    );
    Ok(())
}

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
        // Host-owned infrastructure must be available before runtime-unit materialization scans
        // plugin inventory and before any bootstrap/engine plugin can execute `init()`.
        ensure_host_asset_types_registry()?;

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
                self.profile.distribution_runtime_unit_registrations(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_types_registry_is_host_owned_and_idempotent_within_host_context() {
        let _host = newengine_plugin_host::create_host_context();
        ensure_host_asset_types_registry().expect("first host bootstrap registration");
        ensure_host_asset_types_registry().expect("idempotent host bootstrap registration");

        let route = newengine_plugin_host::active_engine_gateway_route(
            newengine_assets_api::ENGINE_ASSET_TYPES_SERVICE_ID,
        )
        .expect("asset types route");
        assert_eq!(
            route.provider_service_id,
            newengine_assets_api::ASSET_TYPES_SERVICE_ID
        );
        assert_eq!(route.provider_owner_id, ASSET_TYPES_HOST_OWNER);
        assert_eq!(
            route.provider_route_id.as_deref(),
            Some(ASSET_TYPES_HOST_ROUTE)
        );
        assert_eq!(route.origin, "engine-runtime");
    }
}
