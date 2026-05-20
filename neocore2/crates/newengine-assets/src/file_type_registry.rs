#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::{RResult, RString};
use newengine_assets_api::{
    file_type_method, AssetFileTypeDescriptor, AssetFileTypeManifest,
    AssetFileTypeProbeRequest, AssetFileTypeProbeResult, AssetFileTypeRegisterRequest,
    ASSET_FILE_TYPE_SERVICE_METHODS, ASSET_FILE_TYPES_BACKEND_CAPABILITY_ID,
    ASSET_FILE_TYPES_SERVICE_ID, ENGINE_ASSET_FILE_TYPES_SERVICE_ID,
};
use newengine_plugin_api::Blob;
use newengine_service_api::EngineServiceKind;
use newengine_service_kit::{
    engine_owned_service_description, ok_empty_blob, ok_json,
    register_engine_owned_gateway_service_best_effort, EngineOwnedGatewayDecl, JsonServiceRouter,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize)]
pub struct AssetFileTypesServiceInfo {
    pub id: &'static str,
    pub gateway: &'static str,
    pub methods: &'static [&'static str],
    pub backend: &'static str,
    pub registered_extensions: Vec<String>,
}

#[derive(Clone, Default)]
struct FileTypeRegistryState {
    registry: BTreeMap<String, AssetFileTypeDescriptor>,
}

impl FileTypeRegistryState {
    fn service_info(&self) -> AssetFileTypesServiceInfo {
        AssetFileTypesServiceInfo {
            id: ASSET_FILE_TYPES_SERVICE_ID,
            gateway: ENGINE_ASSET_FILE_TYPES_SERVICE_ID,
            methods: ASSET_FILE_TYPE_SERVICE_METHODS,
            backend: "engine-owned.asset-file-type-registry",
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
        desc.extension = AssetFileTypeDescriptor::extension_key(&desc.extension);
        if desc.handler_service.trim().is_empty() {
            desc.handler_service = desc.gateway.clone();
        }
        if let Err(e) = desc.validate_generic_rules() {
            let mut rejected = desc.clone();
            rejected.notes = format!("descriptor rejected by generic codec rules: {e}");
            return rejected;
        }
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

pub fn asset_file_types_service_info() -> AssetFileTypesServiceInfo {
    FileTypeRegistryState::default().service_info()
}

pub fn asset_file_types_gateway_service() -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let description = engine_owned_service_description(
        ASSET_FILE_TYPES_SERVICE_ID,
        "newengine-assets.file-type-registry",
        ASSET_FILE_TYPES_BACKEND_CAPABILITY_ID,
        ASSET_FILE_TYPE_SERVICE_METHODS.iter().copied(),
    )
    .gateway(ENGINE_ASSET_FILE_TYPES_SERVICE_ID)
    .protocol("json")
    .features(["codec-descriptor-registry", "self-registration"])
    .notes("Empty descriptor registry. Asset codecs/providers register themselves when ready; the registry does not know or parse formats.");

    JsonServiceRouter::with_state(
        ASSET_FILE_TYPES_SERVICE_ID,
        FileTypeRegistryState::default(),
    )
    .describe_json(&description)
    .info(asset_file_types_service_info)
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

pub fn register_asset_file_types_gateway_best_effort() -> bool {
    register_engine_owned_gateway_service_best_effort(EngineOwnedGatewayDecl {
        gateway: ENGINE_ASSET_FILE_TYPES_SERVICE_ID,
        service_kind: EngineServiceKind::AssetFileTypes,
        provider_service: ASSET_FILE_TYPES_SERVICE_ID,
        capability: ASSET_FILE_TYPES_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: "newengine-assets.file-type-registry",
        service: asset_file_types_gateway_service(),
    })
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
