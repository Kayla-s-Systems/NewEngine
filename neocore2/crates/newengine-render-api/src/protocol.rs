use crate::{
    BeginFrameDesc, BeginRenderTargetDesc, BindGroupDesc, BindGroupId, BindGroupLayoutDesc,
    BindGroupLayoutId, BufferDesc, BufferId, BufferSlice, Color4, DrawArgs, DrawIndexedArgs,
    Extent2D, IndexFormat, PipelineDesc, PipelineId, PipelineWarmupDesc, PipelineWarmupReport, PostFxFrameParams,
    RectI32, RenderBackendCapabilities, RenderDiagnosticsSnapshot, RenderDrawListKind, RenderEffectStack,
    RenderFeature, RenderGraphCompileReport, RenderGraphDesc, RenderGraphPassKind,
    RenderGraphSubmitReport, RenderGraphValidationReport, RenderTargetDesc, RenderTargetId,
    RenderBackendEvent, RenderWorkBudget, SamplerDesc, SamplerId, ShaderDesc, ShaderId, ShaderRuntimeCacheStats,
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
    SetRenderPhase { phase: Option<RenderGraphPassKind> },
    SetDrawListKind { kind: Option<RenderDrawListKind> },
    DiscardRecordedCommands,
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

const COMMAND_BATCH_BIN_MAGIC: &[u8; 8] = b"NECB\x01\0\0\0";

/// Encodes frame-local unit render commands into a compact binary packet.
///
/// JSON remains the service control protocol. This packet is only for the
/// hot path commands that return `Unit`; commands that allocate ids or query
/// snapshots intentionally stay on the typed JSON request/response surface.
pub fn encode_unit_command_batch_bin(commands: &[RenderCommand]) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(16 + commands.len().saturating_mul(32));
    out.extend_from_slice(COMMAND_BATCH_BIN_MAGIC);
    let command_count = u32::try_from(commands.len())
        .map_err(|_| "render command binary batch contains too many commands".to_owned())?;
    put_u32(&mut out, command_count);
    for command in commands {
        encode_unit_command(&mut out, command)?;
    }
    Ok(out)
}

pub fn decode_unit_command_batch_bin(bytes: &[u8]) -> Result<Vec<RenderCommand>, String> {
    let mut r = BinReader::new(bytes);
    let magic = r.take(8)?;
    if magic != COMMAND_BATCH_BIN_MAGIC {
        return Err("render command batch binary packet has invalid magic".to_owned());
    }
    let count = r.u32()? as usize;
    let mut commands = Vec::with_capacity(count);
    for _ in 0..count {
        commands.push(decode_unit_command(&mut r)?);
    }
    if !r.is_eof() {
        return Err("render command batch binary packet has trailing bytes".to_owned());
    }
    Ok(commands)
}

fn encode_unit_command(out: &mut Vec<u8>, command: &RenderCommand) -> Result<(), String> {
    match command {
        RenderCommand::WriteBuffer { id, offset, data } => {
            put_u8(out, 1);
            put_u32(out, id.get());
            put_u64(out, *offset);
            let len = u32::try_from(data.len())
                .map_err(|_| "render command binary write_buffer payload is too large".to_owned())?;
            put_u32(out, len);
            out.extend_from_slice(data);
        }
        RenderCommand::SetViewport(vp) => {
            put_u8(out, 2);
            put_f32(out, vp.x);
            put_f32(out, vp.y);
            put_f32(out, vp.w);
            put_f32(out, vp.h);
            put_f32(out, vp.min_depth);
            put_f32(out, vp.max_depth);
        }
        RenderCommand::SetScissor(rect) => {
            put_u8(out, 3);
            put_i32(out, rect.x);
            put_i32(out, rect.y);
            put_i32(out, rect.w);
            put_i32(out, rect.h);
        }
        RenderCommand::SetPipeline { pipeline } => {
            put_u8(out, 4);
            put_u32(out, pipeline.get());
        }
        RenderCommand::SetBindGroup { index, group } => {
            put_u8(out, 5);
            put_u32(out, *index);
            put_u32(out, group.get());
        }
        RenderCommand::SetVertexBuffer { slot, slice } => {
            put_u8(out, 6);
            put_u32(out, *slot);
            put_u32(out, slice.buffer.get());
            put_u64(out, slice.offset);
        }
        RenderCommand::SetIndexBuffer { slice, format } => {
            put_u8(out, 7);
            put_u32(out, slice.buffer.get());
            put_u64(out, slice.offset);
            put_index_format(out, *format);
        }
        RenderCommand::Draw(args) => {
            put_u8(out, 8);
            put_u32(out, args.vertex_count);
            put_u32(out, args.instance_count);
            put_u32(out, args.first_vertex);
            put_u32(out, args.first_instance);
        }
        RenderCommand::DrawIndexed(args) => {
            put_u8(out, 9);
            put_u32(out, args.index_count);
            put_u32(out, args.instance_count);
            put_u32(out, args.first_index);
            put_i32(out, args.vertex_offset);
            put_u32(out, args.first_instance);
        }
        RenderCommand::SetUiDrawList(ui) => {
            put_u8(out, 10);
            let ui_bytes = newengine_ui_draw::encode_ui_draw_list_bin(ui)?;
            put_bytes(out, &ui_bytes, "ui draw-list binary payload")?;
        }
        RenderCommand::SetRenderPhase { phase } => {
            put_u8(out, 11);
            put_optional_render_graph_pass_kind(out, *phase);
        }
        RenderCommand::SetDrawListKind { kind } => {
            put_u8(out, 12);
            put_optional_render_draw_list_kind(out, *kind);
        }
        RenderCommand::DiscardRecordedCommands => {
            put_u8(out, 13);
        }
        _ => return Err(format!("render command is not supported by binary unit batch: {command:?}")),
    }
    Ok(())
}

fn decode_unit_command(r: &mut BinReader<'_>) -> Result<RenderCommand, String> {
    match r.u8()? {
        1 => {
            let id = BufferId::new(r.u32()?);
            let offset = r.u64()?;
            let len = r.u32()? as usize;
            let data = r.take(len)?.to_vec();
            Ok(RenderCommand::WriteBuffer { id, offset, data })
        }
        2 => Ok(RenderCommand::SetViewport(Viewport {
            x: r.f32()?,
            y: r.f32()?,
            w: r.f32()?,
            h: r.f32()?,
            min_depth: r.f32()?,
            max_depth: r.f32()?,
        })),
        3 => Ok(RenderCommand::SetScissor(RectI32 {
            x: r.i32()?,
            y: r.i32()?,
            w: r.i32()?,
            h: r.i32()?,
        })),
        4 => Ok(RenderCommand::SetPipeline { pipeline: PipelineId::new(r.u32()?) }),
        5 => Ok(RenderCommand::SetBindGroup {
            index: r.u32()?,
            group: BindGroupId::new(r.u32()?),
        }),
        6 => Ok(RenderCommand::SetVertexBuffer {
            slot: r.u32()?,
            slice: BufferSlice::new(BufferId::new(r.u32()?), r.u64()?),
        }),
        7 => Ok(RenderCommand::SetIndexBuffer {
            slice: BufferSlice::new(BufferId::new(r.u32()?), r.u64()?),
            format: get_index_format(r.u8()?)?,
        }),
        8 => Ok(RenderCommand::Draw(DrawArgs {
            vertex_count: r.u32()?,
            instance_count: r.u32()?,
            first_vertex: r.u32()?,
            first_instance: r.u32()?,
        })),
        9 => Ok(RenderCommand::DrawIndexed(DrawIndexedArgs {
            index_count: r.u32()?,
            instance_count: r.u32()?,
            first_index: r.u32()?,
            vertex_offset: r.i32()?,
            first_instance: r.u32()?,
        })),
        10 => {
            let ui_bytes = r.bytes_vec()?;
            Ok(RenderCommand::SetUiDrawList(newengine_ui_draw::decode_ui_draw_list_bin(&ui_bytes)?))
        },
        11 => Ok(RenderCommand::SetRenderPhase { phase: r.optional_render_graph_pass_kind()? }),
        12 => Ok(RenderCommand::SetDrawListKind { kind: r.optional_render_draw_list_kind()? }),
        13 => Ok(RenderCommand::DiscardRecordedCommands),
        tag => Err(format!("unknown render command batch binary tag {tag}")),
    }
}


#[inline]
fn put_len(out: &mut Vec<u8>, len: usize, what: &str) -> Result<(), String> {
    let len = u32::try_from(len).map_err(|_| format!("{what} is too large for binary render command packet"))?;
    put_u32(out, len);
    Ok(())
}

#[inline]
fn put_bytes(out: &mut Vec<u8>, bytes: &[u8], what: &str) -> Result<(), String> {
    put_len(out, bytes.len(), what)?;
    out.extend_from_slice(bytes);
    Ok(())
}

#[inline]
fn put_u8(out: &mut Vec<u8>, v: u8) { out.push(v); }
#[inline]
fn put_u32(out: &mut Vec<u8>, v: u32) { out.extend_from_slice(&v.to_le_bytes()); }
#[inline]
fn put_i32(out: &mut Vec<u8>, v: i32) { out.extend_from_slice(&v.to_le_bytes()); }
#[inline]
fn put_u64(out: &mut Vec<u8>, v: u64) { out.extend_from_slice(&v.to_le_bytes()); }
#[inline]
fn put_f32(out: &mut Vec<u8>, v: f32) { out.extend_from_slice(&v.to_le_bytes()); }
#[inline]
fn put_optional_render_graph_pass_kind(out: &mut Vec<u8>, phase: Option<RenderGraphPassKind>) {
    match phase {
        Some(phase) => {
            put_u8(out, 1);
            put_u8(out, render_graph_pass_kind_tag(phase));
        }
        None => put_u8(out, 0),
    }
}

#[inline]
fn put_optional_render_draw_list_kind(out: &mut Vec<u8>, kind: Option<RenderDrawListKind>) {
    match kind {
        Some(kind) => {
            put_u8(out, 1);
            put_u8(out, render_draw_list_kind_tag(kind));
        }
        None => put_u8(out, 0),
    }
}

#[inline]
fn render_graph_pass_kind_tag(kind: RenderGraphPassKind) -> u8 {
    match kind {
        RenderGraphPassKind::DepthPrepass => 1,
        RenderGraphPassKind::ShadowMap => 2,
        RenderGraphPassKind::ShadowCascadeMap => 3,
        RenderGraphPassKind::TessellationPrepare => 4,
        RenderGraphPassKind::GBuffer => 5,
        RenderGraphPassKind::DeferredLighting => 6,
        RenderGraphPassKind::ForwardOpaque => 7,
        RenderGraphPassKind::Transparent => 8,
        RenderGraphPassKind::Water => 9,
        RenderGraphPassKind::PostFx => 10,
        RenderGraphPassKind::BloomExtract => 11,
        RenderGraphPassKind::BloomBlur => 12,
        RenderGraphPassKind::TaaResolve => 13,
        RenderGraphPassKind::MsaaResolve => 14,
        RenderGraphPassKind::UiComposite => 15,
        RenderGraphPassKind::UiBackdropBlur => 19,
        RenderGraphPassKind::DebugOverlay => 16,
        RenderGraphPassKind::Copy => 17,
        RenderGraphPassKind::Custom => 18,
    }
}

#[inline]
fn render_graph_pass_kind_from_tag(tag: u8) -> Result<RenderGraphPassKind, String> {
    match tag {
        1 => Ok(RenderGraphPassKind::DepthPrepass),
        2 => Ok(RenderGraphPassKind::ShadowMap),
        3 => Ok(RenderGraphPassKind::ShadowCascadeMap),
        4 => Ok(RenderGraphPassKind::TessellationPrepare),
        5 => Ok(RenderGraphPassKind::GBuffer),
        6 => Ok(RenderGraphPassKind::DeferredLighting),
        7 => Ok(RenderGraphPassKind::ForwardOpaque),
        8 => Ok(RenderGraphPassKind::Transparent),
        9 => Ok(RenderGraphPassKind::Water),
        10 => Ok(RenderGraphPassKind::PostFx),
        11 => Ok(RenderGraphPassKind::BloomExtract),
        12 => Ok(RenderGraphPassKind::BloomBlur),
        13 => Ok(RenderGraphPassKind::TaaResolve),
        14 => Ok(RenderGraphPassKind::MsaaResolve),
        15 => Ok(RenderGraphPassKind::UiComposite),
        19 => Ok(RenderGraphPassKind::UiBackdropBlur),
        16 => Ok(RenderGraphPassKind::DebugOverlay),
        17 => Ok(RenderGraphPassKind::Copy),
        18 => Ok(RenderGraphPassKind::Custom),
        _ => Err(format!("invalid render graph pass kind tag {tag}")),
    }
}

#[inline]
fn render_draw_list_kind_tag(kind: RenderDrawListKind) -> u8 {
    match kind {
        RenderDrawListKind::ShadowCasters => 1,
        RenderDrawListKind::OpaqueForward => 2,
        RenderDrawListKind::Transparent => 3,
        RenderDrawListKind::Ui => 4,
        RenderDrawListKind::Debug => 5,
    }
}

#[inline]
fn render_draw_list_kind_from_tag(tag: u8) -> Result<RenderDrawListKind, String> {
    match tag {
        1 => Ok(RenderDrawListKind::ShadowCasters),
        2 => Ok(RenderDrawListKind::OpaqueForward),
        3 => Ok(RenderDrawListKind::Transparent),
        4 => Ok(RenderDrawListKind::Ui),
        5 => Ok(RenderDrawListKind::Debug),
        _ => Err(format!("invalid render draw-list kind tag {tag}")),
    }
}

#[inline]
fn put_index_format(out: &mut Vec<u8>, format: IndexFormat) {
    out.push(match format {
        IndexFormat::U16 => 16,
        IndexFormat::U32 => 32,
    });
}
#[inline]
fn get_index_format(v: u8) -> Result<IndexFormat, String> {
    match v {
        16 => Ok(IndexFormat::U16),
        32 => Ok(IndexFormat::U32),
        _ => Err(format!("invalid index format tag {v}")),
    }
}

struct BinReader<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> BinReader<'a> {
    #[inline]
    fn new(bytes: &'a [u8]) -> Self { Self { bytes, cursor: 0 } }
    #[inline]
    fn is_eof(&self) -> bool { self.cursor == self.bytes.len() }

    fn take(&mut self, len: usize) -> Result<&'a [u8], String> {
        let end = self.cursor.saturating_add(len);
        if end > self.bytes.len() {
            return Err("render command batch binary packet ended early".to_owned());
        }
        let out = &self.bytes[self.cursor..end];
        self.cursor = end;
        Ok(out)
    }

    #[inline]
    fn u8(&mut self) -> Result<u8, String> { Ok(self.take(1)?[0]) }
    #[inline]
    fn u32(&mut self) -> Result<u32, String> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    #[inline]
    fn i32(&mut self) -> Result<i32, String> {
        let b = self.take(4)?;
        Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    #[inline]
    fn u64(&mut self) -> Result<u64, String> {
        let b = self.take(8)?;
        Ok(u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
    }
    fn bytes_vec(&mut self) -> Result<Vec<u8>, String> {
        let len = self.u32()? as usize;
        Ok(self.take(len)?.to_vec())
    }
    fn optional_render_graph_pass_kind(&mut self) -> Result<Option<RenderGraphPassKind>, String> {
        match self.u8()? {
            0 => Ok(None),
            1 => Ok(Some(render_graph_pass_kind_from_tag(self.u8()?)?)),
            tag => Err(format!("invalid optional render graph pass kind presence tag {tag}")),
        }
    }

    fn optional_render_draw_list_kind(&mut self) -> Result<Option<RenderDrawListKind>, String> {
        match self.u8()? {
            0 => Ok(None),
            1 => Ok(Some(render_draw_list_kind_from_tag(self.u8()?)?)),
            tag => Err(format!("invalid optional render draw-list kind presence tag {tag}")),
        }
    }

    #[inline]
    fn f32(&mut self) -> Result<f32, String> {
        let b = self.take(4)?;
        Ok(f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
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


#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RenderFrameDomainIntent {
    #[serde(default = "default_true_domain")]
    pub render3d_enabled: bool,
    #[serde(default = "default_true_domain")]
    pub render2d_enabled: bool,
    #[serde(default)]
    pub ui_postprocess_enabled: bool,
}

impl Default for RenderFrameDomainIntent {
    #[inline]
    fn default() -> Self {
        Self { render3d_enabled: true, render2d_enabled: true, ui_postprocess_enabled: false }
    }
}

#[inline]
fn default_true_domain() -> bool { true }

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
    #[serde(default)]
    pub domains: RenderFrameDomainIntent,
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
            domains: RenderFrameDomainIntent::default(),
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
    pub fn with_domain_intent(mut self, domains: RenderFrameDomainIntent) -> Self {
        self.domains = domains;
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
    DrainBackendEvents,
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
    BackendEvents(Vec<RenderBackendEvent>),
    Problem(RenderProblemDetails),
}

#[cfg(test)]
mod binary_batch_tests {
    use super::*;

    #[test]
    fn binary_unit_batch_roundtrips_ui_draw_list() {
        let mut ui = UiDrawList::new();
        ui.screen_size_px = [320, 200];
        ui.pixels_per_point = 1.0;

        let encoded = encode_unit_command_batch_bin(&[RenderCommand::SetUiDrawList(ui)]).unwrap();
        let decoded = decode_unit_command_batch_bin(&encoded).unwrap();
        match &decoded[0] {
            RenderCommand::SetUiDrawList(list) => assert_eq!(list.screen_size_px, [320, 200]),
            other => panic!("expected SetUiDrawList, got {other:?}"),
        }
    }

    #[test]
    fn binary_unit_batch_roundtrips_recording_scope_commands() {
        let encoded = encode_unit_command_batch_bin(&[
            RenderCommand::SetDrawListKind { kind: Some(RenderDrawListKind::OpaqueForward) },
            RenderCommand::SetRenderPhase { phase: Some(RenderGraphPassKind::UiComposite) },
            RenderCommand::SetDrawListKind { kind: None },
            RenderCommand::DiscardRecordedCommands,
        ]).unwrap();
        let decoded = decode_unit_command_batch_bin(&encoded).unwrap();

        assert!(matches!(
            decoded[0],
            RenderCommand::SetDrawListKind { kind: Some(RenderDrawListKind::OpaqueForward) }
        ));
        assert!(matches!(
            decoded[1],
            RenderCommand::SetRenderPhase { phase: Some(RenderGraphPassKind::UiComposite) }
        ));
        assert!(matches!(decoded[2], RenderCommand::SetDrawListKind { kind: None }));
        assert!(matches!(decoded[3], RenderCommand::DiscardRecordedCommands));
    }
}
