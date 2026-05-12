use newengine_core::render::{
    BeginFrameDesc, BeginRenderTargetDesc, BindGroupDesc, BindGroupId, BindGroupLayoutDesc,
    BindGroupLayoutId, BufferDesc, BufferId, BufferSlice, DrawArgs, DrawIndexedArgs,
    IndexFormat, PipelineDesc, PipelineId, PipelineWarmupDesc, PipelineWarmupReport, RectI32, RenderApi,
    RenderDrawListKind, RenderFrameEnvelope, RenderGraphCompileReport, RenderGraphDesc, RenderGraphPassKind,
    RenderGraphSubmitReport, RenderGraphValidationReport, RenderTargetDesc, RenderDiagnosticsSnapshot,
    RenderTargetId, RenderWorkBudget, SamplerDesc,
    SamplerId, ShaderDesc, ShaderId, ShaderRuntimeCacheStats, TextureDesc, TextureId,
    TextureResidencySnapshot, UiDrawList, UiTexId, UploadPumpDesc, UploadPumpReport, Viewport,
};
use newengine_core::{EngineError, EngineResult};
use newengine_render_api::{RenderCommand, RenderCommandResponse, RenderServiceRequest, RenderServiceResponse};

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
    fn unit(&self, req: RenderCommand) -> EngineResult<()> {
        match self.client.command(req).map_err(EngineError::other)? {
            RenderCommandResponse::Unit => Ok(()),
            other => Err(EngineError::other(format!(
                "render service protocol error: expected unit response, got {:?}",
                other
            ))),
        }
    }

    #[inline]
    fn invoke_service(&self, req: RenderServiceRequest) -> EngineResult<RenderServiceResponse> {
        match self.client.invoke(req).map_err(EngineError::other)? {
            RenderServiceResponse::Problem(problem) => Err(EngineError::other(format!(
                "render service problem {} at {:?}: {}",
                problem.code, problem.phase, problem.detail
            ))),
            response => Ok(response),
        }
    }
}

impl RenderApi for ServiceBackedRenderApi {
    fn begin_frame(&mut self, desc: BeginFrameDesc) -> EngineResult<()> {
        self.unit(RenderCommand::BeginFrame(desc))
    }

    fn set_ui_draw_list(&mut self, ui: UiDrawList) {
        let _ = self.unit(RenderCommand::SetUiDrawList(ui));
    }

    fn set_debug_text(&mut self, text: String) {
        let _ = self.unit(RenderCommand::SetDebugText(text));
    }

    fn end_frame(&mut self) -> EngineResult<()> {
        self.unit(RenderCommand::EndFrame)
    }

    fn resize(&mut self, width: u32, height: u32) -> EngineResult<()> {
        self.unit(RenderCommand::Resize { width, height })
    }

    fn create_render_target(&mut self, desc: RenderTargetDesc) -> EngineResult<RenderTargetId> {
        match self
            .client
            .command(RenderCommand::CreateRenderTarget(desc))
            .map_err(EngineError::other)?
        {
            RenderCommandResponse::RenderTargetId(id) => Ok(id),
            other => Err(EngineError::other(format!(
                "render service protocol error: expected RenderTargetId, got {:?}",
                other
            ))),
        }
    }

    fn destroy_render_target(&mut self, id: RenderTargetId) {
        let _ = self.unit(RenderCommand::DestroyRenderTarget { id });
    }

    fn render_target_ui_tex_id(&self, id: RenderTargetId) -> EngineResult<UiTexId> {
        match self
            .client
            .command(RenderCommand::RenderTargetUiTexId { id })
            .map_err(EngineError::other)?
        {
            RenderCommandResponse::UiTexId(id) => Ok(id),
            other => Err(EngineError::other(format!(
                "render service protocol error: expected UiTexId, got {:?}",
                other
            ))),
        }
    }

    fn render_target_color_texture_id(&self, id: RenderTargetId) -> EngineResult<TextureId> {
        match self
            .client
            .command(RenderCommand::RenderTargetColorTextureId { id })
            .map_err(EngineError::other)?
        {
            RenderCommandResponse::TextureId(id) => Ok(id),
            other => Err(EngineError::other(format!(
                "render service protocol error: expected TextureId, got {:?}",
                other
            ))),
        }
    }

    fn begin_render_target(&mut self, desc: BeginRenderTargetDesc) -> EngineResult<()> {
        self.unit(RenderCommand::BeginRenderTarget(desc))
    }

    fn end_render_target(&mut self) -> EngineResult<()> {
        self.unit(RenderCommand::EndRenderTarget)
    }

    fn create_buffer(&mut self, desc: BufferDesc) -> EngineResult<BufferId> {
        match self
            .client
            .command(RenderCommand::CreateBuffer(desc))
            .map_err(EngineError::other)?
        {
            RenderCommandResponse::BufferId(id) => Ok(id),
            other => Err(EngineError::other(format!(
                "render service protocol error: expected BufferId, got {:?}",
                other
            ))),
        }
    }

    fn destroy_buffer(&mut self, id: BufferId) {
        let _ = self.unit(RenderCommand::DestroyBuffer { id });
    }

    fn write_buffer(&mut self, id: BufferId, offset: u64, data: &[u8]) -> EngineResult<()> {
        self.unit(RenderCommand::WriteBuffer {
            id,
            offset,
            data: data.to_vec(),
        })
    }

    fn create_texture(&mut self, desc: TextureDesc) -> EngineResult<TextureId> {
        match self
            .client
            .command(RenderCommand::CreateTexture(desc))
            .map_err(EngineError::other)?
        {
            RenderCommandResponse::TextureId(id) => Ok(id),
            other => Err(EngineError::other(format!(
                "render service protocol error: expected TextureId, got {:?}",
                other
            ))),
        }
    }

    fn destroy_texture(&mut self, id: TextureId) {
        let _ = self.unit(RenderCommand::DestroyTexture { id });
    }

    fn create_sampler(&mut self, desc: SamplerDesc) -> EngineResult<SamplerId> {
        match self
            .client
            .command(RenderCommand::CreateSampler(desc))
            .map_err(EngineError::other)?
        {
            RenderCommandResponse::SamplerId(id) => Ok(id),
            other => Err(EngineError::other(format!(
                "render service protocol error: expected SamplerId, got {:?}",
                other
            ))),
        }
    }

    fn destroy_sampler(&mut self, id: SamplerId) {
        let _ = self.unit(RenderCommand::DestroySampler { id });
    }

    fn create_shader(&mut self, desc: ShaderDesc) -> EngineResult<ShaderId> {
        match self
            .client
            .command(RenderCommand::CreateShader(desc))
            .map_err(EngineError::other)?
        {
            RenderCommandResponse::ShaderId(id) => Ok(id),
            other => Err(EngineError::other(format!(
                "render service protocol error: expected ShaderId, got {:?}",
                other
            ))),
        }
    }

    fn destroy_shader(&mut self, id: ShaderId) {
        let _ = self.unit(RenderCommand::DestroyShader { id });
    }

    fn create_pipeline(&mut self, desc: PipelineDesc) -> EngineResult<PipelineId> {
        match self
            .client
            .command(RenderCommand::CreatePipeline(desc))
            .map_err(EngineError::other)?
        {
            RenderCommandResponse::PipelineId(id) => Ok(id),
            other => Err(EngineError::other(format!(
                "render service protocol error: expected PipelineId, got {:?}",
                other
            ))),
        }
    }

    fn destroy_pipeline(&mut self, id: PipelineId) {
        let _ = self.unit(RenderCommand::DestroyPipeline { id });
    }

    fn create_bind_group_layout(
        &mut self,
        desc: BindGroupLayoutDesc,
    ) -> EngineResult<BindGroupLayoutId> {
        match self
            .client
            .command(RenderCommand::CreateBindGroupLayout(desc))
            .map_err(EngineError::other)?
        {
            RenderCommandResponse::BindGroupLayoutId(id) => Ok(id),
            other => Err(EngineError::other(format!(
                "render service protocol error: expected BindGroupLayoutId, got {:?}",
                other
            ))),
        }
    }

    fn destroy_bind_group_layout(&mut self, id: BindGroupLayoutId) {
        let _ = self.unit(RenderCommand::DestroyBindGroupLayout { id });
    }

    fn create_bind_group(&mut self, desc: BindGroupDesc) -> EngineResult<BindGroupId> {
        match self
            .client
            .command(RenderCommand::CreateBindGroup(desc))
            .map_err(EngineError::other)?
        {
            RenderCommandResponse::BindGroupId(id) => Ok(id),
            other => Err(EngineError::other(format!(
                "render service protocol error: expected BindGroupId, got {:?}",
                other
            ))),
        }
    }

    fn destroy_bind_group(&mut self, id: BindGroupId) {
        let _ = self.unit(RenderCommand::DestroyBindGroup { id });
    }

    fn set_viewport(&mut self, vp: Viewport) -> EngineResult<()> {
        self.unit(RenderCommand::SetViewport(vp))
    }

    fn set_scissor(&mut self, rect: RectI32) -> EngineResult<()> {
        self.unit(RenderCommand::SetScissor(rect))
    }

    fn set_pipeline(&mut self, pipeline: PipelineId) -> EngineResult<()> {
        self.unit(RenderCommand::SetPipeline { pipeline })
    }

    fn set_bind_group(&mut self, index: u32, group: BindGroupId) -> EngineResult<()> {
        self.unit(RenderCommand::SetBindGroup { index, group })
    }

    fn set_vertex_buffer(&mut self, slot: u32, slice: BufferSlice) -> EngineResult<()> {
        self.unit(RenderCommand::SetVertexBuffer { slot, slice })
    }

    fn set_index_buffer(&mut self, slice: BufferSlice, format: IndexFormat) -> EngineResult<()> {
        self.unit(RenderCommand::SetIndexBuffer { slice, format })
    }

    fn draw(&mut self, args: DrawArgs) -> EngineResult<()> {
        self.unit(RenderCommand::Draw(args))
    }

    fn draw_indexed(&mut self, args: DrawIndexedArgs) -> EngineResult<()> {
        self.unit(RenderCommand::DrawIndexed(args))
    }

    fn set_render_phase(&mut self, phase: Option<RenderGraphPassKind>) -> EngineResult<()> {
        match self.invoke_service(RenderServiceRequest::SetRenderPhase { phase })? {
            RenderServiceResponse::Unit => Ok(()),
            other => Err(EngineError::other(format!(
                "render service protocol error: expected Unit, got {:?}",
                other
            ))),
        }
    }


    fn set_draw_list_kind(&mut self, kind: Option<RenderDrawListKind>) -> EngineResult<()> {
        match self.invoke_service(RenderServiceRequest::SetDrawListKind { kind })? {
            RenderServiceResponse::Unit => Ok(()),
            other => Err(EngineError::other(format!(
                "render service protocol error: expected Unit, got {:?}",
                other
            ))),
        }
    }

    fn discard_recorded_commands(&mut self) -> EngineResult<()> {
        match self.invoke_service(RenderServiceRequest::DiscardRecordedCommands)? {
            RenderServiceResponse::Unit => Ok(()),
            other => Err(EngineError::other(format!(
                "render service protocol error: expected Unit, got {:?}",
                other
            ))),
        }
    }

    fn compile_render_graph(
        &mut self,
        graph: RenderGraphDesc,
    ) -> EngineResult<RenderGraphCompileReport> {
        match self.invoke_service(RenderServiceRequest::CompileRenderGraph(graph))? {
            RenderServiceResponse::GraphCompileReport(report) => Ok(report),
            other => Err(EngineError::other(format!(
                "render service protocol error: expected GraphCompileReport, got {:?}",
                other
            ))),
        }
    }

    fn validate_render_graph(
        &mut self,
        graph: RenderGraphDesc,
    ) -> EngineResult<RenderGraphValidationReport> {
        match self.invoke_service(RenderServiceRequest::ValidateRenderGraph(graph))? {
            RenderServiceResponse::GraphValidationReport(report) => Ok(report),
            other => Err(EngineError::other(format!(
                "render service protocol error: expected GraphValidationReport, got {:?}",
                other
            ))),
        }
    }

    fn submit_render_graph(
        &mut self,
        graph: RenderGraphDesc,
    ) -> EngineResult<RenderGraphSubmitReport> {
        match self.invoke_service(RenderServiceRequest::SubmitRenderGraph(graph))? {
            RenderServiceResponse::GraphSubmitReport(report) => Ok(report),
            other => Err(EngineError::other(format!(
                "render service protocol error: expected GraphSubmitReport, got {:?}",
                other
            ))),
        }
    }

    fn submit_frame(&mut self, frame: RenderFrameEnvelope) -> EngineResult<RenderGraphSubmitReport> {
        match self.invoke_service(RenderServiceRequest::SubmitFrame(frame))? {
            RenderServiceResponse::GraphSubmitReport(report) => Ok(report),
            other => Err(EngineError::other(format!(
                "render service protocol error: expected GraphSubmitReport, got {:?}",
                other
            ))),
        }
    }

    fn set_work_budget(&mut self, budget: RenderWorkBudget) -> EngineResult<()> {
        match self.invoke_service(RenderServiceRequest::SetWorkBudget(budget))? {
            RenderServiceResponse::Unit => Ok(()),
            other => Err(EngineError::other(format!(
                "render service protocol error: expected Unit, got {:?}",
                other
            ))),
        }
    }


    fn pump_uploads(&mut self, desc: UploadPumpDesc) -> EngineResult<UploadPumpReport> {
        match self.invoke_service(RenderServiceRequest::PumpUploads(desc))? {
            RenderServiceResponse::UploadPumpReport(report) => Ok(report),
            other => Err(EngineError::other(format!(
                "render service protocol error: expected UploadPumpReport, got {:?}",
                other
            ))),
        }
    }

    fn texture_residency(&self, id: TextureId) -> EngineResult<TextureResidencySnapshot> {
        match self
            .client
            .command(RenderCommand::TextureResidency { id })
            .map_err(EngineError::other)?
        {
            RenderCommandResponse::TextureResidency(snapshot) => Ok(snapshot),
            other => Err(EngineError::other(format!(
                "render service protocol error: expected TextureResidency, got {:?}",
                other
            ))),
        }
    }

    fn warmup_pipelines(&mut self, desc: PipelineWarmupDesc) -> EngineResult<PipelineWarmupReport> {
        match self
            .client
            .command(RenderCommand::WarmupPipelines(desc))
            .map_err(EngineError::other)?
        {
            RenderCommandResponse::PipelineWarmupReport(report) => Ok(report),
            other => Err(EngineError::other(format!(
                "render service protocol error: expected PipelineWarmupReport, got {:?}",
                other
            ))),
        }
    }

    fn shader_cache_stats(&self) -> EngineResult<ShaderRuntimeCacheStats> {
        match self
            .client
            .command(RenderCommand::ShaderCacheStats)
            .map_err(EngineError::other)?
        {
            RenderCommandResponse::ShaderCacheStats(stats) => Ok(stats),
            other => Err(EngineError::other(format!(
                "render service protocol error: expected ShaderCacheStats, got {:?}",
                other
            ))),
        }
    }

    fn diagnostics_snapshot(&self) -> EngineResult<RenderDiagnosticsSnapshot> {
        match self.invoke_service(RenderServiceRequest::DiagnosticsSnapshot)? {
            RenderServiceResponse::DiagnosticsSnapshot(snapshot) => Ok(snapshot),
            other => Err(EngineError::other(format!(
                "render service protocol error: expected DiagnosticsSnapshot, got {:?}",
                other
            ))),
        }
    }
}
