use newengine_plugin_api::MethodName;

use super::AssetServiceClient;
use crate::{
    file_type_method, parse_asset_reference, AssetFileTypeDescriptor, AssetFileTypeManifest,
    AssetFileTypeProbeRequest, AssetFileTypeProbeResult, AssetReference,
    ENGINE_ASSET_TYPES_SERVICE_ID,
};

impl AssetServiceClient {
    /// Return the authoritative runtime file-type manifest populated by StarVault format modules.
    pub fn file_type_manifest_v1(&self) -> Result<AssetFileTypeManifest, String> {
        self.call_service_json_typed(
            ENGINE_ASSET_TYPES_SERVICE_ID,
            MethodName::from(file_type_method::MANIFEST_JSON_V1),
            &serde_json::Value::Null,
            "asset_types.manifest_json_v1",
        )
    }

    /// Resolve the registered descriptor for a VFS logical path.
    pub fn file_type_probe_v1(
        &self,
        logical_path: &str,
    ) -> Result<AssetFileTypeProbeResult, String> {
        self.call_service_json_typed(
            ENGINE_ASSET_TYPES_SERVICE_ID,
            MethodName::from(file_type_method::PROBE_JSON_V1),
            &AssetFileTypeProbeRequest {
                logical_path: logical_path.to_owned(),
            },
            "asset_types.probe_json_v1",
        )
    }

    /// Resolve a concrete registered format descriptor. Unknown extensions are rejected instead of
    /// being interpreted by domain code.
    pub fn resolve_file_type_v1(
        &self,
        logical_path: &str,
    ) -> Result<AssetFileTypeDescriptor, String> {
        let probe = self.file_type_probe_v1(logical_path)?;
        probe.descriptor.ok_or_else(|| {
            format!(
                "asset type is not registered for logical path '{}' extension='{}'",
                probe.logical_path, probe.extension
            )
        })
    }

    /// Parse an asset reference and resolve its type from `engine.assets.types`.
    pub fn resolve_typed_asset_reference_v1(
        &self,
        value: &str,
        require_entry: bool,
    ) -> Result<(AssetReference, AssetFileTypeDescriptor), String> {
        let reference = parse_asset_reference(value)?;
        if require_entry {
            reference.require_entry()?;
        }
        let descriptor = self.resolve_file_type_v1(&reference.logical_path)?;
        Ok((reference, descriptor))
    }

    /// Require semantic ownership rather than a hard-coded file extension.
    ///
    /// This permits a domain to accept future compatible formats without any code change. The
    /// concrete extension/gateway binding remains exclusively owned by the StarVault descriptor.
    pub fn require_semantic_asset_reference_v1(
        &self,
        value: &str,
        semantic_gateway: &str,
        require_entry: bool,
    ) -> Result<(AssetReference, AssetFileTypeDescriptor), String> {
        let (reference, descriptor) =
            self.resolve_typed_asset_reference_v1(value, require_entry)?;
        if descriptor
            .semantic_gateway
            .trim()
            .eq_ignore_ascii_case(semantic_gateway.trim())
        {
            return Ok((reference, descriptor));
        }
        Err(format!(
            "asset reference '{}' resolves to format module='{}' kind='{}' semantic_gateway='{}'; expected semantic_gateway='{}'",
            reference.canonical,
            descriptor.module_id,
            descriptor.asset_kind,
            descriptor.semantic_gateway,
            semantic_gateway
        ))
    }

    /// Require a registered asset kind without knowing or repeating its extension.
    pub fn require_asset_kind_reference_v1(
        &self,
        value: &str,
        asset_kind: &str,
        require_entry: bool,
    ) -> Result<(AssetReference, AssetFileTypeDescriptor), String> {
        let (reference, descriptor) =
            self.resolve_typed_asset_reference_v1(value, require_entry)?;
        if descriptor
            .asset_kind
            .trim()
            .eq_ignore_ascii_case(asset_kind.trim())
        {
            return Ok((reference, descriptor));
        }
        Err(format!(
            "asset reference '{}' resolves to format module='{}' kind='{}'; expected kind='{}'",
            reference.canonical, descriptor.module_id, descriptor.asset_kind, asset_kind
        ))
    }
}
