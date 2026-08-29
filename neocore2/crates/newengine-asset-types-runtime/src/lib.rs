#![forbid(unsafe_op_in_unsafe_fn)]

//! First-party `asset-types` runtime provider.
//!
//! This crate owns the policy that North Star's standard distribution exposes the
//! asset-type registry together with its first-party NEF8 descriptors. The generic
//! asset service and the distribution catalog remain format-agnostic.

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
    let _ = newengine_assets::register_asset_types_gateway_best_effort();
    let host = newengine_plugin_host::default_host_api();
    let registered = newengine_asset_format_nef8::descriptors()
        .into_iter()
        .filter(|descriptor| {
            newengine_assets::register_asset_type_descriptor_best_effort(&host, descriptor.clone())
        })
        .count();
    newengine_ulog_api::ulog::info!(
        "asset-types runtime unit: registered {} first-party NEF8 descriptors",
        registered
    );
    Ok(None)
}

pub const RUNTIME_UNIT_REGISTRATION: newengine_runtime_unit_api::RuntimeUnitRegistration =
    newengine_runtime_unit_api::RuntimeUnitRegistration::new(
        RUNTIME_UNIT_SPEC,
        runtime_unit_factory,
    );
