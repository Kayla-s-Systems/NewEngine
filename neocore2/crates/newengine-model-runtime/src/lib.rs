#![forbid(unsafe_op_in_unsafe_fn)]

//! Gateway-backed drawable/model runtime.
//!
//! `engine.assets.models` owns only drawable/model semantics. Definition metadata is read
//! through `engine.assets.definitions`; dependency graph expansion is read through
//! `engine.assets.graph`. This crate still hosts the current engine-runtime graph
//! provider implementation; definition decoding is not part of model API.
use abi_stable::std_types::{RResult, RString};
use newengine_assets_api::{AssetDecodeRequest, AssetService, AssetServiceClient};
use newengine_math::Vec3;
use newengine_model_domain_api::{
    attach_content_hash, attach_metadata_namespace, attach_node_warning, attach_vfs_source,
    finalize_graph, fnv1a64, normalize_asset_ref, push_manifest_dependency, split_asset_ref,
    AssetGraphResolver, AssetGraphVfsSource, DrawableDictionaryManifest, DrawableDictionaryRequest,
    FoliageImportRequestV1, FoliageImportResponseV1, ModelAssetBundle, ModelAssetRequest,
    ModelConstructionManifest, ModelConstructionValidation, ModelMaterialBinding, ModelMeshPart,
    ModelRuntimeConfiguration, ModelSkinBinding, ModelSkinVertex, ResolvedAssetGraphV2,
    DRAWABLE_DICTIONARY_EXTENSION, ENGINE_ASSETS_MODELS_SERVICE_ID, MODEL_BACKEND_CAPABILITY_ID,
    MODEL_FEATURE_DOMAINS, MODEL_SERVICE_ID, MODEL_SERVICE_METHODS,
    MODEL_SERVICE_METHOD_ASSEMBLE_JSON_V1,
    MODEL_SERVICE_METHOD_DRAWABLE_DICTIONARY_MANIFEST_JSON_V1,
    MODEL_SERVICE_METHOD_IMPORT_FOLIAGE_V1, MODEL_SERVICE_METHOD_INVOKE,
    MODEL_SERVICE_METHOD_RESOLVE_DRAWABLE_V1, MODEL_SERVICE_METHOD_VALIDATE_JSON_V1,
};
use newengine_model_import_obj::normalize_logical_path;
use newengine_model_skeleton_api::ModelSkeletonMetadata;
use newengine_plugin_api::{Blob, HostApiV1, MethodName};
use newengine_primitives::{PrimitiveMesh, PrimitiveVertex};
use newengine_service_api::EngineServiceKind;
use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_empty_blob, ok_json, payload_json,
    register_engine_gateway_provider_service_best_effort, EngineGatewayProviderDecl,
    JsonServiceRouter,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

mod adapter;
mod asset_graph_gateway;
mod skeleton_metadata;

pub use adapter::ModelAssetAdapter;
pub use asset_graph_gateway::register_asset_graph_gateway_best_effort;

pub use newengine_model_client::ModelGatewayClient;

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
    fn new(adapter: ModelAssetAdapter) -> Self {
        Self { adapter }
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
            Err(e) => {
                return RResult::RErr(RString::from(format!(
                    "model.api: invalid invoke_json payload: {e}"
                )))
            }
        };
        match envelope.method.as_str() {
            MODEL_SERVICE_METHOD_ASSEMBLE_JSON_V1 => {
                let request = match serde_json::from_value::<ModelAssetRequest>(envelope.request) {
                    Ok(request) => request,
                    Err(e) => {
                        return RResult::RErr(RString::from(format!(
                            "model.api: invalid assemble request: {e}"
                        )))
                    }
                };
                match self.adapter.load_bundle(&request) {
                    Ok(bundle) => ok_json(bundle),
                    Err(e) => RResult::RErr(RString::from(e)),
                }
            }
            MODEL_SERVICE_METHOD_VALIDATE_JSON_V1 => {
                let request = match serde_json::from_value::<ModelAssetRequest>(envelope.request) {
                    Ok(request) => request,
                    Err(e) => {
                        return RResult::RErr(RString::from(format!(
                            "model.api: invalid validate request: {e}"
                        )))
                    }
                };
                ok_json(self.adapter.validate_request(&request))
            }
            MODEL_SERVICE_METHOD_DRAWABLE_DICTIONARY_MANIFEST_JSON_V1
            | MODEL_SERVICE_METHOD_RESOLVE_DRAWABLE_V1 => {
                let request =
                    match serde_json::from_value::<DrawableDictionaryRequest>(envelope.request) {
                        Ok(request) => request,
                        Err(e) => {
                            return RResult::RErr(RString::from(format!(
                                "model.api: invalid drawable request: {e}"
                            )))
                        }
                    };
                match self.adapter.resolve_drawable(&request) {
                    Ok(manifest) => ok_json(manifest),
                    Err(e) => RResult::RErr(RString::from(e)),
                }
            }
            MODEL_SERVICE_METHOD_IMPORT_FOLIAGE_V1 => {
                let request =
                    match serde_json::from_value::<FoliageImportRequestV1>(envelope.request) {
                        Ok(request) => request,
                        Err(e) => {
                            return RResult::RErr(RString::from(format!(
                                "model.api: invalid foliage import request: {e}"
                            )))
                        }
                    };
                match self.adapter.import_foliage_source(&request) {
                    Ok(response) => ok_json(response),
                    Err(e) => RResult::RErr(RString::from(e)),
                }
            }
            other => RResult::RErr(RString::from(format!(
                "model.api: unknown invoke method '{other}'"
            ))),
        }
    }
}

pub fn model_runtime_service_info() -> ModelRuntimeServiceInfo {
    ModelRuntimeServiceInfo {
        id: MODEL_SERVICE_ID,
        gateway: ENGINE_ASSETS_MODELS_SERVICE_ID,
        methods: MODEL_SERVICE_METHODS,
        backend: "engine.assets.starvault.models-runtime",
        feature_domains: MODEL_FEATURE_DOMAINS,
    }
}

pub fn model_gateway_service(
    adapter: ModelAssetAdapter,
) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let description = engine_gateway_provider_service_description(
        MODEL_SERVICE_ID,
        "newengine-model-runtime.model-gateway",
        MODEL_BACKEND_CAPABILITY_ID,
        MODEL_SERVICE_METHODS.iter().copied(),
    )
    .gateway(ENGINE_ASSETS_MODELS_SERVICE_ID)
    .protocol("json")
    .features(MODEL_FEATURE_DOMAINS.iter().copied())
    .notes("Gateway-backed drawable/model service. Definition metadata is owned by engine.assets.definitions; asset graph expansion is owned by engine.assets.graph.");

    JsonServiceRouter::with_state(MODEL_SERVICE_ID, ModelRuntimeState::new(adapter))
        .describe_json(&description)
        .info(model_runtime_service_info)
        .blob(MODEL_SERVICE_METHOD_INVOKE, |state, payload| {
            state.invoke_json(payload)
        })
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
        .post_json_result::<DrawableDictionaryRequest, DrawableDictionaryManifest, _>(
            MODEL_SERVICE_METHOD_RESOLVE_DRAWABLE_V1,
            |state, request| state.adapter.resolve_drawable(&request),
        )
        .post_json_result::<FoliageImportRequestV1, FoliageImportResponseV1, _>(
            MODEL_SERVICE_METHOD_IMPORT_FOLIAGE_V1,
            |state, request| state.adapter.import_foliage_source(&request),
        )
        .blob(
            newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1,
            |_state, _payload| ok_empty_blob(),
        )
        .into_service_v1()
}

pub fn register_model_gateway_best_effort(client: AssetServiceClient) -> bool {
    register_model_gateway_service(client, None)
}

pub fn register_model_gateway_best_effort_with_host(
    host: HostApiV1,
    client: AssetServiceClient,
) -> bool {
    register_model_gateway_service(client, Some(host))
}

fn register_model_gateway_service(client: AssetServiceClient, host: Option<HostApiV1>) -> bool {
    let adapter = match host {
        Some(host) => ModelAssetAdapter::with_client_and_host(client, host),
        None => ModelAssetAdapter::with_client(client),
    };
    register_engine_gateway_provider_service_best_effort(EngineGatewayProviderDecl {
        gateway: ENGINE_ASSETS_MODELS_SERVICE_ID,
        service_kind: EngineServiceKind::Model,
        provider_service: MODEL_SERVICE_ID,
        provider_route: "engine.assets.starvault.models",
        capability: MODEL_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: "newengine-model-runtime.model-gateway",
        service: model_gateway_service(adapter),
    })
}

pub const MODELS_RUNTIME_UNIT_SPEC: newengine_runtime_unit_api::EngineRuntimeUnitSpec =
    newengine_runtime_unit_api::EngineRuntimeUnitSpec::new(
        "engine.runtime.models",
        1,
        newengine_runtime_unit_api::EngineRuntimeUnitKind::Provider,
        &[newengine_model_domain_api::MODEL_BACKEND_CAPABILITY_ID],
        &[
            newengine_assets_api::ASSET_BACKEND_CAPABILITY_ID,
            newengine_materials::MATERIALS_BACKEND_CAPABILITY_ID,
        ],
        newengine_runtime_unit_api::STATIC_PROVIDER_TAGS,
    );

pub const ASSET_GRAPH_RUNTIME_UNIT_SPEC: newengine_runtime_unit_api::EngineRuntimeUnitSpec =
    newengine_runtime_unit_api::EngineRuntimeUnitSpec::new(
        "engine.runtime.asset-graph",
        1,
        newengine_runtime_unit_api::EngineRuntimeUnitKind::Provider,
        &[newengine_model_domain_api::ASSET_GRAPH_BACKEND_CAPABILITY_ID],
        &[
            newengine_assets_api::ASSET_BACKEND_CAPABILITY_ID,
            newengine_model_domain_api::MODEL_BACKEND_CAPABILITY_ID,
            newengine_assets_api::DEFINITIONS_BACKEND_CAPABILITY_ID,
        ],
        newengine_runtime_unit_api::STATIC_PROVIDER_TAGS,
    );

fn models_runtime_unit_factory(
    _: &mut newengine_runtime_unit_api::Engine<()>,
    _: &newengine_runtime_unit_api::StartupConfig,
) -> newengine_runtime_unit_api::EngineResult<Option<Box<dyn newengine_runtime_unit_api::Module<()>>>>
{
    let host = newengine_plugin_host::default_host_api();
    let client = newengine_assets_api::AssetServiceClient::new(host.clone());
    let _ = register_model_gateway_best_effort_with_host(host, client);
    Ok(None)
}

fn asset_graph_runtime_unit_factory(
    _: &mut newengine_runtime_unit_api::Engine<()>,
    _: &newengine_runtime_unit_api::StartupConfig,
) -> newengine_runtime_unit_api::EngineResult<Option<Box<dyn newengine_runtime_unit_api::Module<()>>>>
{
    let host = newengine_plugin_host::default_host_api();
    let client = newengine_assets_api::AssetServiceClient::new(host.clone());
    let _ = register_asset_graph_gateway_best_effort(host, client);
    Ok(None)
}

pub const MODELS_RUNTIME_UNIT_REGISTRATION: newengine_runtime_unit_api::RuntimeUnitRegistration =
    newengine_runtime_unit_api::RuntimeUnitRegistration::new(
        MODELS_RUNTIME_UNIT_SPEC,
        models_runtime_unit_factory,
    );
pub const ASSET_GRAPH_RUNTIME_UNIT_REGISTRATION:
    newengine_runtime_unit_api::RuntimeUnitRegistration =
    newengine_runtime_unit_api::RuntimeUnitRegistration::new(
        ASSET_GRAPH_RUNTIME_UNIT_SPEC,
        asset_graph_runtime_unit_factory,
    );
