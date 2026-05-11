use crate::{
    BeginFrameDesc, BeginRenderTargetDesc, BindGroupDesc, BindGroupId, BindGroupLayoutDesc,
    BindGroupLayoutId, BufferDesc, BufferId, BufferSlice, DrawArgs, DrawIndexedArgs,
    IndexFormat, PipelineDesc, PipelineId, PipelineWarmupDesc, PipelineWarmupReport, RectI32,
    RenderBackendCapabilities, RenderDiagnosticsSnapshot, RenderFeature, RenderGraphCompileReport,
    RenderGraphDesc, RenderGraphSubmitReport, RenderGraphValidationReport, RenderTargetDesc,
    RenderTargetId, RenderWorkBudget, SamplerDesc, SamplerId, ShaderDesc, ShaderId,
    ShaderRuntimeCacheStats, TextureDesc, TextureId, TextureResidencySnapshot, UiDrawList,
    UiTexId, UploadPumpDesc, UploadPumpReport, Viewport,
};
use serde::{Deserialize, Serialize};

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

/// RFC 9457-style machine-readable problem detail for render service calls.
///
/// The transport may still return a string for ABI compatibility, but v2 callers
/// can encode/decode this struct and make deterministic decisions by `code`,
/// `phase` and `retryable` instead of parsing human text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderProblemDetails {
    #[serde(rename = "type")]
    pub type_uri: String,
    pub title: String,
    pub status: u16,
    pub detail: String,
    pub instance: Option<String>,
    pub code: String,
    pub phase: Option<String>,
    pub backend: Option<String>,
    pub retryable: bool,
}

impl RenderProblemDetails {
    #[inline]
    pub fn new(code: impl Into<String>, title: impl Into<String>, detail: impl Into<String>) -> Self {
        let code = code.into();
        Self {
            type_uri: format!("urn:newengine:render:error:{code}"),
            title: title.into(),
            status: 500,
            detail: detail.into(),
            instance: None,
            code,
            phase: None,
            backend: None,
            retryable: false,
        }
    }

    #[inline]
    pub fn unsupported(detail: impl Into<String>) -> Self {
        Self {
            status: 501,
            retryable: false,
            ..Self::new("render.unsupported", "Unsupported render API operation", detail)
        }
    }

    #[inline]
    pub fn with_phase(mut self, phase: impl Into<String>) -> Self {
        self.phase = Some(phase.into());
        self
    }

    #[inline]
    pub fn with_backend(mut self, backend: impl Into<String>) -> Self {
        self.backend = Some(backend.into());
        self
    }

    #[inline]
    pub fn retryable(mut self, retryable: bool) -> Self {
        self.retryable = retryable;
        self
    }
}

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
pub struct RenderBackendInfoV2 {
    pub backend: RenderBackendInfoV1,
    pub protocol_version: RenderApiVersion,
    pub min_protocol_version: RenderApiVersion,
    pub supported_protocol_versions: Vec<RenderApiVersion>,
    pub supported_features: Vec<RenderFeature>,
    pub methods: Vec<String>,
    pub problem_details: bool,
}

impl RenderBackendInfoV2 {
    #[inline]
    pub fn from_v1(backend: RenderBackendInfoV1) -> Self {
        let supported_features = backend.capabilities.features.clone();
        Self {
            backend,
            protocol_version: RenderApiVersion::new(2, 0, 0),
            min_protocol_version: RenderApiVersion::new(1, 0, 0),
            supported_protocol_versions: vec![RenderApiVersion::new(1, 0, 0), RenderApiVersion::new(2, 0, 0)],
            supported_features,
            methods: vec![
                "info_json_v1".to_string(),
                "invoke_json_v1".to_string(),
                "info_json_v2".to_string(),
                "invoke_json_v2".to_string(),
            ],
            problem_details: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderCapabilityNegotiationRequest {
    pub min_version: RenderApiVersion,
    pub preferred_version: RenderApiVersion,
    #[serde(default)]
    pub required_features: Vec<RenderFeature>,
    #[serde(default)]
    pub optional_features: Vec<RenderFeature>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderCapabilityNegotiationResponse {
    pub accepted_version: RenderApiVersion,
    pub backend_version: RenderApiVersion,
    pub enabled_features: Vec<RenderFeature>,
    pub missing_required_features: Vec<RenderFeature>,
    pub ok: bool,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RenderRequestV2 {
    Negotiate(RenderCapabilityNegotiationRequest),
    CompileRenderGraph(RenderGraphDesc),
    SubmitRenderGraph(RenderGraphDesc),
    ValidateRenderGraph(RenderGraphDesc),
    SetWorkBudget(RenderWorkBudget),
    PumpUploads(UploadPumpDesc),
    DiagnosticsSnapshot,
    /// Compatibility escape hatch for v2 clients that still need a v1 command.
    V1(RenderRequestV1),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RenderResponseV2 {
    Unit,
    Negotiation(RenderCapabilityNegotiationResponse),
    GraphCompileReport(RenderGraphCompileReport),
    GraphSubmitReport(RenderGraphSubmitReport),
    GraphValidationReport(RenderGraphValidationReport),
    UploadPumpReport(UploadPumpReport),
    DiagnosticsSnapshot(RenderDiagnosticsSnapshot),
    V1(RenderResponseV1),
    Problem(RenderProblemDetails),
}

#[inline]
pub fn encode_json<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    serde_json::to_vec(value).map_err(|e| e.to_string())
}

#[inline]
pub fn decode_json<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<T, String> {
    serde_json::from_slice(bytes).map_err(|e| e.to_string())
}

#[inline]
pub fn encode_problem_error(problem: &RenderProblemDetails) -> String {
    serde_json::to_string(problem).unwrap_or_else(|_| problem.detail.clone())
}

#[inline]
pub fn decode_problem_error(text: &str) -> Option<RenderProblemDetails> {
    serde_json::from_str(text).ok()
}
