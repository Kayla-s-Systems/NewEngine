use crate::{
    BeginFrameDesc, BeginRenderTargetDesc, BindGroupDesc, BindGroupId, BindGroupLayoutDesc,
    BindGroupLayoutId, BufferDesc, BufferId, BufferSlice, DrawArgs, DrawIndexedArgs,
    IndexFormat, PipelineDesc, PipelineId, PipelineWarmupDesc, PipelineWarmupReport, RectI32,
    RenderBackendCapabilities, RenderDiagnosticsSnapshot, RenderTargetDesc, RenderTargetId,
    RenderWorkBudget, SamplerDesc, SamplerId, ShaderDesc, ShaderId, ShaderRuntimeCacheStats,
    TextureDesc, TextureId, TextureResidencySnapshot, UiDrawList, UiTexId, UploadPumpDesc,
    UploadPumpReport, Viewport,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderBackendInfoV1 {
    pub backend_id: String,
    pub backend_name: String,
    pub backend_version: String,
    pub debug_text: String,
    pub clear_color: [f32; 4],
    #[serde(default)]
    pub capabilities: RenderBackendCapabilities,
    #[serde(default)]
    pub work_budget: RenderWorkBudget,
}

impl RenderBackendInfoV1 {
    #[inline]
    pub fn with_capabilities(mut self, capabilities: RenderBackendCapabilities) -> Self {
        self.capabilities = capabilities;
        self
    }

    #[inline]
    pub fn with_work_budget(mut self, budget: RenderWorkBudget) -> Self {
        self.work_budget = budget;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RenderRequestV1 {
    BeginFrame(BeginFrameDesc),
    SetUiDrawList(UiDrawList),
    EndFrame,
    Resize { width: u32, height: u32 },
    CreateRenderTarget(RenderTargetDesc),
    DestroyRenderTarget { id: RenderTargetId },
    RenderTargetUiTexId { id: RenderTargetId },
    RenderTargetColorTextureId { id: RenderTargetId },
    BeginRenderTarget(BeginRenderTargetDesc),
    EndRenderTarget,
    CreateBuffer(BufferDesc),
    DestroyBuffer { id: BufferId },
    WriteBuffer { id: BufferId, offset: u64, data: Vec<u8> },
    CreateTexture(TextureDesc),
    DestroyTexture { id: TextureId },
    CreateSampler(SamplerDesc),
    DestroySampler { id: SamplerId },
    CreateShader(ShaderDesc),
    DestroyShader { id: ShaderId },
    CreatePipeline(PipelineDesc),
    DestroyPipeline { id: PipelineId },
    CreateBindGroupLayout(BindGroupLayoutDesc),
    DestroyBindGroupLayout { id: BindGroupLayoutId },
    CreateBindGroup(BindGroupDesc),
    DestroyBindGroup { id: BindGroupId },
    SetViewport(Viewport),
    SetScissor(RectI32),
    SetPipeline { pipeline: PipelineId },
    SetBindGroup { index: u32, group: BindGroupId },
    SetVertexBuffer { slot: u32, slice: BufferSlice },
    SetIndexBuffer { slice: BufferSlice, format: IndexFormat },
    Draw(DrawArgs),
    DrawIndexed(DrawIndexedArgs),
    SetWorkBudget(RenderWorkBudget),
    PumpUploads(UploadPumpDesc),
    TextureResidency { id: TextureId },
    WarmupPipelines(PipelineWarmupDesc),
    ShaderCacheStats,
    DiagnosticsSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RenderResponseV1 {
    Unit,
    RenderTargetId(RenderTargetId),
    UiTexId(UiTexId),
    BufferId(BufferId),
    TextureId(TextureId),
    SamplerId(SamplerId),
    ShaderId(ShaderId),
    PipelineId(PipelineId),
    BindGroupLayoutId(BindGroupLayoutId),
    BindGroupId(BindGroupId),
    UploadPumpReport(UploadPumpReport),
    TextureResidency(TextureResidencySnapshot),
    PipelineWarmupReport(PipelineWarmupReport),
    ShaderCacheStats(ShaderRuntimeCacheStats),
    DiagnosticsSnapshot(RenderDiagnosticsSnapshot),
}

#[inline]
pub fn encode_json<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    serde_json::to_vec(value).map_err(|e| e.to_string())
}

#[inline]
pub fn decode_json<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<T, String> {
    serde_json::from_slice(bytes).map_err(|e| e.to_string())
}
