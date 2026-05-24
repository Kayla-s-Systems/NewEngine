#![forbid(unsafe_op_in_unsafe_fn)]

//! Shared descriptor builder for self-declared NEF8/ListFile format crates.
//!
//! This crate is intentionally not a central table of known formats. It only
//! owns the repeated mechanical shape of a NEF8/ListFile descriptor. Each
//! format crate remains the authority for its own extension, content kind,
//! semantic gateway, selector syntax and consumer domains.

use newengine_assets_api::{codec_type, method, AssetFileTypeDescriptor};

pub const NEF8_MAGIC_HEX: &str = "4e454638";
pub const DOMAIN_MANIFEST_OUTPUT: &str = "domain.manifest_json";
pub const ASSET_BLOB_OUTPUT: &str = "asset.blob";

#[derive(Clone, Debug)]
pub struct ListFileFormatDescriptorBuilder {
    extension: &'static str,
    asset_kind: &'static str,
    content_kind: u32,
    semantic_gateway: &'static str,
    purpose: &'static str,
    handler_service: Option<&'static str>,
    selector_syntax: Option<&'static str>,
    consumer_domains: Vec<&'static str>,
    outputs: Vec<&'static str>,
    priority: i32,
}

impl ListFileFormatDescriptorBuilder {
    #[must_use]
    pub fn new(
        extension: &'static str,
        content_kind: u32,
        asset_kind: &'static str,
        semantic_gateway: &'static str,
        purpose: &'static str,
    ) -> Self {
        Self {
            extension,
            asset_kind,
            content_kind,
            semantic_gateway,
            purpose,
            handler_service: None,
            selector_syntax: None,
            consumer_domains: Vec::new(),
            outputs: vec![
                newengine_assets_api::ASSET_LIST_FILE_MANIFEST_OUTPUT,
                newengine_assets_api::ASSET_LIST_FILE_HEADER_OUTPUT,
                newengine_assets_api::ASSET_LIST_FILE_BODY_OUTPUT,
                DOMAIN_MANIFEST_OUTPUT,
                ASSET_BLOB_OUTPUT,
            ],
            priority: 0,
        }
    }

    #[must_use]
    pub fn handler_service(mut self, handler_service: &'static str) -> Self {
        self.handler_service = Some(handler_service);
        self
    }

    #[must_use]
    pub fn selector_syntax(mut self, selector_syntax: &'static str) -> Self {
        self.selector_syntax = Some(selector_syntax);
        self
    }

    #[must_use]
    pub fn consumer_domains(mut self, consumer_domains: &'static [&'static str]) -> Self {
        self.consumer_domains = consumer_domains.to_vec();
        self
    }

    #[must_use]
    pub fn outputs(mut self, outputs: &'static [&'static str]) -> Self {
        self.outputs = outputs.to_vec();
        self
    }

    #[must_use]
    pub fn priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    #[must_use]
    pub fn build(self, declaring_crate: &'static str) -> AssetFileTypeDescriptor {
        let extension = AssetFileTypeDescriptor::extension_key(self.extension);
        let handler_service = self
            .handler_service
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| format!("asset.codec.listfile.{extension}"));
        let selector_syntax = self
            .selector_syntax
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| format!("file.{extension}@entry"));
        let consumer_domains = if self.consumer_domains.is_empty() {
            vec![self.semantic_gateway.to_owned()]
        } else {
            self.consumer_domains.into_iter().map(ToOwned::to_owned).collect()
        };

        let mut descriptor = AssetFileTypeDescriptor {
            extension: extension.clone(),
            asset_kind: self.asset_kind.to_owned(),
            container: format!("newengine.listfile.nef8.{extension}"),
            content_kind: Some(self.content_kind),
            codec_type: codec_type::LIST_FILE.to_owned(),
            byte_owner: newengine_assets_api::ENGINE_ASSET_SERVICE_ID.to_owned(),
            semantic_gateway: self.semantic_gateway.to_owned(),
            handler_service,
            read_method: method::DECODE_V1.to_owned(),
            selector_syntax: Some(selector_syntax),
            consumer_domains,
            magic: Some(NEF8_MAGIC_HEX.to_owned()),
            outputs: self.outputs.into_iter().map(ToOwned::to_owned).collect(),
            priority: self.priority,
            vfs_backed: true,
            runtime_ready: true,
            allow_nested_assets: false,
            native_container: true,
            requires_magic: true,
            notes: format!(
                "Self-declared NEF8/ListFile descriptor from {declaring_crate} crate: {}",
                self.purpose
            ),
            ..Default::default()
        };
        descriptor.normalize_layer_contract();
        descriptor
    }
}

#[must_use]
pub fn list_file_descriptor(
    declaring_crate: &'static str,
    extension: &'static str,
    content_kind: u32,
    asset_kind: &'static str,
    semantic_gateway: &'static str,
    purpose: &'static str,
    selector_syntax: &'static str,
    consumer_domains: &'static [&'static str],
) -> AssetFileTypeDescriptor {
    ListFileFormatDescriptorBuilder::new(
        extension,
        content_kind,
        asset_kind,
        semantic_gateway,
        purpose,
    )
    .selector_syntax(selector_syntax)
    .consumer_domains(consumer_domains)
    .build(declaring_crate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_emits_valid_nef8_list_file_descriptor() {
        let descriptor = list_file_descriptor(
            "test-format-crate",
            "ytd",
            newengine_assets_api::LIST_FILE_CONTENT_KIND_YTD,
            "texture_dictionary",
            newengine_assets_api::ENGINE_ASSETS_TEXTURES_SERVICE_ID,
            "Texture Dictionary",
            "file.ytd@entry",
            &[newengine_assets_api::ENGINE_ASSETS_TEXTURES_SERVICE_ID, "engine.render"],
        );

        assert_eq!(descriptor.extension, "ytd");
        assert_eq!(descriptor.asset_kind, "texture_dictionary");
        assert_eq!(descriptor.container, "newengine.listfile.nef8.ytd");
        assert_eq!(descriptor.content_kind, Some(newengine_assets_api::LIST_FILE_CONTENT_KIND_YTD));
        assert_eq!(descriptor.codec_type, codec_type::LIST_FILE);
        assert_eq!(descriptor.byte_owner, newengine_assets_api::ENGINE_ASSET_SERVICE_ID);
        assert_eq!(descriptor.semantic_gateway, newengine_assets_api::ENGINE_ASSETS_TEXTURES_SERVICE_ID);
        assert_eq!(descriptor.gateway, descriptor.semantic_gateway);
        assert_eq!(descriptor.handler_service, "asset.codec.listfile.ytd");
        assert_eq!(descriptor.selector_syntax.as_deref(), Some("file.ytd@entry"));
        assert_eq!(descriptor.magic.as_deref(), Some(NEF8_MAGIC_HEX));
        assert!(descriptor.validate_generic_rules().is_ok());
    }
}
