use std::sync::atomic::{AtomicBool, Ordering};

use newengine_plugin_api::{HostApiV1, MethodName};
use newengine_render_api::{
    decode_json, decode_texture_id_bin, encode_create_texture_bin, encode_json,
    encode_unit_command_batch_bin, RenderBackendInfo, RenderCommand, RenderCommandResponse,
    RenderServiceRequest, RenderServiceResponse, TextureDesc, TextureId, ENGINE_RENDER_SERVICE_ID,
    RENDER_SERVICE_METHOD_COMMAND_BATCH_BIN_V1, RENDER_SERVICE_METHOD_CREATE_TEXTURE_BIN_V1,
};

use newengine_runtime_adapter_core::GenericJsonServiceClient;

static TRY_BINARY_RENDER_BATCH: AtomicBool = AtomicBool::new(true);
static TRY_BINARY_CREATE_TEXTURE: AtomicBool = AtomicBool::new(true);

#[derive(Clone)]
pub(crate) struct RenderServiceClient {
    service: GenericJsonServiceClient,
    command_batch_bin_method: MethodName,
    create_texture_bin_method: MethodName,
}

impl RenderServiceClient {
    #[inline]
    pub(crate) fn new(host: HostApiV1) -> Self {
        Self {
            service: GenericJsonServiceClient::new(host, ENGINE_RENDER_SERVICE_ID),
            command_batch_bin_method: MethodName::from(RENDER_SERVICE_METHOD_COMMAND_BATCH_BIN_V1),
            create_texture_bin_method: MethodName::from(
                RENDER_SERVICE_METHOD_CREATE_TEXTURE_BIN_V1,
            ),
        }
    }

    #[inline]
    pub(crate) fn info(&self) -> Result<RenderBackendInfo, String> {
        let bytes = self.service.info_json()?;
        decode_json(&bytes)
    }

    #[inline]
    pub(crate) fn invoke(
        &self,
        req: RenderServiceRequest,
    ) -> Result<RenderServiceResponse, String> {
        let payload = encode_json(&req)?;
        let bytes = self.service.invoke_json(payload)?;
        decode_json(&bytes)
    }

    pub(crate) fn create_texture(&self, desc: TextureDesc) -> Result<TextureId, String> {
        if TRY_BINARY_CREATE_TEXTURE.load(Ordering::Relaxed) {
            match encode_create_texture_bin(&desc) {
                Ok(packet) => match self
                    .service
                    .call_raw_method(&self.create_texture_bin_method, packet)
                {
                    Ok(response) => return decode_texture_id_bin(&response),
                    Err(err) => {
                        let detail = err.to_string();
                        let unsupported = detail.contains("unsupported")
                            || detail.contains("unknown method")
                            || detail.contains("not found");
                        if unsupported {
                            if TRY_BINARY_CREATE_TEXTURE.swap(false, Ordering::Relaxed) {
                                newengine_ulog_api::ulog::debug!(
                                    "render service: binary create-texture disabled for this run; falling back to JSON err='{}'",
                                    detail
                                );
                            }
                        } else {
                            return Err(detail);
                        }
                    }
                },
                Err(err) => {
                    newengine_ulog_api::ulog::warn!(
                        "render service: binary create-texture encode failed; falling back to JSON err='{}'",
                        err
                    );
                }
            }
        }

        match self.command(RenderCommand::CreateTexture(desc))? {
            RenderCommandResponse::TextureId(id) => Ok(id),
            other => Err(format!(
                "render service protocol error: expected TextureId, got {:?}",
                other
            )),
        }
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
                match self
                    .service
                    .call_raw_method(&self.command_batch_bin_method, packet)
                {
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
