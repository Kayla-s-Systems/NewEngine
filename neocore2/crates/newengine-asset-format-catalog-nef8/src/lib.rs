#![forbid(unsafe_op_in_unsafe_fn)]

//! First-party NEF8/ListFile format descriptor catalog.
//!
//! The generic registry stays empty until descriptors are registered. Each format
//! crate owns its descriptor; this catalog is profile composition only.

use newengine_assets_api::AssetFileTypeDescriptor;
use newengine_plugin_api::HostApiV1;

pub fn descriptors() -> Vec<AssetFileTypeDescriptor> {
    vec![
        newengine_asset_format_ytd::file_type_descriptor(),
        newengine_asset_format_ydd::file_type_descriptor(),
        newengine_asset_format_ydr::file_type_descriptor(),
        newengine_asset_format_yft::file_type_descriptor(),
        newengine_asset_format_ybn::file_type_descriptor(),
        newengine_asset_format_ytyp::file_type_descriptor(),
        newengine_asset_format_nemat::file_type_descriptor(),
        newengine_asset_format_neui::file_type_descriptor(),
        newengine_asset_format_ymap::file_type_descriptor(),
        newengine_asset_format_ymf::file_type_descriptor(),
        newengine_asset_format_ymt::file_type_descriptor(),
        newengine_asset_format_ycd::file_type_descriptor(),
        newengine_asset_format_yed::file_type_descriptor(),
        newengine_asset_format_yfd::file_type_descriptor(),
        newengine_asset_format_yld::file_type_descriptor(),
        newengine_asset_format_ypdb::file_type_descriptor(),
        newengine_asset_format_yvr::file_type_descriptor(),
        newengine_asset_format_ywr::file_type_descriptor(),
        newengine_asset_format_ysc::file_type_descriptor(),
        newengine_asset_format_ybd::file_type_descriptor(),
        newengine_asset_format_ytf::file_type_descriptor(),
        newengine_asset_format_nepak::file_type_descriptor(),
    ]
}

pub fn register_all_file_types_best_effort(host: &HostApiV1) -> usize {
    descriptors()
        .into_iter()
        .filter(|descriptor| newengine_assets::register_asset_file_type_descriptor_best_effort(host, descriptor.clone()))
        .count()
}
