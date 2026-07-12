//! Static format registry and descriptor lookup helpers.

use newengine_assets_api::AssetFileTypeDescriptor;

use super::descriptor::Nef8FormatSpec;
use super::formats::{
    neftd, neitems, nemat, nepak, neui, ybd, ybn, ycd, ydd, ydr, yed, yfd, yld, ymap, ymf, ymt,
    ypdb, ysc, ytd, ytf, ytyd, ytyp, yvr, ywr,
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
    listfile_spec!(neftd),
    listfile_spec!(yld),
    listfile_spec!(ymap),
    listfile_spec!(ymf),
    listfile_spec!(ymt),
    listfile_spec!(ypdb),
    listfile_spec!(ysc),
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
