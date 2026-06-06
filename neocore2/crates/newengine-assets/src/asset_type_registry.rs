#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::{RResult, RString};
use newengine_assets_api::{
    file_type_method, AssetFileTypeDescriptor, AssetFileTypeManifest,
    AssetFileTypeProbeRequest, AssetFileTypeProbeResult, AssetFileTypeRegisterRequest,
    ASSET_TYPES_SERVICE_METHODS, ASSET_TYPES_BACKEND_CAPABILITY_ID,
    ASSET_TYPES_SERVICE_ID, ENGINE_ASSET_TYPES_SERVICE_ID,
};
use newengine_plugin_api::{Blob, HostApiV1, MethodName};
use newengine_service_api::EngineServiceKind;
use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_empty_blob, ok_json,
    register_engine_gateway_provider_service_best_effort, EngineGatewayProviderDecl, JsonServiceRouter,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize)]
pub struct AssetTypesServiceInfo {
    pub id: &'static str,
    pub gateway: &'static str,
    pub methods: &'static [&'static str],
    pub backend: &'static str,
    pub registered_extensions: Vec<String>,
}

#[derive(Clone)]
struct AssetTypeRegistryState {
    registry: BTreeMap<String, AssetFileTypeDescriptor>,
}

impl Default for AssetTypeRegistryState {
    fn default() -> Self {
        // Empty by design: formats self-register through their own crates/providers.
        // The registry is a generic collector/resolver, not a god table of
        // extensions and semantic gateways.
        Self { registry: BTreeMap::new() }
    }
}

impl AssetTypeRegistryState {
    fn service_info(&self) -> AssetTypesServiceInfo {
        AssetTypesServiceInfo {
            id: ASSET_TYPES_SERVICE_ID,
            gateway: ENGINE_ASSET_TYPES_SERVICE_ID,
            methods: ASSET_TYPES_SERVICE_METHODS,
            backend: "engine.assets.starvault.file-type-registry",
            registered_extensions: self.registry.keys().cloned().collect(),
        }
    }

    fn manifest(&self) -> AssetFileTypeManifest {
        AssetFileTypeManifest {
            formats: self.registry.values().cloned().collect(),
            ..AssetFileTypeManifest::default()
        }
    }

    fn register(&mut self, request: AssetFileTypeRegisterRequest) -> AssetFileTypeDescriptor {
        let mut desc = request.descriptor;
        desc.normalize_layer_contract();
        if let Err(e) = desc.validate_generic_rules() {
            let mut rejected = desc.clone();
            rejected.notes = format!("descriptor rejected by generic codec rules: {e}");
            return rejected;
        }
        warn_if_semantic_gateway_unresolved(&desc);
        let key = desc.extension.clone();
        let replace = self
            .registry
            .get(&key)
            .map(|prev| desc.priority > prev.priority || (desc.priority == prev.priority && desc.handler_service < prev.handler_service))
            .unwrap_or(true);
        if replace {
            self.registry.insert(key, desc.clone());
        }
        self.registry.get(&desc.extension).cloned().unwrap_or(desc)
    }

    fn probe(&self, request: AssetFileTypeProbeRequest) -> AssetFileTypeProbeResult {
        let logical_path = normalize_logical_path(&request.logical_path);
        let extension = self.best_extension_match(&logical_path).unwrap_or_else(|| path_extension(&logical_path));
        let descriptor = self.registry.get(&extension).cloned();
        AssetFileTypeProbeResult {
            logical_path,
            extension,
            known: descriptor.is_some(),
            descriptor,
        }
    }

    fn best_extension_match(&self, logical_path: &str) -> Option<String> {
        let path = logical_path.split('@').next().unwrap_or(logical_path).to_ascii_lowercase();
        self.registry
            .keys()
            .filter(|ext| path.ends_with(&format!(".{ext}")))
            .max_by_key(|ext| ext.len())
            .cloned()
    }

    fn invoke_json(&mut self, payload: Blob) -> RResult<Blob, RString> {
        #[derive(Deserialize)]
        struct InvokeEnvelope {
            method: String,
            #[serde(default)]
            request: serde_json::Value,
        }

        let envelope = match serde_json::from_slice::<InvokeEnvelope>(payload.as_slice()) {
            Ok(envelope) => envelope,
            Err(e) => return RResult::RErr(RString::from(format!("asset.file_types: invalid invoke_json payload: {e}"))),
        };

        match envelope.method.as_str() {
            file_type_method::MANIFEST_JSON_V1 => ok_json(self.manifest()),
            file_type_method::REGISTER_JSON_V1 => {
                let request = match serde_json::from_value::<AssetFileTypeRegisterRequest>(envelope.request) {
                    Ok(request) => request,
                    Err(e) => return RResult::RErr(RString::from(format!("asset.file_types: invalid register request: {e}"))),
                };
                ok_json(self.register(request))
            }
            file_type_method::PROBE_JSON_V1 | file_type_method::RESOLVE_JSON_V1 => {
                let request = match serde_json::from_value::<AssetFileTypeProbeRequest>(envelope.request) {
                    Ok(request) => request,
                    Err(e) => return RResult::RErr(RString::from(format!("asset.file_types: invalid probe request: {e}"))),
                };
                ok_json(self.probe(request))
            }
            other => RResult::RErr(RString::from(format!("asset.file_types: unknown invoke method '{other}'"))),
        }
    }
}


fn warn_if_semantic_gateway_unresolved(desc: &AssetFileTypeDescriptor) {
    if !newengine_service_api::is_engine_service_gateway_id(&desc.semantic_gateway) {
        newengine_ulog_api::ulog::warn!(
            "asset type registry: invalid semantic_gateway='{}' extension='.{}' asset_kind='{}'; descriptors must target an engine.* gateway",
            desc.semantic_gateway,
            desc.extension,
            desc.asset_kind
        );
        return;
    }

    let is_byte_bucket_only = desc.semantic_gateway == newengine_assets_api::ENGINE_ASSET_SERVICE_ID;
    if is_byte_bucket_only && !desc.is_container_codec() {
        newengine_ulog_api::ulog::warn!(
            "asset type registry: semantic_gateway fell back to engine.assets for non-container extension='.{}' asset_kind='{}'; this is a byte-owner only fallback and must be replaced by a real domain gateway",
            desc.extension,
            desc.asset_kind
        );
    }
}

pub fn asset_types_service_info() -> AssetTypesServiceInfo {
    AssetTypeRegistryState::default().service_info()
}

pub fn register_asset_type_descriptor_best_effort(
    host: &HostApiV1,
    descriptor: AssetFileTypeDescriptor,
) -> bool {
    let payload = match serde_json::to_vec(&AssetFileTypeRegisterRequest { descriptor }) {
        Ok(payload) => payload,
        Err(e) => {
            newengine_ulog_api::ulog::warn!("asset type registry: failed to serialize descriptor registration: {e}");
            return false;
        }
    };
    let result = (host.call_service_v1)(
        RString::from(ENGINE_ASSET_TYPES_SERVICE_ID),
        MethodName::from(file_type_method::REGISTER_JSON_V1),
        Blob::from(payload),
    );
    match result.into_result() {
        Ok(_) => true,
        Err(e) => {
            newengine_ulog_api::ulog::warn!("asset type registry: descriptor self-registration failed: {e}");
            false
        }
    }
}

pub fn asset_types_gateway_service() -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let description = engine_gateway_provider_service_description(
        ASSET_TYPES_SERVICE_ID,
        "newengine-assets.file-type-registry",
        ASSET_TYPES_BACKEND_CAPABILITY_ID,
        ASSET_TYPES_SERVICE_METHODS.iter().copied(),
    )
    .gateway(ENGINE_ASSET_TYPES_SERVICE_ID)
    .protocol("json")
    .features(["codec-descriptor-registry", "self-registration", "provider-owned-format-descriptors"])
    .notes("Descriptor registry starts empty. Format crates/codecs/providers self-register descriptors; the registry only stores, validates and resolves them.");

    JsonServiceRouter::with_state(
        ASSET_TYPES_SERVICE_ID,
        AssetTypeRegistryState::default(),
    )
    .describe_json(&description)
    .info(asset_types_service_info)
    .get_json(file_type_method::MANIFEST_JSON_V1, |state| state.manifest())
    .post_json::<AssetFileTypeRegisterRequest, AssetFileTypeDescriptor, _>(
        file_type_method::REGISTER_JSON_V1,
        |state, request| state.register(request),
    )
    .post_json::<AssetFileTypeProbeRequest, AssetFileTypeProbeResult, _>(
        file_type_method::PROBE_JSON_V1,
        |state, request| state.probe(request),
    )
    .post_json::<AssetFileTypeProbeRequest, AssetFileTypeProbeResult, _>(
        file_type_method::RESOLVE_JSON_V1,
        |state, request| state.probe(request),
    )
    .blob(file_type_method::INVOKE_JSON, |state, payload| state.invoke_json(payload))
    .blob(file_type_method::SHUTDOWN_V1, |_state, _payload| ok_empty_blob())
    .into_service_v1()
}

pub fn register_asset_types_gateway_best_effort() -> bool {
    register_engine_gateway_provider_service_best_effort(EngineGatewayProviderDecl {
        gateway: ENGINE_ASSET_TYPES_SERVICE_ID,
        service_kind: EngineServiceKind::AssetTypes,
        provider_service: ASSET_TYPES_SERVICE_ID,
        provider_route: "engine.assets.starvault.types",
        capability: ASSET_TYPES_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: "newengine-assets.file-type-registry",
        service: asset_types_gateway_service(),
    })
}


#[cfg(test)]
mod tests {
    use super::*;

    fn explicit_descriptor(extension: &str, priority: i32, semantic_gateway: &str) -> AssetFileTypeDescriptor {
        AssetFileTypeDescriptor {
            extension: extension.to_owned(),
            asset_kind: "provider_declared_asset".to_owned(),
            container: format!("newengine.listfile.nef8.{extension}"),
            content_kind: Some(1000),
            codec_type: newengine_assets_api::codec_type::LIST_FILE.to_owned(),
            byte_owner: newengine_assets_api::ENGINE_ASSET_SERVICE_ID.to_owned(),
            semantic_gateway: semantic_gateway.to_owned(),
            handler_service: format!("asset.codec.listfile.{extension}"),
            selector_syntax: Some(format!("file.{extension}@entry")),
            consumer_domains: vec![semantic_gateway.to_owned()],
            magic: Some("4e454638".to_owned()),
            outputs: vec![newengine_assets_api::ASSET_LIST_FILE_MANIFEST_OUTPUT.to_owned(), "asset.blob".to_owned()],
            priority,
            vfs_backed: true,
            runtime_ready: true,
            allow_nested_assets: false,
            native_container: true,
            requires_magic: true,
            notes: "test descriptor declared by test format crate".to_owned(),
            ..Default::default()
        }
    }

    fn register_one(descriptor: AssetFileTypeDescriptor) -> AssetFileTypeDescriptor {
        let mut state = AssetTypeRegistryState::default();
        state.register(AssetFileTypeRegisterRequest { descriptor })
    }

    #[test]
    fn registry_starts_empty_until_formats_self_register() {
        let state = AssetTypeRegistryState::default();
        assert!(state.manifest().formats.is_empty());
    }

    #[test]
    fn registry_accepts_provider_declared_format_without_known_extension_or_gateway_branch() {
        let registered = register_one(explicit_descriptor("zzx", 0, "engine.assets.zzx"));
        assert_eq!(registered.extension, "zzx");
        assert_eq!(registered.semantic_gateway, "engine.assets.zzx");
        assert_eq!(registered.gateway, "engine.assets.zzx");
        assert_eq!(registered.content_kind, Some(1000));
        assert_eq!(
            newengine_service_api::service_kind_from_engine_gateway_id("engine.assets.zzx").as_deref(),
            Some("assets.zzx")
        );
    }

    #[test]
    fn registry_rejects_descriptor_without_self_declared_semantic_gateway() {
        let registered = register_one(AssetFileTypeDescriptor {
            extension: "bad".to_owned(),
            asset_kind: "provider_declared_asset".to_owned(),
            codec_type: newengine_assets_api::codec_type::LIST_FILE.to_owned(),
            handler_service: "asset.codec.listfile.bad".to_owned(),
            magic: Some("4e454638".to_owned()),
            ..Default::default()
        });
        assert!(registered.notes.contains("descriptor rejected"));
    }

    #[test]
    fn registry_uses_priority_for_same_extension_without_extension_specific_logic() {
        let mut state = AssetTypeRegistryState::default();
        let low = explicit_descriptor("same", 0, "engine.assets.low");
        let high = explicit_descriptor("same", 10, "engine.assets.high");
        state.register(AssetFileTypeRegisterRequest { descriptor: low });
        let registered = state.register(AssetFileTypeRegisterRequest { descriptor: high });
        assert_eq!(registered.semantic_gateway, "engine.assets.high");
        assert_eq!(state.probe(AssetFileTypeProbeRequest { logical_path: "foo.same@main".to_owned() }).descriptor.unwrap().semantic_gateway, "engine.assets.high");
    }

    #[test]
    fn registry_keeps_container_semantics_generic() {
        let registered = register_one(AssetFileTypeDescriptor {
            extension: "pkgx".to_owned(),
            asset_kind: "asset_package".to_owned(),
            container: "newengine.asset_package.provider_declared".to_owned(),
            codec_type: newengine_assets_api::codec_type::CONTAINER.to_owned(),
            byte_owner: newengine_assets_api::ENGINE_ASSET_SERVICE_ID.to_owned(),
            semantic_gateway: newengine_assets_api::ENGINE_ASSET_SERVICE_ID.to_owned(),
            handler_service: "asset.codec.pkgx".to_owned(),
            magic: Some("4e4550414b010000".to_owned()),
            consumer_domains: vec![newengine_assets_api::ENGINE_ASSET_SERVICE_ID.to_owned()],
            allow_nested_assets: true,
            native_container: true,
            runtime_ready: true,
            requires_magic: true,
            ..Default::default()
        });
        assert_eq!(registered.semantic_gateway, newengine_assets_api::ENGINE_ASSET_SERVICE_ID);
        assert_eq!(registered.consumer_domains, vec![newengine_assets_api::ENGINE_ASSET_SERVICE_ID.to_owned()]);
        assert!(registered.selector_syntax.is_none());
    }
}

fn normalize_logical_path(path: &str) -> String {
    let mut s = path.trim().replace('\\', "/");
    while let Some(rest) = s.strip_prefix("./") {
        s = rest.to_owned();
    }
    s.trim_start_matches('/').to_ascii_lowercase()
}

fn path_extension(path: &str) -> String {
    let path = path.split('@').next().unwrap_or(path);
    path.rsplit_once('.')
        .map(|(_, ext)| AssetFileTypeDescriptor::extension_key(ext))
        .unwrap_or_default()
}
