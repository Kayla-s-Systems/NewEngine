use newengine_core::render::{
    BeginFrameDesc, BeginRenderTargetDesc, BindGroupDesc, BindGroupId, BindGroupLayoutDesc,
    BindGroupLayoutId, BufferDesc, BufferId, BufferSlice, DrawArgs, DrawIndexedArgs,
    IndexFormat, PipelineDesc, PipelineId, PipelineWarmupDesc, PipelineWarmupReport, RectI32, RenderApi,
    RenderTargetDesc, RenderDiagnosticsSnapshot, RenderTargetId, RenderWorkBudget, SamplerDesc,
    SamplerId, ShaderDesc, ShaderId, ShaderRuntimeCacheStats, TextureDesc, TextureId,
    TextureResidencySnapshot, UiDrawList, UiTexId, UploadPumpDesc, UploadPumpReport, Viewport,
};
use newengine_core::{EngineError, EngineResult};
use newengine_render_api::{RenderRequestV1, RenderResponseV1};

use crate::render_runtime::client::RenderServiceClient;

pub(crate) struct ServiceBackedRenderApi {
    client: RenderServiceClient,
}

impl ServiceBackedRenderApi {
    #[inline]
    pub(crate) fn new(client: RenderServiceClient) -> Self {
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

    fn set_debug_text(&mut self, _text: String) {
        // Compatibility no-op. Runtime UI must go through UI providers; renderer
        // text is renderer-owned dev mark only.
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

    fn render_target_color_texture_id(&self, id: RenderTargetId) -> EngineResult<TextureId> {
        match self
            .client
            .invoke(RenderRequestV1::RenderTargetColorTextureId { id })
            .map_err(EngineError::other)?
        {
            RenderResponseV1::TextureId(id) => Ok(id),
            other => Err(EngineError::other(format!(
                "render service protocol error: expected TextureId, got {:?}",
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

    fn set_work_budget(&mut self, budget: RenderWorkBudget) -> EngineResult<()> {
        self.unit(RenderRequestV1::SetWorkBudget(budget))
    }


    fn pump_uploads(&mut self, desc: UploadPumpDesc) -> EngineResult<UploadPumpReport> {
        match self
            .client
            .invoke(RenderRequestV1::PumpUploads(desc))
            .map_err(EngineError::other)?
        {
            RenderResponseV1::UploadPumpReport(report) => Ok(report),
            other => Err(EngineError::other(format!(
                "render service protocol error: expected UploadPumpReport, got {:?}",
                other
            ))),
        }
    }

    fn texture_residency(&self, id: TextureId) -> EngineResult<TextureResidencySnapshot> {
        match self
            .client
            .invoke(RenderRequestV1::TextureResidency { id })
            .map_err(EngineError::other)?
        {
            RenderResponseV1::TextureResidency(snapshot) => Ok(snapshot),
            other => Err(EngineError::other(format!(
                "render service protocol error: expected TextureResidency, got {:?}",
                other
            ))),
        }
    }

    fn warmup_pipelines(&mut self, desc: PipelineWarmupDesc) -> EngineResult<PipelineWarmupReport> {
        match self
            .client
            .invoke(RenderRequestV1::WarmupPipelines(desc))
            .map_err(EngineError::other)?
        {
            RenderResponseV1::PipelineWarmupReport(report) => Ok(report),
            other => Err(EngineError::other(format!(
                "render service protocol error: expected PipelineWarmupReport, got {:?}",
                other
            ))),
        }
    }

    fn shader_cache_stats(&self) -> EngineResult<ShaderRuntimeCacheStats> {
        match self
            .client
            .invoke(RenderRequestV1::ShaderCacheStats)
            .map_err(EngineError::other)?
        {
            RenderResponseV1::ShaderCacheStats(stats) => Ok(stats),
            other => Err(EngineError::other(format!(
                "render service protocol error: expected ShaderCacheStats, got {:?}",
                other
            ))),
        }
    }

    fn diagnostics_snapshot(&self) -> EngineResult<RenderDiagnosticsSnapshot> {
        match self
            .client
            .invoke(RenderRequestV1::DiagnosticsSnapshot)
            .map_err(EngineError::other)?
        {
            RenderResponseV1::DiagnosticsSnapshot(snapshot) => Ok(snapshot),
            other => Err(EngineError::other(format!(
                "render service protocol error: expected DiagnosticsSnapshot, got {:?}",
                other
            ))),
        }
    }
}