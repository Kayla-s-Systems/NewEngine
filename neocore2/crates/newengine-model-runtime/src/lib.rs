#![forbid(unsafe_op_in_unsafe_fn)]

//! Gateway-backed model construction runtime.
//!
//! The adapter owns bundle assembly, but not host discovery. Runtime/profile
//! integration injects `AssetServiceClient` and may register this adapter as the
//! `engine.model` service. This keeps player, NPC and prop construction on the
//! same domain service path.

use abi_stable::std_types::{RResult, RString};
use newengine_assets::{AssetDecodeRequest, AssetServiceClient};
use newengine_model_domain_api::{
    build_data_driven_construction_plan, DataDrivenConstructionPlan, DefinitionEntriesManifest, DefinitionEntriesRequest, DrawableDictionaryManifest, DrawableDictionaryRequest,
    ModelAssetBundle, ModelAssetChainManifest, ModelAssetRequest, ModelConstructionManifest, ModelConstructionValidation, ModelMeshPart,
    DRAWABLE_DICTIONARY_EXTENSION, ENGINE_MODEL_SERVICE_ID, MODEL_BACKEND_CAPABILITY_ID,
    MODEL_FEATURE_DOMAINS, MODEL_SERVICE_ID, MODEL_SERVICE_METHOD_ASSEMBLE_JSON_V1,
    MODEL_SERVICE_METHOD_ASSET_CHAIN_JSON_V1, MODEL_SERVICE_METHOD_CONSTRUCTION_PLAN_JSON_V1, MODEL_SERVICE_METHOD_DEFINITION_ENTRIES_JSON_V1,
    MODEL_SERVICE_METHOD_DRAWABLE_DICTIONARY_MANIFEST_JSON_V1, MODEL_SERVICE_METHOD_INVOKE, MODEL_SERVICE_METHOD_VALIDATE_JSON_V1, MODEL_SERVICE_METHODS,
    OBJECT_TYPE_DEFINITIONS_EXTENSION,
};
use newengine_model_import_obj::normalize_logical_path;
use newengine_model_skeleton_api::ModelSkeletonMetadata;
use newengine_plugin_api::{Blob, HostApiV1, MethodName};
use newengine_service_api::EngineServiceKind;
use newengine_service_kit::{
    engine_owned_service_description, ok_empty_blob, ok_json, register_engine_owned_gateway_service_best_effort,
    EngineOwnedGatewayDecl, JsonServiceRouter,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

#[derive(Clone)]
pub struct ModelAssetAdapter {
    client: AssetServiceClient,
}

impl ModelAssetAdapter {
    #[inline]
    pub fn with_client(client: AssetServiceClient) -> Self {
        Self { client }
    }

    pub fn load_bundle(&self, request: &ModelAssetRequest) -> Result<ModelAssetBundle, String> {
        let request = self.resolve_request(request)?;
        let target_height = request.target_height.clamp(0.25, 3.0);
        let source = normalize_logical_path(&request.model, false)?;
        let texture_dictionary = request
            .texture_dictionary
            .as_deref()
            .map(|path| normalize_logical_path(path, false))
            .transpose()?
            .filter(|path| path.ends_with(".ytd") || path.ends_with(".neytd"));

        let skeleton = match request.skeleton.as_deref() {
            Some(path) => Some(self.load_skeleton_metadata(path, target_height, request.eye_height_ratio)?),
            None => None,
        };

        let obj_text = self.read_text(&source)?;
        let decoded = newengine_model_import_obj::decode_obj_with_mtl_loader(
            &source,
            &obj_text,
            target_height,
            |path| self.read_text(path).ok(),
        )?;

        let mut parts = Vec::with_capacity(decoded.parts.len());
        for part in decoded.parts {
            let material = newengine_material_runtime::material_binding(
                &part.material_slot,
                decoded.materials.get(&part.material_slot),
                texture_dictionary.as_deref(),
            );
            parts.push(ModelMeshPart { material_slot: part.material_slot, mesh: part.mesh, material });
        }

        let collisions = if request.collisions.is_empty() {
            newengine_model_collision_runtime::default_collisions_for_model(skeleton.as_ref(), target_height)
        } else {
            request.collisions.clone()
        };

        Ok(ModelAssetBundle { source, parts, skeleton, texture_dictionary, collisions })
    }

    pub fn load_manifest(&self, logical_path: &str) -> Result<ModelConstructionManifest, String> {
        let source = normalize_logical_path(logical_path, false)?;
        let text = self.read_text(&source)?;
        serde_json::from_str::<ModelConstructionManifest>(&text)
            .map_err(|e| format!("model manifest parse failed path='{source}' err='{e}'"))
    }

    pub fn resolve_request(&self, request: &ModelAssetRequest) -> Result<ModelAssetRequest, String> {
        let Some(manifest_path) = request.manifest.as_deref() else { return Ok(request.clone()); };
        let manifest = self.load_manifest(manifest_path)?;
        let mut resolved = request.clone();
        if resolved.model.trim().is_empty() {
            resolved.model = manifest.model;
        }
        if resolved.skeleton.is_none() {
            resolved.skeleton = manifest.skeleton.map(|it| it.source);
        }
        if resolved.texture_dictionary.is_none() {
            resolved.texture_dictionary = manifest.material_set.texture_dictionary;
        }
        if resolved.collisions.is_empty() {
            resolved.collisions = manifest.collisions;
        }
        if (resolved.target_height - ModelAssetRequest::default().target_height).abs() < f32::EPSILON {
            resolved.target_height = manifest.target_height;
        }
        if (resolved.eye_height_ratio - ModelAssetRequest::default().eye_height_ratio).abs() < f32::EPSILON {
            resolved.eye_height_ratio = manifest.eye_height_ratio;
        }
        Ok(resolved)
    }

    pub fn validate_request(&self, request: &ModelAssetRequest) -> ModelConstructionValidation {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let resolved = match self.resolve_request(request) {
            Ok(resolved) => Some(resolved),
            Err(e) => {
                errors.push(e);
                None
            }
        };
        if let Some(resolved) = resolved.as_ref() {
            if resolved.model.trim().is_empty() {
                errors.push("model asset path is empty after manifest resolution".to_owned());
            }
            if resolved.skeleton.is_none() {
                warnings.push("no skeleton source declared; runtime model will use mesh-only binding".to_owned());
            }
            if resolved.texture_dictionary.is_none() {
                warnings.push("no texture dictionary declared; data-driven construction should declare a .ytd texture dictionary".to_owned());
            }
        }
        ModelConstructionValidation { valid: errors.is_empty(), resolved, errors, warnings }
    }

    pub fn load_drawable_dictionary_manifest(
        &self,
        request: &DrawableDictionaryRequest,
    ) -> Result<DrawableDictionaryManifest, String> {
        self.decode_model_manifest(
            &request.source,
            request.selector.as_deref(),
            DRAWABLE_DICTIONARY_EXTENSION,
            "drawable.manifest_json",
            "ydd drawable dictionary manifest",
        )
    }

    pub fn load_definition_entries_manifest(
        &self,
        request: &DefinitionEntriesRequest,
    ) -> Result<DefinitionEntriesManifest, String> {
        self.decode_model_manifest(
            &request.source,
            request.selector.as_deref(),
            OBJECT_TYPE_DEFINITIONS_EXTENSION,
            "model.definition_entries_json",
            "ytyp definition entries",
        )
    }

    pub fn load_construction_plan(
        &self,
        request: &DefinitionEntriesRequest,
    ) -> Result<DataDrivenConstructionPlan, String> {
        let manifest = self.load_definition_entries_manifest(request)?;
        Ok(build_data_driven_construction_plan(&manifest))
    }

    fn decode_model_manifest<T>(
        &self,
        source: &str,
        selector: Option<&str>,
        extension: &str,
        output_kind: &str,
        label: &str,
    ) -> Result<T, String>
    where
        T: DeserializeOwned,
    {
        let source = normalize_logical_path(source, false)?;
        if !has_extension(&source, extension) {
            return Err(format!(
                "{label} requires .{} source, got '{source}'",
                extension.trim_start_matches('.')
            ));
        }
        let bytes = self.client.decode_v1(&AssetDecodeRequest {
            logical_path: source.clone(),
            output_kind: output_kind.to_owned(),
            selector: selector
                .map(|selector| serde_json::json!({"selector": selector}))
                .unwrap_or(serde_json::Value::Null),
        })
        .map_err(|e| format!("engine.assets decode_v1 failed path='{source}' output='{output_kind}' err='{e}'"))?;
        serde_json::from_slice::<T>(&bytes)
            .map_err(|e| format!("model.api: {label} decode returned invalid json path='{source}' err='{e}'"))
    }



    pub fn load_skeleton_metadata(
        &self,
        logical_path: &str,
        target_height: f32,
        eye_height_ratio: f32,
    ) -> Result<ModelSkeletonMetadata, String> {
        let source = normalize_logical_path(logical_path, false)?;
        let bytes = self.read_bytes(&source)?;
        newengine_model_skeleton_rsc7::probe_rsc7_ymt_skeleton_metadata(&source, &bytes, target_height, eye_height_ratio)
    }

    fn read_bytes(&self, logical_path: &str) -> Result<Vec<u8>, String> {
        let path = normalize_logical_path(logical_path, false)?;
        self.client
            .raw_bytes_v1(&path)
            .map_err(|e| format!("asset.raw_bytes_v1 failed path='{path}' err='{e}'"))
    }

    fn read_text(&self, logical_path: &str) -> Result<String, String> {
        let path = normalize_logical_path(logical_path, false)?;
        let bytes = self.read_bytes(&path)?;
        String::from_utf8(bytes).map_err(|e| format!("asset text is not UTF-8 path='{path}' err='{e}'"))
    }
}

fn has_extension(path: &str, extension: &str) -> bool {
    let expected = extension.trim().trim_start_matches('.').to_ascii_lowercase();
    path.split('@')
        .next()
        .unwrap_or(path)
        .to_ascii_lowercase()
        .ends_with(&format!(".{expected}"))
}

#[derive(Clone)]
pub struct ModelGatewayClient {
    host: HostApiV1,
    service_id: RString,
}

impl ModelGatewayClient {
    #[inline]
    pub fn new(host: HostApiV1) -> Self {
        Self { host, service_id: RString::from(ENGINE_MODEL_SERVICE_ID) }
    }

    #[inline]
    pub fn with_service_id(host: HostApiV1, service_id: &str) -> Self {
        Self { host, service_id: RString::from(service_id) }
    }

    pub fn assemble_bundle(&self, request: &ModelAssetRequest) -> Result<ModelAssetBundle, String> {
        let payload = serde_json::to_vec(request).map_err(|e| e.to_string())?;
        let bytes = self.call_raw(MODEL_SERVICE_METHOD_ASSEMBLE_JSON_V1, payload)?;
        serde_json::from_slice::<ModelAssetBundle>(&bytes)
            .map_err(|e| format!("engine.model returned invalid ModelAssetBundle JSON: {e}"))
    }

    pub fn validate_request(&self, request: &ModelAssetRequest) -> Result<ModelConstructionValidation, String> {
        let payload = serde_json::to_vec(request).map_err(|e| e.to_string())?;
        let bytes = self.call_raw(MODEL_SERVICE_METHOD_VALIDATE_JSON_V1, payload)?;
        serde_json::from_slice::<ModelConstructionValidation>(&bytes)
            .map_err(|e| format!("engine.model returned invalid validation JSON: {e}"))
    }

    pub fn drawable_dictionary_manifest(
        &self,
        request: &DrawableDictionaryRequest,
    ) -> Result<DrawableDictionaryManifest, String> {
        let payload = serde_json::to_vec(request).map_err(|e| e.to_string())?;
        let bytes = self.call_raw(MODEL_SERVICE_METHOD_DRAWABLE_DICTIONARY_MANIFEST_JSON_V1, payload)?;
        serde_json::from_slice::<DrawableDictionaryManifest>(&bytes)
            .map_err(|e| format!("engine.model returned invalid drawable dictionary manifest JSON: {e}"))
    }

    pub fn definition_entries_manifest(
        &self,
        request: &DefinitionEntriesRequest,
    ) -> Result<DefinitionEntriesManifest, String> {
        let payload = serde_json::to_vec(request).map_err(|e| e.to_string())?;
        let bytes = self.call_raw(MODEL_SERVICE_METHOD_DEFINITION_ENTRIES_JSON_V1, payload)?;
        serde_json::from_slice::<DefinitionEntriesManifest>(&bytes)
            .map_err(|e| format!("engine.model returned invalid definition entries JSON: {e}"))
    }

    pub fn asset_chain_manifest(&self) -> Result<ModelAssetChainManifest, String> {
        let bytes = self.call_raw(MODEL_SERVICE_METHOD_ASSET_CHAIN_JSON_V1, Vec::new())?;
        serde_json::from_slice::<ModelAssetChainManifest>(&bytes)
            .map_err(|e| format!("engine.model returned invalid asset chain JSON: {e}"))
    }

    pub fn construction_plan(
        &self,
        request: &DefinitionEntriesRequest,
    ) -> Result<DataDrivenConstructionPlan, String> {
        let payload = serde_json::to_vec(request).map_err(|e| e.to_string())?;
        let bytes = self.call_raw(MODEL_SERVICE_METHOD_CONSTRUCTION_PLAN_JSON_V1, payload)?;
        serde_json::from_slice::<DataDrivenConstructionPlan>(&bytes)
            .map_err(|e| format!("engine.model returned invalid construction plan JSON: {e}"))
    }

    fn call_raw(&self, method_name: &str, payload: Vec<u8>) -> Result<Vec<u8>, String> {
        (self.host.call_service_v1)(
            self.service_id.clone(),
            MethodName::from(method_name),
            Blob::from(payload),
        )
        .into_result()
        .map(|value| value.into_vec())
        .map_err(|err| err.to_string())
    }
}

#[derive(Clone)]
struct ModelRuntimeState {
    adapter: ModelAssetAdapter,
}

#[derive(Clone, Debug, Serialize)]
pub struct ModelRuntimeServiceInfo {
    pub id: &'static str,
    pub gateway: &'static str,
    pub methods: &'static [&'static str],
    pub backend: &'static str,
    pub feature_domains: &'static [&'static str],
}

impl ModelRuntimeState {
    fn new(adapter: ModelAssetAdapter) -> Self { Self { adapter } }

    fn invoke_json(&mut self, payload: Blob) -> RResult<Blob, RString> {
        #[derive(Deserialize)]
        struct InvokeEnvelope {
            method: String,
            #[serde(default)]
            request: serde_json::Value,
        }
        let envelope = match serde_json::from_slice::<InvokeEnvelope>(payload.as_slice()) {
            Ok(envelope) => envelope,
            Err(e) => return RResult::RErr(RString::from(format!("model.api: invalid invoke_json payload: {e}"))),
        };
        match envelope.method.as_str() {
            MODEL_SERVICE_METHOD_ASSEMBLE_JSON_V1 => {
                let request = match serde_json::from_value::<ModelAssetRequest>(envelope.request) {
                    Ok(request) => request,
                    Err(e) => return RResult::RErr(RString::from(format!("model.api: invalid assemble request: {e}"))),
                };
                match self.adapter.load_bundle(&request) {
                    Ok(bundle) => ok_json(bundle),
                    Err(e) => RResult::RErr(RString::from(e)),
                }
            }
            MODEL_SERVICE_METHOD_VALIDATE_JSON_V1 => {
                let request = match serde_json::from_value::<ModelAssetRequest>(envelope.request) {
                    Ok(request) => request,
                    Err(e) => return RResult::RErr(RString::from(format!("model.api: invalid validate request: {e}"))),
                };
                ok_json(self.adapter.validate_request(&request))
            }
            MODEL_SERVICE_METHOD_DRAWABLE_DICTIONARY_MANIFEST_JSON_V1 => {
                let request = match serde_json::from_value::<DrawableDictionaryRequest>(envelope.request) {
                    Ok(request) => request,
                    Err(e) => return RResult::RErr(RString::from(format!("model.api: invalid ydd manifest request: {e}"))),
                };
                match self.adapter.load_drawable_dictionary_manifest(&request) {
                    Ok(manifest) => ok_json(manifest),
                    Err(e) => RResult::RErr(RString::from(e)),
                }
            }
            MODEL_SERVICE_METHOD_DEFINITION_ENTRIES_JSON_V1 => {
                let request = match serde_json::from_value::<DefinitionEntriesRequest>(envelope.request) {
                    Ok(request) => request,
                    Err(e) => return RResult::RErr(RString::from(format!("model.api: invalid ytyp definition entries request: {e}"))),
                };
                match self.adapter.load_definition_entries_manifest(&request) {
                    Ok(manifest) => ok_json(manifest),
                    Err(e) => RResult::RErr(RString::from(e)),
                }
            }
            MODEL_SERVICE_METHOD_ASSET_CHAIN_JSON_V1 => ok_json(ModelAssetChainManifest::default()),
            MODEL_SERVICE_METHOD_CONSTRUCTION_PLAN_JSON_V1 => {
                let request = match serde_json::from_value::<DefinitionEntriesRequest>(envelope.request) {
                    Ok(request) => request,
                    Err(e) => return RResult::RErr(RString::from(format!("model.api: invalid construction plan request: {e}"))),
                };
                match self.adapter.load_construction_plan(&request) {
                    Ok(plan) => ok_json(plan),
                    Err(e) => RResult::RErr(RString::from(e)),
                }
            }
            other => RResult::RErr(RString::from(format!("model.api: unknown invoke method '{other}'"))),
        }
    }
}

pub fn model_runtime_service_info() -> ModelRuntimeServiceInfo {
    ModelRuntimeServiceInfo {
        id: MODEL_SERVICE_ID,
        gateway: ENGINE_MODEL_SERVICE_ID,
        methods: MODEL_SERVICE_METHODS,
        backend: "engine-owned.model-runtime",
        feature_domains: MODEL_FEATURE_DOMAINS,
    }
}

pub fn model_gateway_service(adapter: ModelAssetAdapter) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let description = engine_owned_service_description(
        MODEL_SERVICE_ID,
        "newengine-model-runtime.model-gateway",
        MODEL_BACKEND_CAPABILITY_ID,
        MODEL_SERVICE_METHODS.iter().copied(),
    )
    .gateway(ENGINE_MODEL_SERVICE_ID)
    .protocol("json")
    .features(MODEL_FEATURE_DOMAINS.iter().copied())
    .notes("Gateway-backed data-driven model constructor for YTYP/YDD/YTD player/NPC/prop construction");

    JsonServiceRouter::with_state(MODEL_SERVICE_ID, ModelRuntimeState::new(adapter))
        .describe_json(&description)
        .info(model_runtime_service_info)
        .blob(MODEL_SERVICE_METHOD_INVOKE, |state, payload| state.invoke_json(payload))
        .post_json_result::<ModelAssetRequest, ModelAssetBundle, _>(
            MODEL_SERVICE_METHOD_ASSEMBLE_JSON_V1,
            |state, request| state.adapter.load_bundle(&request),
        )
        .post_json::<ModelAssetRequest, ModelConstructionValidation, _>(
            MODEL_SERVICE_METHOD_VALIDATE_JSON_V1,
            |state, request| state.adapter.validate_request(&request),
        )
        .post_json_result::<DrawableDictionaryRequest, DrawableDictionaryManifest, _>(
            MODEL_SERVICE_METHOD_DRAWABLE_DICTIONARY_MANIFEST_JSON_V1,
            |state, request| state.adapter.load_drawable_dictionary_manifest(&request),
        )
        .post_json_result::<DefinitionEntriesRequest, DefinitionEntriesManifest, _>(
            MODEL_SERVICE_METHOD_DEFINITION_ENTRIES_JSON_V1,
            |state, request| state.adapter.load_definition_entries_manifest(&request),
        )
        .blob(MODEL_SERVICE_METHOD_ASSET_CHAIN_JSON_V1, |_state, _payload| ok_json(ModelAssetChainManifest::default()))
        .post_json_result::<DefinitionEntriesRequest, DataDrivenConstructionPlan, _>(
            MODEL_SERVICE_METHOD_CONSTRUCTION_PLAN_JSON_V1,
            |state, request| state.adapter.load_construction_plan(&request),
        )
        .blob(newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1, |_state, _payload| ok_empty_blob())
        .into_service_v1()
}

pub fn register_model_gateway_best_effort(client: AssetServiceClient) -> bool {
    register_engine_owned_gateway_service_best_effort(EngineOwnedGatewayDecl {
        gateway: ENGINE_MODEL_SERVICE_ID,
        service_kind: EngineServiceKind::Model,
        provider_service: MODEL_SERVICE_ID,
        capability: MODEL_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: "newengine-model-runtime.model-gateway",
        service: model_gateway_service(ModelAssetAdapter::with_client(client)),
    })
}
