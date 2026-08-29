#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::RString;
use newengine_model_domain_api::{
    DrawableDictionaryManifest, DrawableDictionaryRequest, FoliageImportRequestV1,
    FoliageImportResponseV1, ModelAssetBundle, ModelAssetRequest, ModelConstructionValidation,
    ENGINE_ASSETS_MODELS_SERVICE_ID, MODEL_SERVICE_METHOD_ASSEMBLE_JSON_V1,
    MODEL_SERVICE_METHOD_DRAWABLE_DICTIONARY_MANIFEST_JSON_V1,
    MODEL_SERVICE_METHOD_IMPORT_FOLIAGE_V1, MODEL_SERVICE_METHOD_RESOLVE_DRAWABLE_V1,
    MODEL_SERVICE_METHOD_VALIDATE_JSON_V1,
};
use newengine_plugin_api::{Blob, HostApiV1, MethodName};

#[derive(Clone)]
pub struct ModelGatewayClient {
    host: HostApiV1,
    service_id: RString,
}

impl ModelGatewayClient {
    #[inline]
    pub fn new(host: HostApiV1) -> Self {
        Self {
            host,
            service_id: RString::from(ENGINE_ASSETS_MODELS_SERVICE_ID),
        }
    }

    #[inline]
    pub fn with_service_id(host: HostApiV1, service_id: &str) -> Self {
        Self {
            host,
            service_id: RString::from(service_id),
        }
    }

    pub fn assemble_bundle(&self, request: &ModelAssetRequest) -> Result<ModelAssetBundle, String> {
        self.call_json(MODEL_SERVICE_METHOD_ASSEMBLE_JSON_V1, request)
    }

    pub fn validate_request(
        &self,
        request: &ModelAssetRequest,
    ) -> Result<ModelConstructionValidation, String> {
        self.call_json(MODEL_SERVICE_METHOD_VALIDATE_JSON_V1, request)
    }

    pub fn drawable_dictionary_manifest(
        &self,
        request: &DrawableDictionaryRequest,
    ) -> Result<DrawableDictionaryManifest, String> {
        self.call_json(
            MODEL_SERVICE_METHOD_DRAWABLE_DICTIONARY_MANIFEST_JSON_V1,
            request,
        )
    }

    pub fn resolve_drawable(
        &self,
        request: &DrawableDictionaryRequest,
    ) -> Result<DrawableDictionaryManifest, String> {
        self.call_json(MODEL_SERVICE_METHOD_RESOLVE_DRAWABLE_V1, request)
    }

    pub fn import_foliage(
        &self,
        request: &FoliageImportRequestV1,
    ) -> Result<FoliageImportResponseV1, String> {
        self.call_json(MODEL_SERVICE_METHOD_IMPORT_FOLIAGE_V1, request)
    }

    fn call_json<I, O>(&self, method_name: &str, request: &I) -> Result<O, String>
    where
        I: serde::Serialize,
        O: serde::de::DeserializeOwned,
    {
        let payload = serde_json::to_vec(request).map_err(|error| error.to_string())?;
        let bytes = self.call_raw(method_name, payload)?;
        serde_json::from_slice::<O>(&bytes).map_err(|error| {
            format!("engine.assets.models method '{method_name}' returned invalid JSON: {error}")
        })
    }

    fn call_raw(&self, method_name: &str, payload: Vec<u8>) -> Result<Vec<u8>, String> {
        (self.host.call_service_v1)(
            self.service_id.clone(),
            MethodName::from(method_name),
            Blob::from(payload),
        )
        .into_result()
        .map(|value| value.into_vec())
        .map_err(|error| error.to_string())
    }
}
