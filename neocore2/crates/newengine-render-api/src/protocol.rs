use crate::{
    BeginFrameDesc, BeginRenderTargetDesc, BindGroupDesc, BindGroupId, BindGroupLayoutDesc,
    BindGroupLayoutId, BufferDesc, BufferId, BufferSlice, DrawArgs, DrawIndexedArgs,
    IndexFormat, PipelineDesc, PipelineId, PipelineWarmupDesc, PipelineWarmupReport, RectI32,
    RenderBackendCapabilities, RenderDiagnosticsSnapshot, RenderFeature, RenderTargetDesc,
    RenderTargetId, RenderWorkBudget, SamplerDesc, SamplerId, ShaderDesc, ShaderId,
    ShaderRuntimeCacheStats, TextureDesc, TextureId, TextureResidencySnapshot, UiDrawList,
    UiTexId, UploadPumpDesc, UploadPumpReport, Viewport,
};
use crate::render_graph::{
    RenderDrawListKind, RenderGraphCompileReport, RenderGraphDesc, RenderGraphPassKind, RenderGraphSubmitReport,
    RenderGraphValidationReport,
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

/// Current V3 immediate-command compatibility payload. It intentionally has a
/// separate semantic name from the legacy V1 wire endpoint: V3 clients should
/// send this through `invoke_json_v3`, while direct `invoke_json_v1` remains
/// legacy-only.
pub type RenderImmediateRequest = RenderRequestV1;
pub type RenderImmediateResponse = RenderResponseV1;

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
        Self::new(3, 0, 0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderProtocolStatus {
    Current,
    Legacy,
}

impl Default for RenderProtocolStatus {
    #[inline]
    fn default() -> Self {
        Self::Current
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderLegacyWarning {
    pub protocol: RenderApiVersion,
    pub status: RenderProtocolStatus,
    pub code: String,
    pub message: String,
    pub migration_target: RenderApiVersion,
}

impl RenderLegacyWarning {
    #[inline]
    pub fn new(protocol: RenderApiVersion) -> Self {
        Self {
            protocol,
            status: RenderProtocolStatus::Legacy,
            code: format!("render.protocol.v{}.legacy", protocol.major),
            message: format!(
                "Render API V{} is a legacy compatibility protocol; migrate runtime/backend calls to Render API V3.",
                protocol.major
            ),
            migration_target: RenderApiVersion::new(3, 0, 0),
        }
    }
}

#[inline]
pub fn render_legacy_protocol_warning(version: RenderApiVersion) -> RenderLegacyWarning {
    RenderLegacyWarning::new(version)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderBackendInfoV3 {
    pub protocol_version: RenderApiVersion,
    pub backend: RenderBackendInfoV1,
    #[serde(default)]
    pub protocol_status: RenderProtocolStatus,
    #[serde(default)]
    pub legacy_warnings: Vec<RenderLegacyWarning>,
}

impl RenderBackendInfoV3 {
    #[inline]
    pub fn from_v1(backend: RenderBackendInfoV1) -> Self {
        Self {
            protocol_version: RenderApiVersion::new(3, 0, 0),
            backend,
            protocol_status: RenderProtocolStatus::Current,
            legacy_warnings: vec![
                RenderLegacyWarning::new(RenderApiVersion::new(1, 0, 0)),
                RenderLegacyWarning::new(RenderApiVersion::new(2, 0, 0)),
            ],
        }
    }
}

pub type RenderBackendInfoV2 = RenderBackendInfoV3;

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
pub struct RenderCapabilityNegotiationResponse {
    pub accepted_version: RenderApiVersion,
    pub backend_version: RenderApiVersion,
    pub ok: bool,
    pub enabled_features: Vec<RenderFeature>,
    pub missing_required_features: Vec<RenderFeature>,
    #[serde(default)]
    pub warnings: Vec<RenderLegacyWarning>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RenderRequestV3 {
    Negotiate(RenderCapabilityNegotiationRequest),
    CompileRenderGraph(RenderGraphDesc),
    ValidateRenderGraph(RenderGraphDesc),
    SetRenderPhase { phase: Option<RenderGraphPassKind> },
    SetDrawListKind { kind: Option<RenderDrawListKind> },
    DiscardRecordedCommands,
    SubmitRenderGraph(RenderGraphDesc),
    SetWorkBudget(RenderWorkBudget),
    PumpUploads(UploadPumpDesc),
    DiagnosticsSnapshot,
    Immediate(RenderImmediateRequest),
    V1(RenderRequestV1),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RenderResponseV3 {
    Unit,
    Negotiation(RenderCapabilityNegotiationResponse),
    GraphCompileReport(RenderGraphCompileReport),
    GraphValidationReport(RenderGraphValidationReport),
    GraphSubmitReport(RenderGraphSubmitReport),
    UploadPumpReport(UploadPumpReport),
    DiagnosticsSnapshot(RenderDiagnosticsSnapshot),
    Problem(RenderProblemDetails),
    LegacyWarning(RenderLegacyWarning),
    Immediate(RenderImmediateResponse),
    V1(RenderResponseV1),
}

pub type RenderRequestV2 = RenderRequestV3;
pub type RenderResponseV2 = RenderResponseV3;
