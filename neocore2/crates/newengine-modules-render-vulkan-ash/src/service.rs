#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::{Arc, Mutex};

use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::{Blob, CapabilityId, MethodName, ServiceV1};
use newengine_render_api::{
    decode_json, encode_json, RenderBackendInfoV1, RenderRequestV1, RenderResponseV1,
    RENDER_SERVICE_ID, RENDER_SERVICE_METHOD_INFO_V1, RENDER_SERVICE_METHOD_INVOKE_V1,
};

use crate::render_api::VulkanRenderApi;

#[derive(Clone)]
pub struct VulkanRenderService {
    api: Arc<Mutex<VulkanRenderApi>>,
    info: RenderBackendInfoV1,
}

impl VulkanRenderService {
    #[inline]
    pub fn new(api: VulkanRenderApi, info: RenderBackendInfoV1) -> Self {
        Self {
            api: Arc::new(Mutex::new(api)),
            info,
        }
    }

    #[inline]
    fn ok_json<T: serde::Serialize>(value: &T) -> RResult<Blob, RString> {
        match encode_json(value) {
            Ok(bytes) => RResult::ROk(Blob::from(bytes)),
            Err(e) => RResult::RErr(RString::from(e)),
        }
    }

    #[inline]
    fn with_api<T>(
        &self,
        f: impl FnOnce(&mut VulkanRenderApi) -> Result<T, String>,
    ) -> RResult<T, RString> {
        let mut guard = match self.api.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        match f(&mut guard) {
            Ok(v) => RResult::ROk(v),
            Err(e) => RResult::RErr(RString::from(e)),
        }
    }

    fn invoke(&self, req: RenderRequestV1) -> RResult<RenderResponseV1, RString> {
        self.with_api(|api| match req {
            RenderRequestV1::BeginFrame(desc) => {
                api.begin_frame(desc)?;
                Ok(RenderResponseV1::Unit)
            }
            RenderRequestV1::SetUiDrawList(ui) => {
                api.set_ui_draw_list(ui);
                Ok(RenderResponseV1::Unit)
            }
            RenderRequestV1::EndFrame => {
                api.end_frame()?;
                Ok(RenderResponseV1::Unit)
            }
            RenderRequestV1::Resize { width, height } => {
                api.resize(width, height)?;
                Ok(RenderResponseV1::Unit)
            }
            RenderRequestV1::CreateRenderTarget(desc) => {
                Ok(RenderResponseV1::RenderTargetId(api.create_render_target(desc)?))
            }
            RenderRequestV1::DestroyRenderTarget { id } => {
                api.destroy_render_target(id);
                Ok(RenderResponseV1::Unit)
            }
            RenderRequestV1::RenderTargetUiTexId { id } => {
                Ok(RenderResponseV1::UiTexId(api.render_target_ui_tex_id(id)?))
            }
            RenderRequestV1::BeginRenderTarget(desc) => {
                api.begin_render_target(desc)?;
                Ok(RenderResponseV1::Unit)
            }
            RenderRequestV1::EndRenderTarget => {
                api.end_render_target()?;
                Ok(RenderResponseV1::Unit)
            }
            RenderRequestV1::CreateBuffer(desc) => {
                Ok(RenderResponseV1::BufferId(api.create_buffer(desc)?))
            }
            RenderRequestV1::DestroyBuffer { id } => {
                api.destroy_buffer(id);
                Ok(RenderResponseV1::Unit)
            }
            RenderRequestV1::WriteBuffer { id, offset, data } => {
                api.write_buffer(id, offset, &data)?;
                Ok(RenderResponseV1::Unit)
            }
            RenderRequestV1::CreateTexture(desc) => {
                Ok(RenderResponseV1::TextureId(api.create_texture(desc)?))
            }
            RenderRequestV1::DestroyTexture { id } => {
                api.destroy_texture(id);
                Ok(RenderResponseV1::Unit)
            }
            RenderRequestV1::CreateSampler(desc) => {
                Ok(RenderResponseV1::SamplerId(api.create_sampler(desc)?))
            }
            RenderRequestV1::DestroySampler { id } => {
                api.destroy_sampler(id);
                Ok(RenderResponseV1::Unit)
            }
            RenderRequestV1::CreateShader(desc) => {
                Ok(RenderResponseV1::ShaderId(api.create_shader(desc)?))
            }
            RenderRequestV1::DestroyShader { id } => {
                api.destroy_shader(id);
                Ok(RenderResponseV1::Unit)
            }
            RenderRequestV1::CreatePipeline(desc) => {
                Ok(RenderResponseV1::PipelineId(api.create_pipeline(desc)?))
            }
            RenderRequestV1::DestroyPipeline { id } => {
                api.destroy_pipeline(id);
                Ok(RenderResponseV1::Unit)
            }
            RenderRequestV1::CreateBindGroupLayout(desc) => Ok(RenderResponseV1::BindGroupLayoutId(
                api.create_bind_group_layout(desc)?,
            )),
            RenderRequestV1::DestroyBindGroupLayout { id } => {
                api.destroy_bind_group_layout(id);
                Ok(RenderResponseV1::Unit)
            }
            RenderRequestV1::CreateBindGroup(desc) => {
                Ok(RenderResponseV1::BindGroupId(api.create_bind_group(desc)?))
            }
            RenderRequestV1::DestroyBindGroup { id } => {
                api.destroy_bind_group(id);
                Ok(RenderResponseV1::Unit)
            }
            RenderRequestV1::SetViewport(vp) => {
                api.set_viewport(vp)?;
                Ok(RenderResponseV1::Unit)
            }
            RenderRequestV1::SetScissor(rect) => {
                api.set_scissor(rect)?;
                Ok(RenderResponseV1::Unit)
            }
            RenderRequestV1::SetPipeline { pipeline } => {
                api.set_pipeline(pipeline)?;
                Ok(RenderResponseV1::Unit)
            }
            RenderRequestV1::SetBindGroup { index, group } => {
                api.set_bind_group(index, group)?;
                Ok(RenderResponseV1::Unit)
            }
            RenderRequestV1::SetVertexBuffer { slot, slice } => {
                api.set_vertex_buffer(slot, slice)?;
                Ok(RenderResponseV1::Unit)
            }
            RenderRequestV1::SetIndexBuffer { slice, format } => {
                api.set_index_buffer(slice, format)?;
                Ok(RenderResponseV1::Unit)
            }
            RenderRequestV1::Draw(args) => {
                api.draw(args)?;
                Ok(RenderResponseV1::Unit)
            }
            RenderRequestV1::DrawIndexed(args) => {
                api.draw_indexed(args)?;
                Ok(RenderResponseV1::Unit)
            }
        })
    }
}

impl ServiceV1 for VulkanRenderService {
    fn id(&self) -> CapabilityId {
        CapabilityId::from(RENDER_SERVICE_ID)
    }

    fn describe(&self) -> RString {
        let v = serde_json::json!({
            "id": RENDER_SERVICE_ID,
            "version": 1,
            "methods": [RENDER_SERVICE_METHOD_INFO_V1, RENDER_SERVICE_METHOD_INVOKE_V1],
            "backend_id": self.info.backend_id,
            "backend_name": self.info.backend_name,
            "backend_version": self.info.backend_version,
        });
        RString::from(serde_json::to_string(&v).unwrap_or_else(|_| "{}".to_string()))
    }

    fn call(&self, method: MethodName, payload: Blob) -> RResult<Blob, RString> {
        match method.as_str() {
            RENDER_SERVICE_METHOD_INFO_V1 => Self::ok_json(&self.info),
            RENDER_SERVICE_METHOD_INVOKE_V1 => {
                let req: RenderRequestV1 = match decode_json(payload.as_slice()) {
                    Ok(v) => v,
                    Err(e) => return RResult::RErr(RString::from(e)),
                };
                match self.invoke(req) {
                    RResult::ROk(resp) => Self::ok_json(&resp),
                    RResult::RErr(err) => RResult::RErr(err),
                }
            }
            m => RResult::RErr(RString::from(format!("render service: unknown method '{}'", m))),
        }
    }
}
