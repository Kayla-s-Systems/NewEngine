#![forbid(unsafe_op_in_unsafe_fn)]

//! First-party NEF8/ListFile format descriptor catalog.
//!
//! The generic registry stays empty until descriptors are registered. Each format
//! crate owns its descriptor; this catalog is profile composition only.

use newengine_assets_api::AssetFileTypeDescriptor;
use newengine_plugin_api::HostApiV1;

pub fn descriptors() -> Vec<AssetFileTypeDescriptor> {
    vec![
        newengine_asset_format_ytd::register_format(),
        newengine_asset_format_ydd::register_format(),
        newengine_asset_format_ydr::register_format(),
        newengine_asset_format_yft::register_format(),
        newengine_asset_format_ybn::register_format(),
        newengine_asset_format_ytyp::register_format(),
        newengine_asset_format_nemat::register_format(),
        newengine_asset_format_neui::register_format(),
        newengine_asset_format_ymap::register_format(),
        newengine_asset_format_ymf::register_format(),
        newengine_asset_format_ymt::register_format(),
        newengine_asset_format_ycd::register_format(),
        newengine_asset_format_yed::register_format(),
        newengine_asset_format_yfd::register_format(),
        newengine_asset_format_yld::register_format(),
        newengine_asset_format_ypdb::register_format(),
        newengine_asset_format_yvr::register_format(),
        newengine_asset_format_ywr::register_format(),
        newengine_asset_format_ysc::register_format(),
        newengine_asset_format_ybd::register_format(),
        newengine_asset_format_ytf::register_format(),
        newengine_asset_format_nepak::register_format(),
    ]
}

pub fn register_all_file_types_best_effort(host: &HostApiV1) -> usize {
    descriptors()
        .into_iter()
        .filter(|descriptor| newengine_assets::register_asset_file_type_descriptor_best_effort(host, descriptor.clone()))
        .count()
}
