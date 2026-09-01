#![forbid(unsafe_op_in_unsafe_fn)]

//! Provider for the generic `engine.assets.types` registry gateway.
//!
//! The registry is intentionally empty at startup. Concrete format descriptors are
//! owned by StarVault loadable format modules discovered from its relative
//! `formats/` directory. This runtime unit owns registry availability only; it
//! does not carry a built-in table of first-party extensions.

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
    let registered = newengine_assets::register_asset_types_gateway_best_effort();
    newengine_ulog_api::ulog::info!(
        "asset-types runtime unit: registry gateway available={} descriptor_source='starvault-relative-formats'",
        registered
    );
    Ok(None)
}

pub const RUNTIME_UNIT_REGISTRATION: newengine_runtime_unit_api::RuntimeUnitRegistration =
    newengine_runtime_unit_api::RuntimeUnitRegistration::new(
        RUNTIME_UNIT_SPEC,
        runtime_unit_factory,
    );
