#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::PathBuf;

use abi_stable::std_types::RString;
use newengine_core::render::{
    BeginFrameDesc, BeginRenderTargetDesc, BindGroupDesc, BindGroupId, BindGroupLayoutDesc,
    BindGroupLayoutId, BufferDesc, BufferId, BufferSlice, DrawArgs, DrawIndexedArgs,
    IndexFormat, PipelineDesc, PipelineId, RectI32, RenderApi, RenderApiRef, RenderTargetDesc,
    RenderTargetId, SamplerDesc, SamplerId, ShaderDesc, ShaderId, TextureDesc, TextureId,
    UiDrawList, UiTexId, Viewport, RENDER_API_ID, RENDER_API_PROVIDE,
};
use newengine_core::{EngineError, EngineResult, Module, ModuleCtx};
use newengine_plugin_api::{Blob, HostApiV1, MethodName};
use newengine_render_api::{
    decode_json, encode_json, RenderBackendInfoV1, RenderRequestV1, RenderResponseV1,
    RENDER_SERVICE_ID, RENDER_SERVICE_METHOD_INFO_V1, RENDER_SERVICE_METHOD_INVOKE_V1,
};

pub const DEFAULT_RENDER_BACKEND_ID: &str = "newengine.renderer.vulkan";
pub const NULL_RENDER_BACKEND_ID: &str = "newengine.renderer.null";
pub const DEFAULT_RENDER_BACKEND_CLEAR_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

#[derive(Debug, Clone)]
pub struct ResolvedRenderBackendConfig {
    pub backend_id: String,
    pub clear_color: [f32; 4],
    pub debug_text: String,
}

#[derive(Clone)]
struct RenderServiceClient {
    host: HostApiV1,
    service_id: RString,
    m_invoke: MethodName,
    m_info: MethodName,
}

impl RenderServiceClient {
    #[inline]
    fn new(host: HostApiV1) -> Self {
        Self {
            host,
            service_id: RString::from(RENDER_SERVICE_ID),
            m_invoke: MethodName::from(RENDER_SERVICE_METHOD_INVOKE_V1),
            m_info: MethodName::from(RENDER_SERVICE_METHOD_INFO_V1),
        }
    }

    #[inline]
    fn call(&self, method_name: MethodName, payload: Vec<u8>) -> Result<Vec<u8>, String> {
        (self.host.call_service_v1)(self.service_id.clone(), method_name, Blob::from(payload))
            .into_result()
            .map(|v| v.into_vec())
            .map_err(|e| e.to_string())
    }

    #[inline]
    fn info(&self) -> Result<RenderBackendInfoV1, String> {
        let bytes = self.call(self.m_info.clone(), Vec::new())?;
        decode_json(&bytes)
    }

    #[inline]
    fn invoke(&self, req: RenderRequestV1) -> Result<RenderResponseV1, String> {
        let payload = encode_json(&req)?;
        let bytes = self.call(self.m_invoke.clone(), payload)?;
        decode_json(&bytes)
    }
}

struct ServiceBackedRenderApi {
    client: RenderServiceClient,
}

impl ServiceBackedRenderApi {
    #[inline]
    fn new(client: RenderServiceClient) -> Self {
        Self { client }
    }

    #[inline]
    fn unit(&self, req: RenderRequestV1) -> EngineResult<()> {
        match self.client.invoke(req).map_err(EngineError::other)? {
            RenderResponseV1::Unit => Ok(()),
            other => Err(EngineError::other(format!(
                "render service protocol error: expected unit response, got {:?}",
                other
            ))),
        }
    }
}

impl RenderApi for ServiceBackedRenderApi {
    fn begin_frame(&mut self, desc: BeginFrameDesc) -> EngineResult<()> {
        self.unit(RenderRequestV1::BeginFrame(desc))
    }

    fn set_ui_draw_list(&mut self, ui: UiDrawList) {
        let _ = self.unit(RenderRequestV1::SetUiDrawList(ui));
    }

    fn end_frame(&mut self) -> EngineResult<()> {
        self.unit(RenderRequestV1::EndFrame)
    }

    fn resize(&mut self, width: u32, height: u32) -> EngineResult<()> {
        self.unit(RenderRequestV1::Resize { width, height })
    }

    fn create_render_target(&mut self, desc: RenderTargetDesc) -> EngineResult<RenderTargetId> {
        match self
            .client
            .invoke(RenderRequestV1::CreateRenderTarget(desc))
            .map_err(EngineError::other)?
        {
            RenderResponseV1::RenderTargetId(id) => Ok(id),
            other => Err(EngineError::other(format!(
                "render service protocol error: expected RenderTargetId, got {:?}",
                other
            ))),
        }
    }

    fn destroy_render_target(&mut self, id: RenderTargetId) {
        let _ = self.unit(RenderRequestV1::DestroyRenderTarget { id });
    }

    fn render_target_ui_tex_id(&self, id: RenderTargetId) -> EngineResult<UiTexId> {
        match self
            .client
            .invoke(RenderRequestV1::RenderTargetUiTexId { id })
            .map_err(EngineError::other)?
        {
            RenderResponseV1::UiTexId(id) => Ok(id),
            other => Err(EngineError::other(format!(
                "render service protocol error: expected UiTexId, got {:?}",
                other
            ))),
        }
    }

    fn begin_render_target(&mut self, desc: BeginRenderTargetDesc) -> EngineResult<()> {
        self.unit(RenderRequestV1::BeginRenderTarget(desc))
    }

    fn end_render_target(&mut self) -> EngineResult<()> {
        self.unit(RenderRequestV1::EndRenderTarget)
    }

    fn create_buffer(&mut self, desc: BufferDesc) -> EngineResult<BufferId> {
        match self
            .client
            .invoke(RenderRequestV1::CreateBuffer(desc))
            .map_err(EngineError::other)?
        {
            RenderResponseV1::BufferId(id) => Ok(id),
            other => Err(EngineError::other(format!(
                "render service protocol error: expected BufferId, got {:?}",
                other
            ))),
        }
    }

    fn destroy_buffer(&mut self, id: BufferId) {
        let _ = self.unit(RenderRequestV1::DestroyBuffer { id });
    }

    fn write_buffer(&mut self, id: BufferId, offset: u64, data: &[u8]) -> EngineResult<()> {
        self.unit(RenderRequestV1::WriteBuffer {
            id,
            offset,
            data: data.to_vec(),
        })
    }

    fn create_texture(&mut self, desc: TextureDesc) -> EngineResult<TextureId> {
        match self
            .client
            .invoke(RenderRequestV1::CreateTexture(desc))
            .map_err(EngineError::other)?
        {
            RenderResponseV1::TextureId(id) => Ok(id),
            other => Err(EngineError::other(format!(
                "render service protocol error: expected TextureId, got {:?}",
                other
            ))),
        }
    }

    fn destroy_texture(&mut self, id: TextureId) {
        let _ = self.unit(RenderRequestV1::DestroyTexture { id });
    }

    fn create_sampler(&mut self, desc: SamplerDesc) -> EngineResult<SamplerId> {
        match self
            .client
            .invoke(RenderRequestV1::CreateSampler(desc))
            .map_err(EngineError::other)?
        {
            RenderResponseV1::SamplerId(id) => Ok(id),
            other => Err(EngineError::other(format!(
                "render service protocol error: expected SamplerId, got {:?}",
                other
            ))),
        }
    }

    fn destroy_sampler(&mut self, id: SamplerId) {
        let _ = self.unit(RenderRequestV1::DestroySampler { id });
    }

    fn create_shader(&mut self, desc: ShaderDesc) -> EngineResult<ShaderId> {
        match self
            .client
            .invoke(RenderRequestV1::CreateShader(desc))
            .map_err(EngineError::other)?
        {
            RenderResponseV1::ShaderId(id) => Ok(id),
            other => Err(EngineError::other(format!(
                "render service protocol error: expected ShaderId, got {:?}",
                other
            ))),
        }
    }

    fn destroy_shader(&mut self, id: ShaderId) {
        let _ = self.unit(RenderRequestV1::DestroyShader { id });
    }

    fn create_pipeline(&mut self, desc: PipelineDesc) -> EngineResult<PipelineId> {
        match self
            .client
            .invoke(RenderRequestV1::CreatePipeline(desc))
            .map_err(EngineError::other)?
        {
            RenderResponseV1::PipelineId(id) => Ok(id),
            other => Err(EngineError::other(format!(
                "render service protocol error: expected PipelineId, got {:?}",
                other
            ))),
        }
    }

    fn destroy_pipeline(&mut self, id: PipelineId) {
        let _ = self.unit(RenderRequestV1::DestroyPipeline { id });
    }

    fn create_bind_group_layout(
        &mut self,
        desc: BindGroupLayoutDesc,
    ) -> EngineResult<BindGroupLayoutId> {
        match self
            .client
            .invoke(RenderRequestV1::CreateBindGroupLayout(desc))
            .map_err(EngineError::other)?
        {
            RenderResponseV1::BindGroupLayoutId(id) => Ok(id),
            other => Err(EngineError::other(format!(
                "render service protocol error: expected BindGroupLayoutId, got {:?}",
                other
            ))),
        }
    }

    fn destroy_bind_group_layout(&mut self, id: BindGroupLayoutId) {
        let _ = self.unit(RenderRequestV1::DestroyBindGroupLayout { id });
    }

    fn create_bind_group(&mut self, desc: BindGroupDesc) -> EngineResult<BindGroupId> {
        match self
            .client
            .invoke(RenderRequestV1::CreateBindGroup(desc))
            .map_err(EngineError::other)?
        {
            RenderResponseV1::BindGroupId(id) => Ok(id),
            other => Err(EngineError::other(format!(
                "render service protocol error: expected BindGroupId, got {:?}",
                other
            ))),
        }
    }

    fn destroy_bind_group(&mut self, id: BindGroupId) {
        let _ = self.unit(RenderRequestV1::DestroyBindGroup { id });
    }

    fn set_viewport(&mut self, vp: Viewport) -> EngineResult<()> {
        self.unit(RenderRequestV1::SetViewport(vp))
    }

    fn set_scissor(&mut self, rect: RectI32) -> EngineResult<()> {
        self.unit(RenderRequestV1::SetScissor(rect))
    }

    fn set_pipeline(&mut self, pipeline: PipelineId) -> EngineResult<()> {
        self.unit(RenderRequestV1::SetPipeline { pipeline })
    }

    fn set_bind_group(&mut self, index: u32, group: BindGroupId) -> EngineResult<()> {
        self.unit(RenderRequestV1::SetBindGroup { index, group })
    }

    fn set_vertex_buffer(&mut self, slot: u32, slice: BufferSlice) -> EngineResult<()> {
        self.unit(RenderRequestV1::SetVertexBuffer { slot, slice })
    }

    fn set_index_buffer(&mut self, slice: BufferSlice, format: IndexFormat) -> EngineResult<()> {
        self.unit(RenderRequestV1::SetIndexBuffer { slice, format })
    }

    fn draw(&mut self, args: DrawArgs) -> EngineResult<()> {
        self.unit(RenderRequestV1::Draw(args))
    }

    fn draw_indexed(&mut self, args: DrawIndexedArgs) -> EngineResult<()> {
        self.unit(RenderRequestV1::DrawIndexed(args))
    }
}

#[derive(Debug, Default)]
struct NullRenderApi {
    next_id: u32,
}

impl NullRenderApi {
    #[inline]
    fn alloc_id(&mut self) -> u32 {
        self.next_id = self.next_id.saturating_add(1).max(1);
        self.next_id
    }
}

impl RenderApi for NullRenderApi {
    fn begin_frame(&mut self, _desc: BeginFrameDesc) -> EngineResult<()> { Ok(()) }
    fn set_ui_draw_list(&mut self, _ui: UiDrawList) {}
    fn end_frame(&mut self) -> EngineResult<()> { Ok(()) }
    fn resize(&mut self, _width: u32, _height: u32) -> EngineResult<()> { Ok(()) }
    fn create_render_target(&mut self, _desc: RenderTargetDesc) -> EngineResult<RenderTargetId> { Ok(RenderTargetId::new(self.alloc_id())) }
    fn destroy_render_target(&mut self, _id: RenderTargetId) {}
    fn render_target_ui_tex_id(&self, _id: RenderTargetId) -> EngineResult<UiTexId> { Ok(UiTexId::new(0)) }
    fn begin_render_target(&mut self, _desc: BeginRenderTargetDesc) -> EngineResult<()> { Ok(()) }
    fn end_render_target(&mut self) -> EngineResult<()> { Ok(()) }
    fn create_buffer(&mut self, _desc: BufferDesc) -> EngineResult<BufferId> { Ok(BufferId::new(self.alloc_id())) }
    fn destroy_buffer(&mut self, _id: BufferId) {}
    fn write_buffer(&mut self, _id: BufferId, _offset: u64, _data: &[u8]) -> EngineResult<()> { Ok(()) }
    fn create_texture(&mut self, _desc: TextureDesc) -> EngineResult<TextureId> { Err(EngineError::other("null render backend: textures are not supported in headless mode")) }
    fn destroy_texture(&mut self, _id: TextureId) {}
    fn create_sampler(&mut self, _desc: SamplerDesc) -> EngineResult<SamplerId> { Err(EngineError::other("null render backend: samplers are not supported in headless mode")) }
    fn destroy_sampler(&mut self, _id: SamplerId) {}
    fn create_shader(&mut self, _desc: ShaderDesc) -> EngineResult<ShaderId> { Ok(ShaderId::new(self.alloc_id())) }
    fn destroy_shader(&mut self, _id: ShaderId) {}
    fn create_pipeline(&mut self, _desc: PipelineDesc) -> EngineResult<PipelineId> { Ok(PipelineId::new(self.alloc_id())) }
    fn destroy_pipeline(&mut self, _id: PipelineId) {}
    fn create_bind_group_layout(&mut self, _desc: BindGroupLayoutDesc) -> EngineResult<BindGroupLayoutId> { Ok(BindGroupLayoutId::new(self.alloc_id())) }
    fn destroy_bind_group_layout(&mut self, _id: BindGroupLayoutId) {}
    fn create_bind_group(&mut self, _desc: BindGroupDesc) -> EngineResult<BindGroupId> { Ok(BindGroupId::new(self.alloc_id())) }
    fn destroy_bind_group(&mut self, _id: BindGroupId) {}
    fn set_viewport(&mut self, _vp: Viewport) -> EngineResult<()> { Ok(()) }
    fn set_scissor(&mut self, _rect: RectI32) -> EngineResult<()> { Ok(()) }
    fn set_pipeline(&mut self, _pipeline: PipelineId) -> EngineResult<()> { Ok(()) }
    fn set_bind_group(&mut self, _index: u32, _group: BindGroupId) -> EngineResult<()> { Ok(()) }
    fn set_vertex_buffer(&mut self, _slot: u32, _slice: BufferSlice) -> EngineResult<()> { Ok(()) }
    fn set_index_buffer(&mut self, _slice: BufferSlice, _format: IndexFormat) -> EngineResult<()> { Ok(()) }
    fn draw(&mut self, _args: DrawArgs) -> EngineResult<()> { Ok(()) }
    fn draw_indexed(&mut self, _args: DrawIndexedArgs) -> EngineResult<()> { Ok(()) }
}

pub struct RenderBackendRuntimeModule {
    backend_spec: String,
    _modules_dir: PathBuf,
    api: Option<RenderApiRef>,
}

impl RenderBackendRuntimeModule {
    #[inline]
    pub fn new(backend_spec: String, modules_dir: PathBuf) -> Self {
        Self {
            backend_spec,
            _modules_dir: modules_dir,
            api: None,
        }
    }

    fn enable_null_render_backend<E: Send + 'static>(
        &mut self,
        ctx: &mut ModuleCtx<'_, E>,
        reason: impl Into<String>,
    ) -> EngineResult<()> {
        let reason = reason.into();
        log::warn!(
            "render backend: '{}' is unavailable; enabling headless null backend ({})",
            self.backend_spec,
            reason
        );

        let api = RenderApiRef::new(NullRenderApi::default());
        let resolved = ResolvedRenderBackendConfig {
            backend_id: NULL_RENDER_BACKEND_ID.to_owned(),
            clear_color: DEFAULT_RENDER_BACKEND_CLEAR_COLOR,
            debug_text: "NewEngine | Headless".to_owned(),
        };

        ctx.resources_mut().insert(resolved);
        ctx.resources_mut().register_api(RENDER_API_ID, api.clone())?;
        self.api = Some(api);
        Ok(())
    }
}

impl<E: Send + 'static> Module<E> for RenderBackendRuntimeModule {
    fn id(&self) -> &'static str {
        "render.runtime.loader"
    }

    fn provides(&self) -> &'static [newengine_core::ApiProvide] {
        &[RENDER_API_PROVIDE]
    }

    fn init(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        let host = newengine_plugin_host::default_host_api();
        let client = RenderServiceClient::new(host);
        let info = match client.info() {
            Ok(info) => info,
            Err(err) => return self.enable_null_render_backend(ctx, err),
        };

        if !backend_matches(&self.backend_spec, &info.backend_id) {
            return self.enable_null_render_backend(
                ctx,
                format!(
                    "selected backend '{}' does not match active plugin '{}'",
                    self.backend_spec, info.backend_id
                ),
            );
        }

        log::info!(
            "render backend: bridge bound id='{}' name='{}' version='{}' debug_text='{}'",
            info.backend_id,
            info.backend_name,
            info.backend_version,
            info.debug_text
        );

        let resolved = ResolvedRenderBackendConfig {
            backend_id: info.backend_id,
            clear_color: info.clear_color,
            debug_text: info.debug_text,
        };
        let api = RenderApiRef::new(ServiceBackedRenderApi::new(client));
        ctx.resources_mut().insert(resolved);
        ctx.resources_mut().register_api(RENDER_API_ID, api.clone())?;
        self.api = Some(api);
        Ok(())
    }

    fn shutdown(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        let _ = ctx.resources_mut().unregister_api::<RenderApiRef>(RENDER_API_ID);
        let _ = ctx.resources_mut().remove::<ResolvedRenderBackendConfig>();
        self.api = None;
        Ok(())
    }
}

fn backend_matches(spec: &str, active_id: &str) -> bool {
    let spec = normalize_backend_token(spec);
    let active = normalize_backend_token(active_id);
    spec.is_empty() || active.is_empty() || spec == active || active.contains(&spec) || spec.contains(&active)
}

fn normalize_backend_token(input: &str) -> String {
    input
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch.to_ascii_lowercase() } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
