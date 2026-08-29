#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::RString;
use newengine_materials::{
    method as material_method, MaterialDescriptorLoadResponse, MaterialLoadRequest,
    MaterialPreviewRefRequest, MaterialPreviewRefResponse, MaterialTextureRefInfo,
    MaterialTextureRefRequest, MaterialValidationRequest, MaterialValidationResult,
    RenderMaterialPacket, ResolvedMaterialGraph, ENGINE_ASSETS_MATERIALS_SERVICE_ID,
};
use newengine_plugin_api::{Blob, HostApiV1, MethodName};

#[derive(Clone)]
pub struct MaterialGatewayClient {
    host: HostApiV1,
    service_id: RString,
}

impl MaterialGatewayClient {
    #[inline]
    pub fn new(host: HostApiV1) -> Self {
        Self {
            host,
            service_id: RString::from(ENGINE_ASSETS_MATERIALS_SERVICE_ID),
        }
    }

    #[inline]
    pub fn with_service_id(host: HostApiV1, service_id: &str) -> Self {
        Self {
            host,
            service_id: RString::from(service_id),
        }
    }

    pub fn preview_material_ref(&self, logical_path: &str) -> Result<String, String> {
        let response: MaterialPreviewRefResponse = self.call_json(
            material_method::PREVIEW_REF_V1,
            &MaterialPreviewRefRequest {
                logical_path: logical_path.to_owned(),
            },
        )?;
        Ok(response.material_ref)
    }

    pub fn load_descriptor(
        &self,
        request: &MaterialLoadRequest,
    ) -> Result<MaterialDescriptorLoadResponse, String> {
        self.call_json(material_method::LOAD_DESCRIPTOR_V1, request)
    }

    pub fn resolve_graph(
        &self,
        request: &MaterialLoadRequest,
    ) -> Result<ResolvedMaterialGraph, String> {
        self.call_json(material_method::RESOLVE_GRAPH_V1, request)
    }

    pub fn validate(
        &self,
        request: &MaterialValidationRequest,
    ) -> Result<MaterialValidationResult, String> {
        self.call_json(material_method::VALIDATE_V1, request)
    }

    pub fn to_render_packet(
        &self,
        request: &MaterialLoadRequest,
    ) -> Result<RenderMaterialPacket, String> {
        self.call_json(material_method::TO_RENDER_PACKET_V1, request)
    }

    pub fn describe_texture_ref(
        &self,
        request: &MaterialTextureRefRequest,
    ) -> Result<MaterialTextureRefInfo, String> {
        self.call_json(material_method::DESCRIBE_TEXTURE_REF_JSON_V1, request)
    }

    fn call_json<I, O>(&self, method_name: &str, request: &I) -> Result<O, String>
    where
        I: serde::Serialize,
        O: serde::de::DeserializeOwned,
    {
        let payload = serde_json::to_vec(request).map_err(|error| error.to_string())?;
        let bytes = (self.host.call_service_v1)(
            self.service_id.clone(),
            MethodName::from(method_name),
            Blob::from(payload),
        )
        .into_result()
        .map(|value| value.into_vec())
        .map_err(|error| error.to_string())?;
        serde_json::from_slice(&bytes).map_err(|error| {
            format!("engine.assets.materials method '{method_name}' returned invalid JSON: {error}")
        })
    }
}
