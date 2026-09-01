//! Static format registry and descriptor lookup helpers.

use newengine_assets_api::{AssetFileTypeDescriptor, AssetGatewayRoute};

use super::descriptor::Nef8FormatSpec;
use super::formats::{
    fxd, neftd, neitems, nemat, nepak, neui, ybd, ybn, ycd, ydd, ydr, yed, yfd, yft, yld, ymap,
    ymf, ymt, ypdb, ysc, yscd, ytd, ytf, ytyd, ytyp, yvr, ywr,
};

macro_rules! listfile_spec {
    ($format:ident) => {
        Nef8FormatSpec {
            extension: $format::EXTENSION,
            asset_kind: $format::ASSET_KIND,
            content_kind: Some($format::CONTENT_KIND),
            semantic_gateway: $format::SEMANTIC_GATEWAY,
            handler_service: $format::HANDLER_SERVICE,
            selector_syntax: Some($format::SELECTOR_SYNTAX),
            purpose: $format::PURPOSE,
            consumer_domains: $format::CONSUMER_DOMAINS,
        }
    };
}

macro_rules! module_spec {
    ($format:ident) => {
        Nef8FormatSpec {
            extension: $format::EXTENSION,
            asset_kind: $format::ASSET_KIND,
            content_kind: Some($format::CONTENT_KIND),
            semantic_gateway: $format::SEMANTIC_GATEWAY,
            handler_service: $format::HANDLER_SERVICE,
            selector_syntax: None,
            purpose: $format::PURPOSE,
            consumer_domains: $format::CONSUMER_DOMAINS,
        }
    };
}

macro_rules! package_spec {
    ($format:ident) => {
        Nef8FormatSpec {
            extension: $format::EXTENSION,
            asset_kind: $format::ASSET_KIND,
            content_kind: None,
            semantic_gateway: $format::SEMANTIC_GATEWAY,
            handler_service: $format::HANDLER_SERVICE,
            selector_syntax: None,
            purpose: $format::PURPOSE,
            consumer_domains: $format::CONSUMER_DOMAINS,
        }
    };
}

const FORMAT_SPECS: &[Nef8FormatSpec] = &[
    listfile_spec!(fxd),
    listfile_spec!(nemat),
    package_spec!(nepak),
    listfile_spec!(neitems),
    listfile_spec!(neui),
    listfile_spec!(ybd),
    listfile_spec!(ybn),
    listfile_spec!(ycd),
    listfile_spec!(ydd),
    listfile_spec!(ydr),
    listfile_spec!(yed),
    listfile_spec!(yfd),
    listfile_spec!(yft),
    listfile_spec!(neftd),
    listfile_spec!(yld),
    listfile_spec!(ymap),
    listfile_spec!(ymf),
    listfile_spec!(ymt),
    listfile_spec!(ypdb),
    module_spec!(ysc),
    listfile_spec!(yscd),
    listfile_spec!(ytd),
    listfile_spec!(ytf),
    listfile_spec!(ytyp),
    listfile_spec!(ytyd),
    listfile_spec!(yvr),
    listfile_spec!(ywr),
];

#[inline]
pub const fn specs() -> &'static [Nef8FormatSpec] {
    FORMAT_SPECS
}

pub fn descriptors() -> Vec<AssetFileTypeDescriptor> {
    FORMAT_SPECS
        .iter()
        .copied()
        .map(Nef8FormatSpec::descriptor)
        .collect()
}

pub fn descriptor_for_extension(extension: &str) -> Option<AssetFileTypeDescriptor> {
    let key = AssetFileTypeDescriptor::extension_key(extension);
    FORMAT_SPECS
        .iter()
        .copied()
        .find(|spec| spec.extension == key)
        .map(Nef8FormatSpec::descriptor)
}

pub fn spec_for_content_kind(content_kind: u32) -> Option<Nef8FormatSpec> {
    FORMAT_SPECS
        .iter()
        .copied()
        .find(|spec| spec.content_kind == Some(content_kind))
}

/// Canonical default semantic route for a synthesized ListFile entry.
///
/// Format-specific producers may emit more precise per-entry routes (for
/// example YMAP cells versus the map index), but generic writers must use this
/// projection instead of maintaining their own content-kind routing table.
pub fn default_entry_route_for_content_kind(content_kind: u32) -> Option<AssetGatewayRoute> {
    let spec = spec_for_content_kind(content_kind)?;
    let (gateway, method, semantic_owner) = match spec.extension {
        "ytd" => (
            newengine_assets_api::ENGINE_ASSETS_TEXTURES_SERVICE_ID,
            newengine_assets_api::textures_method::ENTRY_RUNTIME_V1,
            "texture_dictionary",
        ),
        "ydd" => (
            "engine.assets.models",
            "model.resolve_drawable_v1",
            "drawable_dictionary",
        ),
        "ytyp" => (
            newengine_assets_api::ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
            newengine_assets_api::definitions_method::ENTRY_JSON_V1,
            "definition",
        ),
        "nemat" => (
            "engine.assets.materials",
            "materials.load_descriptor_v1",
            "material_library",
        ),
        "ymap" => (
            newengine_assets_api::ENGINE_ASSETS_MAPS_SERVICE_ID,
            "assets.maps.index_v1",
            "map",
        ),
        "neui" => (
            newengine_assets_api::ENGINE_ASSETS_UI_SERVICE_ID,
            newengine_assets_api::assets_ui_method::DOCUMENT_V1,
            "ui_dictionary",
        ),
        "neitems" => (
            "engine.gameplay.inventory",
            "items.package_v1",
            "item_definition_dictionary",
        ),
        "yscd" => (
            "engine.audio",
            "audio.sound_cue_dictionary_v1",
            "sound_cue_dictionary",
        ),
        "fxd" => (
            "engine.render.vfx",
            "vfx.effect_dictionary_v1",
            "effect_dictionary",
        ),
        _ => (spec.semantic_gateway, "asset.decode_v1", spec.asset_kind),
    };
    Some(AssetGatewayRoute::new(gateway, method, semantic_owner))
}
