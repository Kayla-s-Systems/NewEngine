use abi_stable::std_types::RString;
use newengine_plugin_api::{Blob, HostApiV1, MethodName};
use newengine_render_api::{
    decode_json, encode_json, RenderBackendInfo, RenderCommand, RenderCommandResponse,
    RenderServiceRequest, RenderServiceResponse, RENDER_SERVICE_ID, RENDER_SERVICE_METHOD_INFO,
    RENDER_SERVICE_METHOD_INVOKE,
};

#[derive(Clone)]
pub(crate) struct RenderServiceClient {
    host: HostApiV1,
    service_id: RString,
}

impl RenderServiceClient {
    #[inline]
    pub(crate) fn new(host: HostApiV1) -> Self {
        Self {
            host,
            service_id: RString::from(RENDER_SERVICE_ID),
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
    pub(crate) fn info(&self) -> Result<RenderBackendInfo, String> {
        let bytes = self.call(MethodName::from(RENDER_SERVICE_METHOD_INFO), Vec::new())?;
        decode_json(&bytes)
    }

    #[inline]
    pub(crate) fn invoke(&self, req: RenderServiceRequest) -> Result<RenderServiceResponse, String> {
        let payload = encode_json(&req)?;
        let bytes = self.call(MethodName::from(RENDER_SERVICE_METHOD_INVOKE), payload)?;
        decode_json(&bytes)
    }

    #[inline]
    pub(crate) fn command(&self, req: RenderCommand) -> Result<RenderCommandResponse, String> {
        match self.invoke(RenderServiceRequest::Command(req))? {
            RenderServiceResponse::Command(response) => Ok(response),
            RenderServiceResponse::Problem(problem) => Err(format!(
                "render service problem {}: {} ({})",
                problem.code, problem.title, problem.detail
            )),
            other => Err(format!(
                "render service protocol error: expected Command response, got {:?}",
                other
            )),
        }
    }
}
