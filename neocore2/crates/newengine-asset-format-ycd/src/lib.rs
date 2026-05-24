#![forbid(unsafe_op_in_unsafe_fn)]

//! Provider-owned file type descriptor for `.ycd`.
//!
//! This crate is the authority for the `.ycd` descriptor. The generic
//! `engine.assets.file_types` registry must only collect/validate/resolve this
//! descriptor; it must not duplicate this mapping in a central extension table.
//! Shared NEF8/ListFile boilerplate lives in `newengine-asset-format-common`;
//! this crate still declares the actual format identity.

use newengine_asset_format_common::ListFileFormatDescriptorBuilder;
use newengine_assets_api::AssetFileTypeDescriptor;

pub const EXTENSION: &str = "ycd";
pub const ASSET_KIND: &str = "clip_dictionary";
pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_YCD;
pub const PURPOSE: &str = "Animation Clips / Clip Dictionary";
pub const SEMANTIC_GATEWAY: &str = "engine.assets.models.skeletons";
pub const HANDLER_SERVICE: &str = "asset.codec.listfile.ycd";
pub const SELECTOR_SYNTAX: &str = "file.ycd@entry";
pub const CONSUMER_DOMAINS: &[&str] = &[
    "engine.assets.models.skeletons",
    "engine.assets.models",
    "engine.scene",
    "engine.render",
];

pub fn register_format() -> AssetFileTypeDescriptor {
    ListFileFormatDescriptorBuilder::new(
        EXTENSION,
        CONTENT_KIND,
        ASSET_KIND,
        SEMANTIC_GATEWAY,
        PURPOSE,
    )
    .handler_service(HANDLER_SERVICE)
    .selector_syntax(SELECTOR_SYNTAX)
    .consumer_domains(CONSUMER_DOMAINS)
    .build(env!("CARGO_PKG_NAME"))
}

#[inline]
pub fn file_type_descriptor() -> AssetFileTypeDescriptor {
    register_format()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_is_self_declared_and_valid() {
        let descriptor = register_format();
        assert_eq!(descriptor.extension, EXTENSION);
        assert_eq!(descriptor.asset_kind, ASSET_KIND);
        assert_eq!(descriptor.content_kind, Some(CONTENT_KIND));
        assert_eq!(descriptor.semantic_gateway, SEMANTIC_GATEWAY);
        assert_eq!(descriptor.handler_service, HANDLER_SERVICE);
        assert_eq!(descriptor.selector_syntax.as_deref(), Some(SELECTOR_SYNTAX));
        assert_eq!(descriptor.consumer_domains, CONSUMER_DOMAINS.iter().map(|it| (*it).to_owned()).collect::<Vec<_>>());
        assert!(descriptor.validate_generic_rules().is_ok());
    }
}
