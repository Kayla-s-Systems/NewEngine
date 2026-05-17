use newengine_plugin_api::HostApiV1;
use newengine_render_api::{
    decode_json, encode_json, RenderBackendInfo, RenderCommand, RenderCommandResponse,
    RenderServiceRequest, RenderServiceResponse, ENGINE_RENDER_SERVICE_ID,
};

use crate::service_runtime::GenericJsonServiceClient;

#[derive(Clone)]
pub(crate) struct RenderServiceClient {
    service: GenericJsonServiceClient,
}

impl RenderServiceClient {
    #[inline]
    pub(crate) fn new(host: HostApiV1) -> Self {
        Self { service: GenericJsonServiceClient::new(host, ENGINE_RENDER_SERVICE_ID) }
    }

    #[inline]
    pub(crate) fn info(&self) -> Result<RenderBackendInfo, String> {
        let bytes = self.service.info_json()?;
        decode_json(&bytes)
    }

    #[inline]
    pub(crate) fn invoke(&self, req: RenderServiceRequest) -> Result<RenderServiceResponse, String> {
        let payload = encode_json(&req)?;
        let bytes = self.service.invoke_json(payload)?;
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
