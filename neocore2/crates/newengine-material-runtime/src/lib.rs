#![forbid(unsafe_op_in_unsafe_fn)]

//! Runtime material gateway and strict `.nemat@entry -> .ytd@entry` material resolution.
//!
//! Source image names are importer inputs, never runtime texture references.

mod adapter;
pub mod authored_registration;
mod binding;
mod cache;
mod nemat;
mod service;

pub use adapter::MaterialAssetGatewayAdapter;
pub use binding::{material_binding, strict_runtime_texture_ref};
pub use service::{
    materials_gateway_service, materials_gateway_service_with_host, materials_service_info,
    register_materials_gateway_best_effort, register_materials_gateway_best_effort_with_host,
    MaterialsServiceInfo,
};

#[cfg(test)]
pub(crate) use nemat::decode_material_entry_payload;
pub use nemat::decode_nemat_material_library_from_body;
pub(crate) use nemat::{
    collect_texture_refs, material_cache_key, material_response_from_authored,
    normalize_material_logical_path, preview_material_name_from_body,
    select_material_entry_from_library, split_nemat_selector, validate_material_body_schema,
};

pub const RUNTIME_UNIT_SPEC: newengine_runtime_unit_api::EngineRuntimeUnitSpec =
    newengine_runtime_unit_api::EngineRuntimeUnitSpec::new(
        "engine.runtime.materials",
        1,
        newengine_runtime_unit_api::EngineRuntimeUnitKind::Provider,
        &[newengine_materials::MATERIALS_BACKEND_CAPABILITY_ID],
        &[newengine_assets_api::ASSET_BACKEND_CAPABILITY_ID],
        newengine_runtime_unit_api::STATIC_PROVIDER_TAGS,
    );

fn runtime_unit_factory(
    _: &mut newengine_runtime_unit_api::Engine<()>,
    _: &newengine_runtime_unit_api::StartupConfig,
) -> newengine_runtime_unit_api::EngineResult<Option<Box<dyn newengine_runtime_unit_api::Module<()>>>>
{
    let host = newengine_plugin_host::default_host_api();
    let client = newengine_assets::AssetServiceClient::new(host.clone());
    let _ = register_materials_gateway_best_effort_with_host(Some(host), client);
    Ok(None)
}

pub const RUNTIME_UNIT_REGISTRATION: newengine_runtime_unit_api::RuntimeUnitRegistration =
    newengine_runtime_unit_api::RuntimeUnitRegistration::new(
        RUNTIME_UNIT_SPEC,
        runtime_unit_factory,
    );

#[cfg(test)]
mod tests;
