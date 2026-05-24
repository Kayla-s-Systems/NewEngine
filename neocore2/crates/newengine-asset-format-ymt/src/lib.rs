#![forbid(unsafe_op_in_unsafe_fn)]

//! Provider-owned file type descriptor for `.ymt`.
//!
//! This crate is the authority for the `.ymt` descriptor. The generic
//! `engine.assets.file_types` registry must only collect/validate/resolve this
//! descriptor; it must not duplicate this mapping in a central extension table.

use newengine_assets_api::{codec_type, method, AssetFileTypeDescriptor};

pub const EXTENSION: &str = "ymt";
pub const ASSET_KIND: &str = "metadata";
pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_YMT;
pub const PURPOSE: &str = "Metadata Container";
pub const SEMANTIC_GATEWAY: &str = "engine.assets.definitions";
pub const HANDLER_SERVICE: &str = "asset.codec.listfile.ymt";
pub const SELECTOR_SYNTAX: &str = "file.ymt@entry";

pub fn file_type_descriptor() -> AssetFileTypeDescriptor {
    let mut descriptor = AssetFileTypeDescriptor {
        extension: EXTENSION.to_owned(),
        asset_kind: ASSET_KIND.to_owned(),
        container: format!("newengine.listfile.nef8.{}", EXTENSION),
        content_kind: Some(CONTENT_KIND),
        codec_type: codec_type::LIST_FILE.to_owned(),
        byte_owner: newengine_assets_api::ENGINE_ASSET_SERVICE_ID.to_owned(),
        semantic_gateway: SEMANTIC_GATEWAY.to_owned(),
        handler_service: HANDLER_SERVICE.to_owned(),
        read_method: method::DECODE_V1.to_owned(),
        selector_syntax: Some(SELECTOR_SYNTAX.to_owned()),
        consumer_domains: vec![
            "engine.assets.definitions".to_owned(),
            "engine.assets.graph".to_owned(),
            "engine.scene".to_owned(),
            "engine.assets.models".to_owned(),
            "engine.assets.materials".to_owned(),
            "engine.physics".to_owned(),
            "engine.ai".to_owned(),
            "engine.editor".to_owned(),
            "engine.streaming".to_owned(),
        ],
        magic: Some("4e454638".to_owned()),
        outputs: vec![
            newengine_assets_api::ASSET_LIST_FILE_MANIFEST_OUTPUT.to_owned(),
            newengine_assets_api::ASSET_LIST_FILE_HEADER_OUTPUT.to_owned(),
            newengine_assets_api::ASSET_LIST_FILE_BODY_OUTPUT.to_owned(),
            "domain.manifest_json".to_owned(),
            "asset.blob".to_owned(),
        ],
        priority: 0,
        vfs_backed: true,
        runtime_ready: true,
        allow_nested_assets: false,
        native_container: true,
        requires_magic: true,
        notes: format!("Self-declared NEF8/ListFile descriptor from {} crate: {}", env!("CARGO_PKG_NAME"), PURPOSE),
        ..Default::default()
    };
    descriptor.normalize_layer_contract();
    descriptor
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_is_self_declared_and_valid() {
        let descriptor = file_type_descriptor();
        assert_eq!(descriptor.extension, EXTENSION);
        assert_eq!(descriptor.asset_kind, ASSET_KIND);
        assert_eq!(descriptor.content_kind, Some(CONTENT_KIND));
        assert_eq!(descriptor.semantic_gateway, SEMANTIC_GATEWAY);
        assert_eq!(descriptor.handler_service, HANDLER_SERVICE);
        assert!(descriptor.validate_generic_rules().is_ok());
    }
}
