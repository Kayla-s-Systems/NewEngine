use crate::{
    BeginFrameDesc, BeginRenderTargetDesc, BindGroupDesc, BindGroupId, BindGroupLayoutDesc,
    BindGroupLayoutId, BufferDesc, BufferId, BufferSlice, Color4, DrawArgs, DrawIndexedArgs,
    Extent2D, IndexFormat, PipelineDesc, PipelineId, PipelineWarmupDesc, PipelineWarmupReport, PostFxFrameParams,
    RectI32, RenderBackendCapabilities, RenderDiagnosticsSnapshot, RenderDrawListKind, RenderEffectStack,
    RenderFeature, RenderGraphCompileReport, RenderGraphDesc, RenderGraphPassKind,
    RenderGraphSubmitReport, RenderGraphValidationReport, RenderTargetDesc, RenderTargetId,
    RenderWorkBudget, SamplerDesc, SamplerId, ShaderDesc, ShaderId, ShaderRuntimeCacheStats,
    TextureDesc, TextureId, TextureResidencySnapshot, UiDrawList, UiTexId, UploadPumpDesc,
    UploadPumpReport, Viewport,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderBackendInfo {
    pub backend_id: String,
    pub backend_name: String,
    pub backend_version: String,
    pub debug_text: String,
    pub clear_color: Color4,
    #[serde(default)]
    pub capabilities: RenderBackendCapabilities,
    #[serde(default)]
    pub work_budget: RenderWorkBudget,
    #[serde(default)]
    pub protocol_version: RenderApiVersion,
}

impl RenderBackendInfo {
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

/// Imperative render device command used inside the stable service protocol.
/// This is the resource/draw command vocabulary shared by runtime, render graph
/// replay and backends.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RenderCommand {
    BeginFrame(BeginFrameDesc),
    SetUiDrawList(UiDrawList),
    SetDebugText(String),
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
pub enum RenderCommandResponse {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RenderApiVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl RenderApiVersion {
    #[inline]
    pub const fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self { major, minor, patch }
    }
}

impl Default for RenderApiVersion {
    #[inline]
    fn default() -> Self {
        Self::new(1, 0, 0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderCapabilityNegotiationRequest {
    pub preferred_version: RenderApiVersion,
    #[serde(default)]
    pub required_features: Vec<RenderFeature>,
    #[serde(default)]
    pub optional_features: Vec<RenderFeature>,
}

impl Default for RenderCapabilityNegotiationRequest {
    #[inline]
    fn default() -> Self {
        Self {
            preferred_version: RenderApiVersion::default(),
            required_features: Vec::new(),
            optional_features: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderProtocolNotice {
    pub code: String,
    pub message: String,
}

impl RenderProtocolNotice {
    #[inline]
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderCapabilityNegotiationResponse {
    pub accepted_version: RenderApiVersion,
    pub backend_version: RenderApiVersion,
    pub ok: bool,
    pub enabled_features: Vec<RenderFeature>,
    pub missing_required_features: Vec<RenderFeature>,
    #[serde(default)]
    pub notices: Vec<RenderProtocolNotice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderProblemDetails {
    pub code: String,
    pub title: String,
    pub detail: String,
    pub backend: Option<String>,
    pub phase: Option<String>,
    #[serde(default)]
    pub recoverable: bool,
}

impl RenderProblemDetails {
    #[inline]
    pub fn new(
        code: impl Into<String>,
        title: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            title: title.into(),
            detail: detail.into(),
            backend: None,
            phase: None,
            recoverable: true,
        }
    }

    #[inline]
    pub fn with_backend(mut self, backend: impl Into<String>) -> Self {
        self.backend = Some(backend.into());
        self
    }

    #[inline]
    pub fn with_phase(mut self, phase: impl Into<String>) -> Self {
        self.phase = Some(phase.into());
        self
    }

    #[inline]
    pub fn fatal(mut self) -> Self {
        self.recoverable = false;
        self
    }
}

/// One renderer-facing frame package inspired by mature phase/draw-list
/// renderers: the runtime submits a single envelope containing the graph,
/// declared draw-list routes and frame extents instead of negotiating scattered
/// per-version service calls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderFrameEnvelope {
    pub frame_index: u64,
    pub label: Option<String>,
    pub clear_color: Color4,
    pub surface_extent: Extent2D,
    pub viewport_extent: Extent2D,
    pub viewport_is_surface: bool,
    #[serde(default)]
    pub postfx: PostFxFrameParams,
    #[serde(default)]
    pub effects: RenderEffectStack,
    pub graph: RenderGraphDesc,
    #[serde(default)]
    pub draw_lists: Vec<RenderDrawListKind>,
    #[serde(default)]
    pub work_budget: Option<RenderWorkBudget>,
}

impl RenderFrameEnvelope {
    #[inline]
    pub fn new(
        frame_index: u64,
        clear_color: Color4,
        surface_extent: Extent2D,
        viewport_extent: Extent2D,
        viewport_is_surface: bool,
        graph: RenderGraphDesc,
    ) -> Self {
        Self {
            frame_index,
            label: graph.label.clone(),
            clear_color,
            surface_extent,
            viewport_extent,
            viewport_is_surface,
            postfx: PostFxFrameParams::default(),
            effects: RenderEffectStack::default(),
            graph,
            draw_lists: Vec::new(),
            work_budget: None,
        }
    }


    #[inline]
    pub fn with_postfx(mut self, postfx: PostFxFrameParams) -> Self {
        self.postfx = postfx;
        self
    }

    #[inline]
    pub fn with_effect_stack(mut self, effects: RenderEffectStack) -> Self {
        self.effects = effects;
        self
    }

    #[inline]
    pub fn with_draw_lists(mut self, draw_lists: impl IntoIterator<Item = RenderDrawListKind>) -> Self {
        self.draw_lists = draw_lists.into_iter().collect();
        self
    }

    #[inline]
    pub fn with_work_budget(mut self, budget: RenderWorkBudget) -> Self {
        self.work_budget = Some(budget);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RenderServiceRequest {
    Negotiate(RenderCapabilityNegotiationRequest),
    Command(RenderCommand),
    /// Executes a sequence of unit render commands in one provider call.
    ///
    /// This keeps the engine-facing API imperative for feature extractors while
    /// avoiding one service-boundary roundtrip per recorded draw command on the
    /// frame hot path. Commands that return ids/snapshots should still use
    /// `Command` so the caller can consume the typed response immediately.
    CommandBatch(Vec<RenderCommand>),
    CompileRenderGraph(RenderGraphDesc),
    ValidateRenderGraph(RenderGraphDesc),
    SetRenderPhase { phase: Option<RenderGraphPassKind> },
    SetDrawListKind { kind: Option<RenderDrawListKind> },
    DiscardRecordedCommands,
    SubmitRenderGraph(RenderGraphDesc),
    SubmitFrame(RenderFrameEnvelope),
    SetWorkBudget(RenderWorkBudget),
    PumpUploads(UploadPumpDesc),
    DiagnosticsSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RenderServiceResponse {
    Unit,
    Negotiation(RenderCapabilityNegotiationResponse),
    Command(RenderCommandResponse),
    CommandBatch(Vec<RenderCommandResponse>),
    GraphCompileReport(RenderGraphCompileReport),
    GraphValidationReport(RenderGraphValidationReport),
    GraphSubmitReport(RenderGraphSubmitReport),
    UploadPumpReport(UploadPumpReport),
    DiagnosticsSnapshot(RenderDiagnosticsSnapshot),
    Problem(RenderProblemDetails),
}
