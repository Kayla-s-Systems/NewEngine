#![forbid(unsafe_op_in_unsafe_fn)]

//! Readiness/diagnostics runtime unit for the host-owned `engine.assets.types` registry.
//!
//! The registry itself is created by RuntimeHost bootstrap before plugin initialization. Concrete
//! descriptors are then published by StarVault format modules. This runtime unit never creates,
//! replaces, seeds or owns `asset.types.api`.

pub const RUNTIME_UNIT_SPEC: newengine_runtime_unit_api::EngineRuntimeUnitSpec =
    newengine_runtime_unit_api::EngineRuntimeUnitSpec::new(
        "engine.runtime.asset-types",
        1,
        newengine_runtime_unit_api::EngineRuntimeUnitKind::Provider,
        &[newengine_assets_api::ASSET_TYPES_BACKEND_CAPABILITY_ID],
        &[newengine_assets_api::ASSET_BACKEND_CAPABILITY_ID],
        newengine_runtime_unit_api::STATIC_PROVIDER_TAGS,
    );

fn runtime_unit_factory(
    _: &mut newengine_runtime_unit_api::Engine<()>,
    _: &newengine_runtime_unit_api::StartupConfig,
) -> newengine_runtime_unit_api::EngineResult<Option<Box<dyn newengine_runtime_unit_api::Module<()>>>>
{
    let route = newengine_plugin_host::active_engine_gateway_route(
        newengine_assets_api::ENGINE_ASSET_TYPES_SERVICE_ID,
    )
    .ok_or_else(|| {
        newengine_runtime_unit_api::EngineError::Other(
            "asset-types readiness failed: host-owned engine.assets.types registry is absent"
                .to_owned(),
        )
    })?;

    if route.provider_service_id != newengine_assets_api::ASSET_TYPES_SERVICE_ID
        || route.provider_owner_id != "newengine-assets.file-type-registry"
        || route.provider_route_id.as_deref() != Some("engine.assets.host.types")
        || route.origin != "engine-runtime"
    {
        return Err(newengine_runtime_unit_api::EngineError::Other(format!(
            "asset-types readiness failed: unexpected registry route service='{}' owner='{}' route='{}' origin='{}'",
            route.provider_service_id,
            route.provider_owner_id,
            route.provider_route_id.as_deref().unwrap_or("<none>"),
            route.origin,
        )));
    }

    newengine_ulog_api::ulog::info!(
        "asset-types runtime unit: registry gateway ready owner='<host>' provider_route='engine.assets.host.types' descriptor_source='starvault-relative-formats'"
    );
    Ok(None)
}

pub const RUNTIME_UNIT_REGISTRATION: newengine_runtime_unit_api::RuntimeUnitRegistration =
    newengine_runtime_unit_api::RuntimeUnitRegistration::new(
        RUNTIME_UNIT_SPEC,
        runtime_unit_factory,
    );

#[cfg(test)]
mod tests {
    #[test]
    fn source_does_not_create_or_seed_registry_service() {
        let source = include_str!("lib.rs");
        let register_symbol = ["register_asset_types_gateway_", "best_effort"].concat();
        let seeded_symbol = ["asset_types_gateway_service_", "seeded"].concat();
        assert!(!source.contains(&register_symbol));
        assert!(!source.contains(&seeded_symbol));
    }
}
