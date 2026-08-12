#![forbid(unsafe_op_in_unsafe_fn)]

//! Runtime material gateway and strict `.nemat@entry -> .ytd@entry` material resolution.
//!
//! Source image names are importer inputs, never runtime texture references.

mod adapter;
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

pub(crate) use nemat::{
    collect_texture_refs, decode_material_entry_payload, material_cache_key,
    material_response_from_authored, normalize_material_logical_path,
    preview_material_name_from_body, split_nemat_selector,
};

#[cfg(test)]
mod tests;
