use crate::{
    BeginFrameDesc, BeginRenderTargetDesc, BindGroupDesc, BindGroupId, BindGroupLayoutDesc,
    BindGroupLayoutId, BufferDesc, BufferId, BufferSlice, Color4, DrawArgs, DrawIndexedArgs,
    Extent2D, IndexFormat, PipelineDesc, PipelineId, PipelineWarmupDesc, PipelineWarmupReport,
    PostFxFrameParams, RectI32, RenderBackendCapabilities, RenderBackendEvent,
    RenderDiagnosticsSnapshot, RenderDrawListKind, RenderEffectStack, RenderFeature,
    RenderGraphCompileReport, RenderGraphDesc, RenderGraphPassKind, RenderGraphSubmitReport,
    RenderGraphValidationReport, RenderTargetDesc, RenderTargetId, RenderWorkBudget, SamplerDesc,
    SamplerId, ShaderDesc, ShaderId, ShaderRuntimeCacheStats, TextureDesc, TextureId,
    TextureResidencySnapshot, UiDrawList, UiTexId, UploadPumpDesc, UploadPumpReport, Viewport,
};
use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;

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
    SetUiDrawList(Box<UiDrawList>),
    SetDebugText(String),
    SetRenderPhase {
        phase: Option<RenderGraphPassKind>,
    },
    SetDrawListKind {
        kind: Option<RenderDrawListKind>,
    },
    DiscardRecordedCommands,
    EndFrame,
    Resize {
        width: u32,
        height: u32,
    },
    CreateRenderTarget(RenderTargetDesc),
    DestroyRenderTarget {
        id: RenderTargetId,
    },
    RenderTargetUiTexId {
        id: RenderTargetId,
    },
    RenderTargetColorTextureId {
        id: RenderTargetId,
    },
    BeginRenderTarget(BeginRenderTargetDesc),
    EndRenderTarget,
    CreateBuffer(BufferDesc),
    DestroyBuffer {
        id: BufferId,
    },
    WriteBuffer {
        id: BufferId,
        offset: u64,
        data: Vec<u8>,
    },
    CreateTexture(TextureDesc),
    DestroyTexture {
        id: TextureId,
    },
    CreateSampler(SamplerDesc),
    DestroySampler {
        id: SamplerId,
    },
    CreateShader(ShaderDesc),
    DestroyShader {
        id: ShaderId,
    },
    CreatePipeline(PipelineDesc),
    DestroyPipeline {
        id: PipelineId,
    },
    CreateBindGroupLayout(BindGroupLayoutDesc),
    DestroyBindGroupLayout {
        id: BindGroupLayoutId,
    },
    CreateBindGroup(BindGroupDesc),
    DestroyBindGroup {
        id: BindGroupId,
    },
    SetViewport(Viewport),
    SetScissor(RectI32),
    SetPipeline {
        pipeline: PipelineId,
    },
    SetBindGroup {
        index: u32,
        group: BindGroupId,
    },
    SetVertexBuffer {
        slot: u32,
        slice: BufferSlice,
    },
    SetIndexBuffer {
        slice: BufferSlice,
        format: IndexFormat,
    },
    Draw(DrawArgs),
    DrawIndexed(DrawIndexedArgs),
    SetWorkBudget(RenderWorkBudget),
    PumpUploads(UploadPumpDesc),
    TextureResidency {
        id: TextureId,
    },
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
    DiagnosticsSnapshot(Box<RenderDiagnosticsSnapshot>),
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

const MULTI_ADAPTER_MESH_REQUEST_MAGIC: &[u8; 8] = b"NEMW\x01\0\0\0";
const MULTI_ADAPTER_MESH_RESPONSE_MAGIC: &[u8; 8] = b"NEMX\x01\0\0\0";
pub const MULTI_ADAPTER_VERTEX_STRIDE_BYTES: usize = 32;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MultiAdapterMeshTranscodeRequest {
    /// Interleaved little-endian f32 records: position.xyz, normal.xyz, uv.xy.
    pub vertex_bytes: Vec<u8>,
}

impl MultiAdapterMeshTranscodeRequest {
    pub fn new(vertex_bytes: Vec<u8>) -> Result<Self, String> {
        validate_multi_adapter_vertex_bytes(&vertex_bytes)?;
        Ok(Self { vertex_bytes })
    }

    #[inline]
    pub fn vertex_count(&self) -> usize {
        self.vertex_bytes.len() / MULTI_ADAPTER_VERTEX_STRIDE_BYTES
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MultiAdapterMeshTranscodeResult {
    pub worker_index: u32,
    pub invalid_vertex_count: u32,
    pub gpu_elapsed_ns: u64,
    pub vertex_bytes: Vec<u8>,
}

impl MultiAdapterMeshTranscodeResult {
    #[inline]
    pub fn vertex_count(&self) -> usize {
        self.vertex_bytes.len() / MULTI_ADAPTER_VERTEX_STRIDE_BYTES
    }
}

pub fn encode_multi_adapter_mesh_transcode_request(
    request: &MultiAdapterMeshTranscodeRequest,
) -> Result<Vec<u8>, String> {
    validate_multi_adapter_vertex_bytes(&request.vertex_bytes)?;
    let vertex_count = u32::try_from(request.vertex_count())
        .map_err(|_| "multi-adapter mesh packet contains too many vertices".to_owned())?;
    let mut out = Vec::with_capacity(request.vertex_bytes.len().saturating_add(20));
    out.extend_from_slice(MULTI_ADAPTER_MESH_REQUEST_MAGIC);
    put_u32(&mut out, MULTI_ADAPTER_VERTEX_STRIDE_BYTES as u32);
    put_u32(&mut out, vertex_count);
    put_bytes(
        &mut out,
        &request.vertex_bytes,
        "multi-adapter vertex payload",
    )?;
    Ok(out)
}

pub fn decode_multi_adapter_mesh_transcode_request(
    bytes: &[u8],
) -> Result<MultiAdapterMeshTranscodeRequest, String> {
    let mut reader = BinReader::new(bytes);
    if reader.take(8)? != MULTI_ADAPTER_MESH_REQUEST_MAGIC {
        return Err("multi-adapter mesh request has invalid magic".to_owned());
    }
    let stride = reader.u32()? as usize;
    if stride != MULTI_ADAPTER_VERTEX_STRIDE_BYTES {
        return Err(format!(
            "multi-adapter mesh request has unsupported vertex stride={stride} expected={MULTI_ADAPTER_VERTEX_STRIDE_BYTES}"
        ));
    }
    let declared_count = reader.u32()? as usize;
    let vertex_bytes = reader.bytes_vec()?;
    if !reader.is_eof() {
        return Err("multi-adapter mesh request has trailing bytes".to_owned());
    }
    validate_multi_adapter_vertex_bytes(&vertex_bytes)?;
    let actual_count = vertex_bytes.len() / MULTI_ADAPTER_VERTEX_STRIDE_BYTES;
    if actual_count != declared_count {
        return Err(format!(
            "multi-adapter mesh request vertex count mismatch declared={declared_count} actual={actual_count}"
        ));
    }
    Ok(MultiAdapterMeshTranscodeRequest { vertex_bytes })
}

pub fn encode_multi_adapter_mesh_transcode_result(
    result: &MultiAdapterMeshTranscodeResult,
) -> Result<Vec<u8>, String> {
    validate_multi_adapter_vertex_bytes(&result.vertex_bytes)?;
    let vertex_count = u32::try_from(result.vertex_count())
        .map_err(|_| "multi-adapter mesh response contains too many vertices".to_owned())?;
    let mut out = Vec::with_capacity(result.vertex_bytes.len().saturating_add(32));
    out.extend_from_slice(MULTI_ADAPTER_MESH_RESPONSE_MAGIC);
    put_u32(&mut out, result.worker_index);
    put_u32(&mut out, result.invalid_vertex_count);
    put_u64(&mut out, result.gpu_elapsed_ns);
    put_u32(&mut out, MULTI_ADAPTER_VERTEX_STRIDE_BYTES as u32);
    put_u32(&mut out, vertex_count);
    put_bytes(
        &mut out,
        &result.vertex_bytes,
        "multi-adapter result payload",
    )?;
    Ok(out)
}

pub fn decode_multi_adapter_mesh_transcode_result(
    bytes: &[u8],
) -> Result<MultiAdapterMeshTranscodeResult, String> {
    let mut reader = BinReader::new(bytes);
    if reader.take(8)? != MULTI_ADAPTER_MESH_RESPONSE_MAGIC {
        return Err("multi-adapter mesh response has invalid magic".to_owned());
    }
    let worker_index = reader.u32()?;
    let invalid_vertex_count = reader.u32()?;
    let gpu_elapsed_ns = reader.u64()?;
    let stride = reader.u32()? as usize;
    if stride != MULTI_ADAPTER_VERTEX_STRIDE_BYTES {
        return Err(format!(
            "multi-adapter mesh response has unsupported vertex stride={stride} expected={MULTI_ADAPTER_VERTEX_STRIDE_BYTES}"
        ));
    }
    let declared_count = reader.u32()? as usize;
    let vertex_bytes = reader.bytes_vec()?;
    if !reader.is_eof() {
        return Err("multi-adapter mesh response has trailing bytes".to_owned());
    }
    validate_multi_adapter_vertex_bytes(&vertex_bytes)?;
    let actual_count = vertex_bytes.len() / MULTI_ADAPTER_VERTEX_STRIDE_BYTES;
    if actual_count != declared_count {
        return Err(format!(
            "multi-adapter mesh response vertex count mismatch declared={declared_count} actual={actual_count}"
        ));
    }
    Ok(MultiAdapterMeshTranscodeResult {
        worker_index,
        invalid_vertex_count,
        gpu_elapsed_ns,
        vertex_bytes,
    })
}

fn validate_multi_adapter_vertex_bytes(bytes: &[u8]) -> Result<(), String> {
    if bytes.is_empty() {
        return Err("multi-adapter mesh packet contains no vertices".to_owned());
    }
    if bytes.len() % MULTI_ADAPTER_VERTEX_STRIDE_BYTES != 0 {
        return Err(format!(
            "multi-adapter mesh packet byte length is not stride-aligned bytes={} stride={MULTI_ADAPTER_VERTEX_STRIDE_BYTES}",
            bytes.len()
        ));
    }
    const MAX_PACKET_BYTES: usize = 128 * 1024 * 1024;
    if bytes.len() > MAX_PACKET_BYTES {
        return Err(format!(
            "multi-adapter mesh packet exceeds safety limit bytes={} limit={MAX_PACKET_BYTES}",
            bytes.len()
        ));
    }
    Ok(())
}

const CREATE_TEXTURE_BIN_MAGIC: &[u8; 8] = b"NECT\x01\0\0\0";
const CREATE_TEXTURE_RESPONSE_BIN_MAGIC: &[u8; 8] = b"NETR\x01\0\0\0";

/// Compact binary transport for `TextureDesc`, including an optional full mip chain.
/// JSON is intentionally not used here because large `Vec<u8>` payloads expand into
/// millions of decimal tokens and can stall the native window for tens of seconds.
pub fn encode_create_texture_bin(desc: &TextureDesc) -> Result<Vec<u8>, String> {
    let payload_len = desc.data.as_ref().map_or(0, Vec::len);
    let mut out = Vec::with_capacity(payload_len.saturating_add(128));
    out.extend_from_slice(CREATE_TEXTURE_BIN_MAGIC);
    match desc.label.as_deref() {
        Some(label) => {
            put_u8(&mut out, 1);
            put_bytes(&mut out, label.as_bytes(), "texture label")?;
        }
        None => put_u8(&mut out, 0),
    }
    put_u32(&mut out, desc.extent.width);
    put_u32(&mut out, desc.extent.height);
    put_u8(&mut out, texture_format_tag(desc.format));
    put_u8(&mut out, texture_usage_tag(desc.usage));
    put_u32(&mut out, desc.mip_levels.get());
    put_u8(&mut out, texture_data_policy_tag(desc.data_policy));
    put_len(&mut out, desc.mip_data.len(), "texture mip layout")?;
    for mip in &desc.mip_data {
        put_u32(&mut out, mip.level);
        put_u32(&mut out, mip.width);
        put_u32(&mut out, mip.height);
        put_u64(&mut out, mip.offset);
        put_u64(&mut out, mip.byte_len);
    }
    match desc.data.as_deref() {
        Some(data) => {
            put_u8(&mut out, 1);
            put_bytes(&mut out, data, "texture payload")?;
        }
        None => put_u8(&mut out, 0),
    }
    Ok(out)
}

pub fn decode_create_texture_bin(bytes: &[u8]) -> Result<TextureDesc, String> {
    let mut r = BinReader::new(bytes);
    if r.take(8)? != CREATE_TEXTURE_BIN_MAGIC {
        return Err("create-texture binary packet has invalid magic".to_owned());
    }
    let label = match r.u8()? {
        0 => None,
        1 => Some(r.string()?),
        tag => return Err(format!("invalid create-texture label presence tag {tag}")),
    };
    let extent = Extent2D::new(r.u32()?, r.u32()?);
    let format = texture_format_from_tag(r.u8()?)?;
    let usage = texture_usage_from_tag(r.u8()?)?;
    let mip_levels = NonZeroU32::new(r.u32()?)
        .ok_or_else(|| "create-texture binary packet has zero mip levels".to_owned())?;
    let data_policy = texture_data_policy_from_tag(r.u8()?)?;
    let mip_count = r.u32()? as usize;
    let mut mip_data = Vec::with_capacity(mip_count);
    for _ in 0..mip_count {
        mip_data.push(crate::TextureMipDataDesc::new(
            r.u32()?,
            r.u32()?,
            r.u32()?,
            r.u64()?,
            r.u64()?,
        ));
    }
    let data = match r.u8()? {
        0 => None,
        1 => Some(r.bytes_vec()?),
        tag => return Err(format!("invalid create-texture data presence tag {tag}")),
    };
    if !r.is_eof() {
        return Err("create-texture binary packet has trailing bytes".to_owned());
    }
    Ok(TextureDesc {
        label,
        extent,
        format,
        usage,
        mip_levels,
        data,
        mip_data,
        data_policy,
    })
}

pub fn encode_texture_id_bin(id: TextureId) -> Vec<u8> {
    let mut out = Vec::with_capacity(12);
    out.extend_from_slice(CREATE_TEXTURE_RESPONSE_BIN_MAGIC);
    put_u32(&mut out, id.get());
    out
}

pub fn decode_texture_id_bin(bytes: &[u8]) -> Result<TextureId, String> {
    let mut r = BinReader::new(bytes);
    if r.take(8)? != CREATE_TEXTURE_RESPONSE_BIN_MAGIC {
        return Err("create-texture binary response has invalid magic".to_owned());
    }
    let id = TextureId::new(r.u32()?);
    if !r.is_eof() {
        return Err("create-texture binary response has trailing bytes".to_owned());
    }
    Ok(id)
}

#[inline]
fn texture_format_tag(format: crate::TextureFormat) -> u8 {
    match format {
        crate::TextureFormat::Rgba8Unorm => 1,
        crate::TextureFormat::Rgba8Srgb => 2,
        crate::TextureFormat::Bgra8Unorm => 3,
        crate::TextureFormat::Bgra8Srgb => 4,
        crate::TextureFormat::Rgba16Float => 5,
        crate::TextureFormat::R32Float => 6,
        crate::TextureFormat::Bc1RgbaUnorm => 7,
        crate::TextureFormat::Bc1RgbaSrgb => 8,
        crate::TextureFormat::Bc3RgbaUnorm => 9,
        crate::TextureFormat::Bc3RgbaSrgb => 10,
        crate::TextureFormat::Bc5RgUnorm => 11,
        crate::TextureFormat::Bc7RgbaUnorm => 12,
        crate::TextureFormat::Bc7RgbaSrgb => 13,
        crate::TextureFormat::Depth24Stencil8 => 14,
        crate::TextureFormat::Depth32Float => 15,
    }
}

fn texture_format_from_tag(tag: u8) -> Result<crate::TextureFormat, String> {
    match tag {
        1 => Ok(crate::TextureFormat::Rgba8Unorm),
        2 => Ok(crate::TextureFormat::Rgba8Srgb),
        3 => Ok(crate::TextureFormat::Bgra8Unorm),
        4 => Ok(crate::TextureFormat::Bgra8Srgb),
        5 => Ok(crate::TextureFormat::Rgba16Float),
        6 => Ok(crate::TextureFormat::R32Float),
        7 => Ok(crate::TextureFormat::Bc1RgbaUnorm),
        8 => Ok(crate::TextureFormat::Bc1RgbaSrgb),
        9 => Ok(crate::TextureFormat::Bc3RgbaUnorm),
        10 => Ok(crate::TextureFormat::Bc3RgbaSrgb),
        11 => Ok(crate::TextureFormat::Bc5RgUnorm),
        12 => Ok(crate::TextureFormat::Bc7RgbaUnorm),
        13 => Ok(crate::TextureFormat::Bc7RgbaSrgb),
        14 => Ok(crate::TextureFormat::Depth24Stencil8),
        15 => Ok(crate::TextureFormat::Depth32Float),
        _ => Err(format!("invalid texture format binary tag {tag}")),
    }
}

#[inline]
fn texture_usage_tag(usage: crate::TextureUsage) -> u8 {
    match usage {
        crate::TextureUsage::Sampled => 1,
        crate::TextureUsage::RenderTarget => 2,
        crate::TextureUsage::DepthStencil => 3,
        crate::TextureUsage::Storage => 4,
    }
}

fn texture_usage_from_tag(tag: u8) -> Result<crate::TextureUsage, String> {
    match tag {
        1 => Ok(crate::TextureUsage::Sampled),
        2 => Ok(crate::TextureUsage::RenderTarget),
        3 => Ok(crate::TextureUsage::DepthStencil),
        4 => Ok(crate::TextureUsage::Storage),
        _ => Err(format!("invalid texture usage binary tag {tag}")),
    }
}

#[inline]
fn texture_data_policy_tag(policy: crate::TextureDataPolicy) -> u8 {
    match policy {
        crate::TextureDataPolicy::Immediate => 1,
        crate::TextureDataPolicy::Deferred => 2,
        crate::TextureDataPolicy::Empty => 3,
    }
}

fn texture_data_policy_from_tag(tag: u8) -> Result<crate::TextureDataPolicy, String> {
    match tag {
        1 => Ok(crate::TextureDataPolicy::Immediate),
        2 => Ok(crate::TextureDataPolicy::Deferred),
        3 => Ok(crate::TextureDataPolicy::Empty),
        _ => Err(format!("invalid texture data-policy binary tag {tag}")),
    }
}

fn encode_unit_command(out: &mut Vec<u8>, command: &RenderCommand) -> Result<(), String> {
    match command {
        RenderCommand::WriteBuffer { id, offset, data } => {
            put_u8(out, 1);
            put_u32(out, id.get());
            put_u64(out, *offset);
            let len = u32::try_from(data.len()).map_err(|_| {
                "render command binary write_buffer payload is too large".to_owned()
            })?;
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
        _ => {
            return Err(format!(
                "render command is not supported by binary unit batch: {command:?}"
            ))
        }
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
        4 => Ok(RenderCommand::SetPipeline {
            pipeline: PipelineId::new(r.u32()?),
        }),
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
            Ok(RenderCommand::SetUiDrawList(Box::new(
                newengine_ui_draw::decode_ui_draw_list_bin(&ui_bytes)?,
            )))
        }
        11 => Ok(RenderCommand::SetRenderPhase {
            phase: r.optional_render_graph_pass_kind()?,
        }),
        12 => Ok(RenderCommand::SetDrawListKind {
            kind: r.optional_render_draw_list_kind()?,
        }),
        13 => Ok(RenderCommand::DiscardRecordedCommands),
        tag => Err(format!("unknown render command batch binary tag {tag}")),
    }
}

#[inline]
fn put_len(out: &mut Vec<u8>, len: usize, what: &str) -> Result<(), String> {
    let len = u32::try_from(len)
        .map_err(|_| format!("{what} is too large for binary render command packet"))?;
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
fn put_u8(out: &mut Vec<u8>, v: u8) {
    out.push(v);
}
#[inline]
fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}
#[inline]
fn put_i32(out: &mut Vec<u8>, v: i32) {
    out.extend_from_slice(&v.to_le_bytes());
}
#[inline]
fn put_u64(out: &mut Vec<u8>, v: u64) {
    out.extend_from_slice(&v.to_le_bytes());
}
#[inline]
fn put_f32(out: &mut Vec<u8>, v: f32) {
    out.extend_from_slice(&v.to_le_bytes());
}
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

struct BinReader<'a>(newengine_ui_draw::binary_codec::ReadCursor<'a>);

impl<'a> BinReader<'a> {
    #[inline]
    fn new(bytes: &'a [u8]) -> Self {
        Self(newengine_ui_draw::binary_codec::ReadCursor::new(
            bytes,
            "render command batch binary packet",
        ))
    }

    fn string(&mut self) -> Result<String, String> {
        String::from_utf8(self.bytes_vec()?)
            .map_err(|e| format!("invalid UTF-8 string in render binary packet: {e}"))
    }

    fn optional_render_graph_pass_kind(&mut self) -> Result<Option<RenderGraphPassKind>, String> {
        match self.u8()? {
            0 => Ok(None),
            1 => Ok(Some(render_graph_pass_kind_from_tag(self.u8()?)?)),
            tag => Err(format!(
                "invalid optional render graph pass kind presence tag {tag}"
            )),
        }
    }

    fn optional_render_draw_list_kind(&mut self) -> Result<Option<RenderDrawListKind>, String> {
        match self.u8()? {
            0 => Ok(None),
            1 => Ok(Some(render_draw_list_kind_from_tag(self.u8()?)?)),
            tag => Err(format!(
                "invalid optional render draw-list kind presence tag {tag}"
            )),
        }
    }
}

impl<'a> core::ops::Deref for BinReader<'a> {
    type Target = newengine_ui_draw::binary_codec::ReadCursor<'a>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl core::ops::DerefMut for BinReader<'_> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
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
        Self {
            major,
            minor,
            patch,
        }
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
        Self {
            render3d_enabled: true,
            render2d_enabled: true,
            ui_postprocess_enabled: false,
        }
    }
}

#[inline]
fn default_true_domain() -> bool {
    true
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
    pub fn with_draw_lists(
        mut self,
        draw_lists: impl IntoIterator<Item = RenderDrawListKind>,
    ) -> Self {
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
    SetRenderPhase {
        phase: Option<RenderGraphPassKind>,
    },
    SetDrawListKind {
        kind: Option<RenderDrawListKind>,
    },
    DiscardRecordedCommands,
    SubmitRenderGraph(RenderGraphDesc),
    SubmitFrame(Box<RenderFrameEnvelope>),
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
    DiagnosticsSnapshot(Box<RenderDiagnosticsSnapshot>),
    BackendEvents(Vec<RenderBackendEvent>),
    Problem(RenderProblemDetails),
}

#[cfg(test)]
mod binary_batch_tests {
    use super::*;

    #[test]
    fn binary_multi_adapter_mesh_packet_roundtrips() {
        let mut vertices = Vec::new();
        for value in [
            1.0_f32, 2.0, 3.0, 0.0, 2.0, 0.0, 0.25, 0.75, 4.0, 5.0, 6.0, 0.0, 0.0, 4.0, 0.5, 1.0,
        ] {
            vertices.extend_from_slice(&value.to_le_bytes());
        }
        let request = MultiAdapterMeshTranscodeRequest::new(vertices.clone()).unwrap();
        let request = decode_multi_adapter_mesh_transcode_request(
            &encode_multi_adapter_mesh_transcode_request(&request).unwrap(),
        )
        .unwrap();
        assert_eq!(request.vertex_count(), 2);
        assert_eq!(request.vertex_bytes, vertices);

        let response = MultiAdapterMeshTranscodeResult {
            worker_index: 1,
            invalid_vertex_count: 2,
            gpu_elapsed_ns: 42_000,
            vertex_bytes: vertices,
        };
        let response = decode_multi_adapter_mesh_transcode_result(
            &encode_multi_adapter_mesh_transcode_result(&response).unwrap(),
        )
        .unwrap();
        assert_eq!(response.worker_index, 1);
        assert_eq!(response.invalid_vertex_count, 2);
        assert_eq!(response.gpu_elapsed_ns, 42_000);
        assert_eq!(response.vertex_count(), 2);
    }

    #[test]
    fn binary_create_texture_roundtrips_payload_and_response() {
        let desc = TextureDesc::new(
            Extent2D::new(8, 4),
            crate::TextureFormat::Bc3RgbaSrgb,
            crate::TextureUsage::Sampled,
        )
        .with_label("binary-texture")
        .with_mips(NonZeroU32::new(2).unwrap())
        .with_deferred_mip_data(
            vec![
                crate::TextureMipDataDesc::new(0, 8, 4, 0, 32),
                crate::TextureMipDataDesc::new(1, 4, 2, 32, 16),
            ],
            (0_u8..48).collect(),
        );
        let encoded = encode_create_texture_bin(&desc).unwrap();
        let decoded = decode_create_texture_bin(&encoded).unwrap();
        assert_eq!(decoded.label.as_deref(), Some("binary-texture"));
        assert_eq!(decoded.extent.width, 8);
        assert_eq!(decoded.extent.height, 4);
        assert_eq!(decoded.format, crate::TextureFormat::Bc3RgbaSrgb);
        assert_eq!(decoded.usage, crate::TextureUsage::Sampled);
        assert_eq!(decoded.mip_levels.get(), 2);
        assert_eq!(decoded.data_policy, crate::TextureDataPolicy::Deferred);
        assert_eq!(decoded.mip_data.len(), 2);
        assert_eq!(decoded.data.as_ref().map(Vec::len), Some(48));

        let id = TextureId::new(77);
        assert_eq!(
            decode_texture_id_bin(&encode_texture_id_bin(id)).unwrap(),
            id
        );
    }

    #[test]
    fn binary_unit_batch_roundtrips_ui_draw_list() {
        let mut ui = UiDrawList::new();
        ui.screen_size_px = [320, 200];
        ui.pixels_per_point = 1.0;

        let encoded =
            encode_unit_command_batch_bin(&[RenderCommand::SetUiDrawList(Box::new(ui))]).unwrap();
        let decoded = decode_unit_command_batch_bin(&encoded).unwrap();
        match &decoded[0] {
            RenderCommand::SetUiDrawList(list) => assert_eq!(list.screen_size_px, [320, 200]),
            other => panic!("expected SetUiDrawList, got {other:?}"),
        }
    }

    #[test]
    fn binary_unit_batch_roundtrips_recording_scope_commands() {
        let encoded = encode_unit_command_batch_bin(&[
            RenderCommand::SetDrawListKind {
                kind: Some(RenderDrawListKind::OpaqueForward),
            },
            RenderCommand::SetRenderPhase {
                phase: Some(RenderGraphPassKind::UiComposite),
            },
            RenderCommand::SetDrawListKind { kind: None },
            RenderCommand::DiscardRecordedCommands,
        ])
        .unwrap();
        let decoded = decode_unit_command_batch_bin(&encoded).unwrap();

        assert!(matches!(
            decoded[0],
            RenderCommand::SetDrawListKind {
                kind: Some(RenderDrawListKind::OpaqueForward)
            }
        ));
        assert!(matches!(
            decoded[1],
            RenderCommand::SetRenderPhase {
                phase: Some(RenderGraphPassKind::UiComposite)
            }
        ));
        assert!(matches!(
            decoded[2],
            RenderCommand::SetDrawListKind { kind: None }
        ));
        assert!(matches!(decoded[3], RenderCommand::DiscardRecordedCommands));
    }
}
