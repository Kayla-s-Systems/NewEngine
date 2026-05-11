use abi_stable::std_types::RString;
use newengine_plugin_api::{Blob, HostApiV1, MethodName};
use newengine_render_api::{
    decode_json, encode_json, RenderBackendInfoV1, RenderBackendInfoV2, RenderRequestV1,
    RenderRequestV2, RenderResponseV1, RenderResponseV2, RENDER_SERVICE_ID,
    RENDER_SERVICE_METHOD_INFO_V1, RENDER_SERVICE_METHOD_INFO_V2,
    RENDER_SERVICE_METHOD_INVOKE_V1, RENDER_SERVICE_METHOD_INVOKE_V2,
};

#[derive(Clone)]
pub(crate) struct RenderServiceClient {
    host: HostApiV1,
    service_id: RString,
    m_invoke: MethodName,
    m_info: MethodName,
    m_invoke_v2: MethodName,
    m_info_v2: MethodName,
}

impl RenderServiceClient {
    #[inline]
    pub(crate) fn new(host: HostApiV1) -> Self {
        Self {
            host,
            service_id: RString::from(RENDER_SERVICE_ID),
            m_invoke: MethodName::from(RENDER_SERVICE_METHOD_INVOKE_V1),
            m_info: MethodName::from(RENDER_SERVICE_METHOD_INFO_V1),
            m_invoke_v2: MethodName::from(RENDER_SERVICE_METHOD_INVOKE_V2),
            m_info_v2: MethodName::from(RENDER_SERVICE_METHOD_INFO_V2),
        }
    }

    #[inline]
    fn call(&self, method_name: MethodName, payload: Vec<u8>) -> Result<Vec<u8>, String> {
        (self.host.call_service_v1)(self.service_id.clone(), method_name, Blob::from(payload))
            .into_result()
            .map(|value| value.into_vec())
            .map_err(|err| err.to_string())
    }

    #[inline]
    pub(crate) fn info(&self) -> Result<RenderBackendInfoV1, String> {
        let bytes = self.call(self.m_info.clone(), Vec::new())?;
        decode_json(&bytes)
    }

    #[inline]
    pub(crate) fn invoke(&self, req: RenderRequestV1) -> Result<RenderResponseV1, String> {
        let payload = encode_json(&req)?;
        let bytes = self.call(self.m_invoke.clone(), payload)?;
        decode_json(&bytes)
    }

    #[inline]
    pub(crate) fn info_v2(&self) -> Result<RenderBackendInfoV2, String> {
        let bytes = self.call(self.m_info_v2.clone(), Vec::new())?;
        decode_json(&bytes)
    }

    #[inline]
    pub(crate) fn invoke_v2(&self, req: RenderRequestV2) -> Result<RenderResponseV2, String> {
        let payload = encode_json(&req)?;
        let bytes = self.call(self.m_invoke_v2.clone(), payload)?;
        decode_json(&bytes)
    }
}