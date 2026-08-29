use abi_stable::std_types::{RResult, RString};
use newengine_assets::AssetServiceClient;
use newengine_materials::{
    method as material_method, MaterialDescriptorLoadResponse, MaterialLoadRequest,
    MaterialPreviewRefRequest, MaterialPreviewRefResponse, MaterialTextureRefInfo,
    MaterialTextureRefRequest, MaterialValidationRequest, MaterialValidationResult,
    MaterialsManifest, RenderMaterialPacket, ResolvedMaterialGraph,
    ENGINE_ASSETS_MATERIALS_SERVICE_ID, MATERIALS_BACKEND_CAPABILITY_ID, MATERIALS_SERVICE_ID,
    MATERIALS_SERVICE_METHODS,
};
use newengine_plugin_api::{Blob, HostApiV1};
use newengine_service_api::EngineServiceKind;
use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_empty_blob, ok_json,
    register_engine_gateway_provider_service_best_effort, EngineGatewayProviderDecl,
    JsonServiceRouter,
};
use serde::{Deserialize, Serialize};

use crate::MaterialAssetGatewayAdapter;

#[derive(Clone)]
struct MaterialGatewayState {
    adapter: MaterialAssetGatewayAdapter,
}

#[derive(Clone, Debug, Serialize)]
pub struct MaterialsServiceInfo {
    pub id: &'static str,
    pub gateway: &'static str,
    pub methods: &'static [&'static str],
    pub backend: &'static str,
    pub native_formats: &'static [&'static str],
    pub texture_reference_policy: &'static str,
}

impl MaterialGatewayState {
    fn new(adapter: MaterialAssetGatewayAdapter) -> Self {
        Self { adapter }
    }

    fn formats_json(&self) -> serde_json::Value {
        serde_json::json!({
            "schema": "newengine.assets.materials.formats.v1",
            "gateway": ENGINE_ASSETS_MATERIALS_SERVICE_ID,
            "formats": [
                {
                    "extension": "nemat",
                    "asset_kind": "material_library",
                    "container": "newengine.listfile.nef8.nemat",
                    "read_method": material_method::LOAD_DESCRIPTOR_V1,
                    "resolve_method": material_method::RESOLVE_GRAPH_V1,
                    "packet_method": material_method::TO_RENDER_PACKET_V1,
                    "runtime_ready": true,
                    "selector_syntax": "<logical-path>.nemat@entry",
                    "notes": "Native NewEngine XML material library inside NEF8/ListFile. Entries bind .ytd@entry texture references and resolve through engine.assets.materials."
                }
            ],
            "body_presentation": "xml",
            "texture_reference_policy": "material textures must be VFS .ytd@entry dictionary selectors; source image paths are importer-only"
        })
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
                    "materials.api: invalid invoke_json payload: {e}"
                )))
            }
        };

        match envelope.method.as_str() {
            material_method::LOAD_JSON_V1 | material_method::LOAD_DESCRIPTOR_V1 => {
                let request = match serde_json::from_value::<MaterialLoadRequest>(envelope.request)
                {
                    Ok(request) => request,
                    Err(e) => {
                        return RResult::RErr(RString::from(format!(
                            "materials.api: invalid load request: {e}"
                        )))
                    }
                };
                match self.adapter.load_descriptor(&request) {
                    Ok(value) => ok_json(value),
                    Err(e) => RResult::RErr(RString::from(e)),
                }
            }
            material_method::PREVIEW_REF_V1 => {
                let request =
                    match serde_json::from_value::<MaterialPreviewRefRequest>(envelope.request) {
                        Ok(request) => request,
                        Err(e) => {
                            return RResult::RErr(RString::from(format!(
                                "materials.api: invalid preview ref request: {e}"
                            )))
                        }
                    };
                match self.adapter.preview_material_ref(&request.logical_path) {
                    Ok(material_ref) => ok_json(MaterialPreviewRefResponse { material_ref }),
                    Err(e) => RResult::RErr(RString::from(e)),
                }
            }
            material_method::DESCRIBE_TEXTURE_REF_JSON_V1 => {
                let request =
                    match serde_json::from_value::<MaterialTextureRefRequest>(envelope.request) {
                        Ok(request) => request,
                        Err(e) => {
                            return RResult::RErr(RString::from(format!(
                                "materials.api: invalid texture ref request: {e}"
                            )))
                        }
                    };
                ok_json(self.adapter.describe_texture_ref(&request))
            }
            material_method::FORMATS_JSON_V1 | material_method::MANIFEST_JSON_V1 => {
                ok_json(MaterialsManifest::default())
            }
            material_method::RESOLVE_GRAPH_V1 => {
                let request = match serde_json::from_value::<MaterialLoadRequest>(envelope.request)
                {
                    Ok(request) => request,
                    Err(e) => {
                        return RResult::RErr(RString::from(format!(
                            "materials.api: invalid resolve graph request: {e}"
                        )))
                    }
                };
                match self.adapter.resolve_graph(&request) {
                    Ok(value) => ok_json(value),
                    Err(e) => RResult::RErr(RString::from(e)),
                }
            }
            material_method::VALIDATE_V1 => {
                let request =
                    match serde_json::from_value::<MaterialValidationRequest>(envelope.request) {
                        Ok(request) => request,
                        Err(e) => {
                            return RResult::RErr(RString::from(format!(
                                "materials.api: invalid validate request: {e}"
                            )))
                        }
                    };
                ok_json(self.adapter.validate_material(&request))
            }
            material_method::TO_RENDER_PACKET_V1 => {
                let request = match serde_json::from_value::<MaterialLoadRequest>(envelope.request)
                {
                    Ok(request) => request,
                    Err(e) => {
                        return RResult::RErr(RString::from(format!(
                            "materials.api: invalid render packet request: {e}"
                        )))
                    }
                };
                match self.adapter.to_render_packet(&request) {
                    Ok(value) => ok_json(value),
                    Err(e) => RResult::RErr(RString::from(e)),
                }
            }
            other => RResult::RErr(RString::from(format!(
                "materials.api: unknown invoke method '{other}'"
            ))),
        }
    }
}

pub fn materials_service_info() -> MaterialsServiceInfo {
    MaterialsServiceInfo {
        id: MATERIALS_SERVICE_ID,
        gateway: ENGINE_ASSETS_MATERIALS_SERVICE_ID,
        methods: MATERIALS_SERVICE_METHODS,
        backend: "engine.assets.starvault.materials-runtime",
        native_formats: &[".nemat"],
        texture_reference_policy:
            ".ytd@entry dictionary selectors only for authored/runtime material graphs",
    }
}

pub fn materials_gateway_service(
    client: AssetServiceClient,
) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    materials_gateway_service_with_host(client, None)
}

pub fn materials_gateway_service_with_host(
    client: AssetServiceClient,
    host: Option<HostApiV1>,
) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let description = engine_gateway_provider_service_description(
        MATERIALS_SERVICE_ID,
        "newengine-material-runtime.material-gateway",
        MATERIALS_BACKEND_CAPABILITY_ID,
        MATERIALS_SERVICE_METHODS.iter().copied(),
    )
    .gateway(ENGINE_ASSETS_MATERIALS_SERVICE_ID)
    .protocol("json")
    .features(["nemat-library", "nemat-entry-selectors", "ytd-texture-selectors", "render-material-packet"])
    .notes("Engine material gateway. Descriptors are read through engine.assets/VFS, resolved to material graphs, then converted to renderer-agnostic RenderMaterialPacket.");

    JsonServiceRouter::with_state(
        MATERIALS_SERVICE_ID,
        MaterialGatewayState::new(match host {
            Some(host) => MaterialAssetGatewayAdapter::with_client_and_host(client, host),
            None => MaterialAssetGatewayAdapter::with_client(client),
        }),
    )
    .describe_json(&description)
    .info(materials_service_info)
    .get_json(material_method::FORMATS_JSON_V1, |state| {
        state.formats_json()
    })
    .get_json(material_method::MANIFEST_JSON_V1, |_state| {
        MaterialsManifest::default()
    })
    .post_json_result::<MaterialLoadRequest, MaterialDescriptorLoadResponse, _>(
        material_method::LOAD_JSON_V1,
        |state, request| state.adapter.load_descriptor(&request),
    )
    .post_json_result::<MaterialLoadRequest, MaterialDescriptorLoadResponse, _>(
        material_method::LOAD_DESCRIPTOR_V1,
        |state, request| state.adapter.load_descriptor(&request),
    )
    .post_json_result::<MaterialLoadRequest, ResolvedMaterialGraph, _>(
        material_method::RESOLVE_GRAPH_V1,
        |state, request| state.adapter.resolve_graph(&request),
    )
    .post_json_result::<MaterialPreviewRefRequest, MaterialPreviewRefResponse, _>(
        material_method::PREVIEW_REF_V1,
        |state, request| {
            state
                .adapter
                .preview_material_ref(&request.logical_path)
                .map(|material_ref| MaterialPreviewRefResponse { material_ref })
        },
    )
    .post_json::<MaterialValidationRequest, MaterialValidationResult, _>(
        material_method::VALIDATE_V1,
        |state, request| state.adapter.validate_material(&request),
    )
    .post_json_result::<MaterialLoadRequest, RenderMaterialPacket, _>(
        material_method::TO_RENDER_PACKET_V1,
        |state, request| state.adapter.to_render_packet(&request),
    )
    .post_json::<MaterialTextureRefRequest, MaterialTextureRefInfo, _>(
        material_method::DESCRIBE_TEXTURE_REF_JSON_V1,
        |state, request| state.adapter.describe_texture_ref(&request),
    )
    .blob(material_method::INVOKE_JSON, |state, payload| {
        state.invoke_json(payload)
    })
    .blob(material_method::SHUTDOWN_V1, |_state, _payload| {
        ok_empty_blob()
    })
    .into_service_v1()
}

pub fn register_materials_gateway_best_effort(client: AssetServiceClient) -> bool {
    register_materials_gateway_best_effort_with_host(None, client)
}

pub fn register_materials_gateway_best_effort_with_host(
    host: Option<HostApiV1>,
    client: AssetServiceClient,
) -> bool {
    register_engine_gateway_provider_service_best_effort(EngineGatewayProviderDecl {
        gateway: ENGINE_ASSETS_MATERIALS_SERVICE_ID,
        service_kind: EngineServiceKind::Materials,
        provider_service: MATERIALS_SERVICE_ID,
        provider_route: "engine.assets.starvault.materials",
        capability: MATERIALS_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: "newengine-material-runtime.material-gateway",
        service: materials_gateway_service_with_host(client, host),
    })
}
