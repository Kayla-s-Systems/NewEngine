use std::sync::atomic::{AtomicBool, Ordering};

use newengine_plugin_api::HostApiV1;
use newengine_render_api::{
    decode_json, encode_json, encode_unit_command_batch_bin, RenderBackendInfo,
    RenderCommand, RenderCommandResponse, RenderServiceRequest, RenderServiceResponse,
    ENGINE_RENDER_SERVICE_ID, RENDER_SERVICE_METHOD_COMMAND_BATCH_BIN_V1,
};

use crate::service_runtime::GenericJsonServiceClient;

static TRY_BINARY_RENDER_BATCH: AtomicBool = AtomicBool::new(true);

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

    pub(crate) fn command_batch(
        &self,
        reqs: Vec<RenderCommand>,
    ) -> Result<Vec<RenderCommandResponse>, String> {
        if reqs.is_empty() {
            return Ok(Vec::new());
        }

        let binary_len = reqs.len();
        if TRY_BINARY_RENDER_BATCH.load(Ordering::Relaxed) {
            if let Ok(packet) = encode_unit_command_batch_bin(&reqs) {
                match self.service.call_raw(RENDER_SERVICE_METHOD_COMMAND_BATCH_BIN_V1, packet) {
                    Ok(_) => return Ok(vec![RenderCommandResponse::Unit; binary_len]),
                    Err(err) => {
                        let detail = err.to_string();
                        if detail.contains("unknown render command batch binary tag")
                            || detail.contains("unsupported")
                            || detail.contains("unknown method")
                            || detail.contains("not found")
                        {
                            if TRY_BINARY_RENDER_BATCH.swap(false, Ordering::Relaxed) {
                                newengine_ulog_api::ulog::debug!(
                                    "render service: binary command batch disabled for this run; falling back to invoke_json err='{}'",
                                    detail
                                );
                            }
                        } else {
                            newengine_ulog_api::ulog::debug!(
                                "render service: binary command batch failed transiently; falling back to invoke_json err='{}'",
                                detail
                            );
                        }
                    }
                }
            }
        }

        match self.invoke(RenderServiceRequest::CommandBatch(reqs))? {
            RenderServiceResponse::CommandBatch(responses) => Ok(responses),
            RenderServiceResponse::Problem(problem) => Err(format!(
                "render service problem {}: {} ({})",
                problem.code, problem.title, problem.detail
            )),
            other => Err(format!(
                "render service protocol error: expected CommandBatch response, got {:?}",
                other
            )),
        }
    }
}
