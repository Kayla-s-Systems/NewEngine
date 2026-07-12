//! Descriptor construction and NEF8/package layer policy.

use newengine_assets_api::{codec_type, method, AssetFileTypeDescriptor};

pub const NEF8_MAGIC_HEX: &str = "4e454638";
pub const DOMAIN_MANIFEST_OUTPUT: &str = "domain.manifest_json";
pub const ASSET_BLOB_OUTPUT: &str = "asset.blob";

#[derive(Clone, Copy, Debug)]
pub struct Nef8FormatSpec {
    pub extension: &'static str,
    pub asset_kind: &'static str,
    pub content_kind: Option<u32>,
    pub semantic_gateway: &'static str,
    pub handler_service: &'static str,
    pub selector_syntax: Option<&'static str>,
    pub purpose: &'static str,
    pub consumer_domains: &'static [&'static str],
}

impl Nef8FormatSpec {
    pub fn descriptor(self) -> AssetFileTypeDescriptor {
        if self.extension == "nepak" {
            return package_descriptor(self);
        }
        let extension = AssetFileTypeDescriptor::extension_key(self.extension);
        let schema_writeback = supports_schema_writeback(&extension);
        let mut descriptor = AssetFileTypeDescriptor {
            extension: extension.clone(),
            asset_kind: self.asset_kind.to_owned(),
            container: format!("newengine.listfile.nef8.{extension}"),
            content_kind: self.content_kind,
            codec_type: codec_type::LIST_FILE.to_owned(),
            byte_owner: newengine_assets_api::ENGINE_ASSET_SERVICE_ID.to_owned(),
            semantic_gateway: self.semantic_gateway.to_owned(),
            handler_service: self.handler_service.to_owned(),
            read_method: method::DECODE_V1.to_owned(),
            selector_syntax: self.selector_syntax.map(ToOwned::to_owned),
            consumer_domains: self
                .consumer_domains
                .iter()
                .map(|it| (*it).to_owned())
                .collect(),
            magic: Some(NEF8_MAGIC_HEX.to_owned()),
            outputs: vec![
                newengine_assets_api::ASSET_LIST_FILE_MANIFEST_OUTPUT.to_owned(),
                newengine_assets_api::ASSET_LIST_FILE_HEADER_OUTPUT.to_owned(),
                newengine_assets_api::ASSET_LIST_FILE_BODY_OUTPUT.to_owned(),
                DOMAIN_MANIFEST_OUTPUT.to_owned(),
                ASSET_BLOB_OUTPUT.to_owned(),
            ],
            priority: 0,
            vfs_backed: true,
            runtime_ready: true,
            preview_provider: true,
            // Generic inspect providers may expose editable field schema, but
            // write-back requires a concrete format/package writer capability.
            editable: false,
            schema_editable: schema_writeback,
            write_back_available: schema_writeback,
            writer_capability: if schema_writeback {
                newengine_assets_api::ASSETS_PACKAGE_WRITER_CAPABILITY_ID.to_owned()
            } else {
                String::new()
            },
            inspect_contract: format!("asset.inspect.{extension}.v1"),
            edit_contract: if schema_writeback {
                format!("asset.edit.{extension}.v1")
            } else {
                String::new()
            },
            allow_nested_assets: false,
            native_container: true,
            requires_magic: true,
            notes: format!("Unified NEF8 descriptor: {}", self.purpose),
            ..Default::default()
        };
        descriptor.normalize_layer_contract();
        descriptor
    }
}

#[inline]
fn supports_schema_writeback(extension: &str) -> bool {
    matches!(
        extension,
        "ymap" | "ytyp" | "ytyd" | "ydd" | "ytd" | "nemat" | "neftd"
    )
}

fn package_descriptor(spec: Nef8FormatSpec) -> AssetFileTypeDescriptor {
    let mut descriptor = AssetFileTypeDescriptor {
        extension: spec.extension.to_owned(),
        asset_kind: spec.asset_kind.to_owned(),
        container: "newengine.asset_package.v1".to_owned(),
        codec_type: codec_type::CONTAINER.to_owned(),
        byte_owner: newengine_assets_api::ENGINE_ASSET_SERVICE_ID.to_owned(),
        semantic_gateway: spec.semantic_gateway.to_owned(),
        handler_service: spec.handler_service.to_owned(),
        read_method: method::DECODE_V1.to_owned(),
        consumer_domains: spec
            .consumer_domains
            .iter()
            .map(|it| (*it).to_owned())
            .collect(),
        magic: Some("4e4550414b010000".to_owned()),
        outputs: vec![
            "container.vfs_layer".to_owned(),
            ASSET_BLOB_OUTPUT.to_owned(),
        ],
        priority: 0,
        vfs_backed: true,
        runtime_ready: true,
        preview_provider: true,
        editable: false,
        schema_editable: false,
        write_back_available: false,
        writer_capability: String::new(),
        inspect_contract: "asset.inspect.nepak.v1".to_owned(),
        edit_contract: String::new(),
        allow_nested_assets: true,
        native_container: true,
        requires_magic: true,
        notes: format!("Unified package descriptor: {}", spec.purpose),
        ..Default::default()
    };
    descriptor.normalize_layer_contract();
    descriptor
}
