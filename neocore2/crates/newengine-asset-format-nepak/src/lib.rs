#![forbid(unsafe_op_in_unsafe_fn)]

//! Provider-owned file type descriptor for `.nepak`.

use newengine_assets_api::{codec_type, method, AssetFileTypeDescriptor};

pub const EXTENSION: &str = "nepak";
pub const ASSET_KIND: &str = "asset_package";
pub const PURPOSE: &str = "NewEngine Asset Package";
pub const SEMANTIC_GATEWAY: &str = newengine_assets_api::ENGINE_ASSET_SERVICE_ID;
pub const HANDLER_SERVICE: &str = "asset.codec.nepak";

pub fn file_type_descriptor() -> AssetFileTypeDescriptor {
    let mut descriptor = AssetFileTypeDescriptor {
        extension: EXTENSION.to_owned(),
        asset_kind: ASSET_KIND.to_owned(),
        container: "newengine.asset_package.v1".to_owned(),
        codec_type: codec_type::CONTAINER.to_owned(),
        byte_owner: newengine_assets_api::ENGINE_ASSET_SERVICE_ID.to_owned(),
        semantic_gateway: SEMANTIC_GATEWAY.to_owned(),
        handler_service: HANDLER_SERVICE.to_owned(),
        read_method: method::DECODE_V1.to_owned(),
        consumer_domains: vec![newengine_assets_api::ENGINE_ASSET_SERVICE_ID.to_owned()],
        magic: Some("4e4550414b010000".to_owned()),
        outputs: vec!["container.vfs_layer".to_owned(), "asset.blob".to_owned()],
        priority: 0,
        vfs_backed: true,
        runtime_ready: true,
        allow_nested_assets: true,
        native_container: true,
        requires_magic: true,
        notes: format!("Self-declared package descriptor from {} crate: {}", env!("CARGO_PKG_NAME"), PURPOSE),
        ..Default::default()
    };
    descriptor.normalize_layer_contract();
    descriptor
}
