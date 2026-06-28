use crate::{Extent2D, RenderTargetId, TextureFormat, TextureId, UploadPumpReport};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RenderGraphResourceId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RenderGraphPassId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderGraphResourceLifetime {
    Persistent,
    TransientFrame,
    Frames(u32),
    External,
}

impl Default for RenderGraphResourceLifetime {
    #[inline]
    fn default() -> Self {
        Self::TransientFrame
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderGraphResourceUsage {
    ColorAttachment,
    DepthAttachment,
    SampledTexture,
    StorageTexture,
    VertexBuffer,
    IndexBuffer,
    UniformBuffer,
    StorageBuffer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderGraphResourceSemantic {
    Unknown,
    SurfaceColor,
    ViewportColor,
    ViewportDepth,
    ShadowMap,
    SceneHdrColor,
    GBufferAlbedo,
    GBufferNormal,
    GBufferMaterial,
    GBufferDepth,
    LitColor,
    PostFxColor,
    UiColor,
    UiBackdropBlur,
    DebugOverlay,
    Custom,
}

impl Default for RenderGraphResourceSemantic {
    #[inline]
    fn default() -> Self {
        Self::Unknown
    }
}

impl RenderGraphResourceSemantic {
    #[inline]
    pub const fn is_depth(self) -> bool {
        matches!(self, Self::ViewportDepth | Self::ShadowMap | Self::GBufferDepth)
    }

    #[inline]
    pub const fn is_surface_color(self) -> bool {
        matches!(self, Self::SurfaceColor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderGraphExternalResource {
    /// Backend-owned swapchain color surface. The backend resolves the current image.
    SwapchainColor,
    /// Runtime/backend render target created through RenderApi::create_render_target.
    RenderTarget(RenderTargetId),
    /// Backend texture imported as a graph-readable external resource.
    Texture(TextureId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderGraphResourceAccess {
    Read,
    Write,
    ReadWrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderGraphQueueKind {
    Graphics,
    Compute,
    Transfer,
}

impl Default for RenderGraphQueueKind {
    #[inline]
    fn default() -> Self {
        Self::Graphics
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RenderGraphPassKind {
    DepthPrepass,
    ShadowMap,
    ShadowCascadeMap,
    TessellationPrepare,
    GBuffer,
    DeferredLighting,
    ForwardOpaque,
    Transparent,
    Water,
    PostFx,
    BloomExtract,
    BloomBlur,
    TaaResolve,
    MsaaResolve,
    UiBackdropBlur,
    UiComposite,
    DebugOverlay,
    Copy,
    Custom,
}

impl Default for RenderGraphPassKind {
    #[inline]
    fn default() -> Self {
        Self::Custom
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderGraphResourceDesc {
    pub id: RenderGraphResourceId,
    pub label: Option<String>,
    #[serde(default)]
    pub semantic: RenderGraphResourceSemantic,
    pub usage: RenderGraphResourceUsage,
    pub lifetime: RenderGraphResourceLifetime,
    #[serde(default)]
    pub extent: Option<Extent2D>,
    #[serde(default)]
    pub format: Option<TextureFormat>,
    #[serde(default)]
    pub external: Option<RenderGraphExternalResource>,
}

impl RenderGraphResourceDesc {
    #[inline]
    pub fn transient_texture(
        id: RenderGraphResourceId,
        label: impl Into<String>,
        usage: RenderGraphResourceUsage,
        extent: Extent2D,
        format: TextureFormat,
    ) -> Self {
        Self {
            id,
            label: Some(label.into()),
            semantic: RenderGraphResourceSemantic::Unknown,
            usage,
            lifetime: RenderGraphResourceLifetime::TransientFrame,
            extent: Some(extent),
            format: Some(format),
            external: None,
        }
    }

    #[inline]
    pub fn external(
        id: RenderGraphResourceId,
        label: impl Into<String>,
        usage: RenderGraphResourceUsage,
    ) -> Self {
        Self {
            id,
            label: Some(label.into()),
            semantic: RenderGraphResourceSemantic::Unknown,
            usage,
            lifetime: RenderGraphResourceLifetime::External,
            extent: None,
            format: None,
            external: None,
        }
    }

    #[inline]
    pub fn external_swapchain(
        id: RenderGraphResourceId,
        label: impl Into<String>,
        usage: RenderGraphResourceUsage,
        extent: Extent2D,
        format: TextureFormat,
    ) -> Self {
        Self {
            id,
            label: Some(label.into()),
            semantic: RenderGraphResourceSemantic::Unknown,
            usage,
            lifetime: RenderGraphResourceLifetime::External,
            extent: Some(extent),
            format: Some(format),
            external: Some(RenderGraphExternalResource::SwapchainColor),
        }
    }

    #[inline]
    pub fn external_render_target(
        id: RenderGraphResourceId,
        label: impl Into<String>,
        render_target: RenderTargetId,
        usage: RenderGraphResourceUsage,
        extent: Extent2D,
        format: TextureFormat,
    ) -> Self {
        Self {
            id,
            label: Some(label.into()),
            semantic: RenderGraphResourceSemantic::Unknown,
            usage,
            lifetime: RenderGraphResourceLifetime::External,
            extent: Some(extent),
            format: Some(format),
            external: Some(RenderGraphExternalResource::RenderTarget(render_target)),
        }
    }

    #[inline]
    pub fn external_texture(
        id: RenderGraphResourceId,
        label: impl Into<String>,
        texture: TextureId,
        usage: RenderGraphResourceUsage,
    ) -> Self {
        Self {
            id,
            label: Some(label.into()),
            semantic: RenderGraphResourceSemantic::Unknown,
            usage,
            lifetime: RenderGraphResourceLifetime::External,
            extent: None,
            format: None,
            external: Some(RenderGraphExternalResource::Texture(texture)),
        }
    }

    #[inline]
    pub fn with_semantic(mut self, semantic: RenderGraphResourceSemantic) -> Self {
        self.semantic = semantic;
        self
    }

}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderGraphResourceRef {
    pub resource: RenderGraphResourceId,
    pub usage: RenderGraphResourceUsage,
    pub access: RenderGraphResourceAccess,
}

impl RenderGraphResourceRef {
    #[inline]
    pub const fn read(resource: RenderGraphResourceId, usage: RenderGraphResourceUsage) -> Self {
        Self {
            resource,
            usage,
            access: RenderGraphResourceAccess::Read,
        }
    }

    #[inline]
    pub const fn write(resource: RenderGraphResourceId, usage: RenderGraphResourceUsage) -> Self {
        Self {
            resource,
            usage,
            access: RenderGraphResourceAccess::Write,
        }
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum RenderDrawListKind {
    /// Geometry that casts into shadow maps.
    ShadowCasters,
    /// Opaque world geometry for the forward viewport path.
    OpaqueForward,
    /// Transparent world geometry that must be drawn after opaque geometry.
    Transparent,
    /// UI draw commands and UI-provider composite work.
    Ui,
    /// Editor/runtime debug primitives and overlays.
    Debug,
}

impl RenderDrawListKind {
    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ShadowCasters => "shadow_casters",
            Self::OpaqueForward => "opaque_forward",
            Self::Transparent => "transparent",
            Self::Ui => "ui",
            Self::Debug => "debug",
        }
    }

    #[inline]
    pub const fn default_pass_kind(self) -> RenderGraphPassKind {
        match self {
            Self::ShadowCasters => RenderGraphPassKind::ShadowMap,
            Self::OpaqueForward => RenderGraphPassKind::ForwardOpaque,
            Self::Transparent => RenderGraphPassKind::Transparent,
            Self::Ui => RenderGraphPassKind::UiComposite,
            Self::Debug => RenderGraphPassKind::DebugOverlay,
        }
    }

    #[inline]
    pub const fn is_compatible_with_pass(self, pass: RenderGraphPassKind) -> bool {
        match (self, pass) {
            (Self::ShadowCasters, RenderGraphPassKind::ShadowMap | RenderGraphPassKind::ShadowCascadeMap | RenderGraphPassKind::DepthPrepass) => true,
            (Self::OpaqueForward, RenderGraphPassKind::ForwardOpaque | RenderGraphPassKind::GBuffer) => true,
            (Self::Transparent, RenderGraphPassKind::Transparent) => true,
            (Self::Ui, RenderGraphPassKind::UiComposite) => true,
            (Self::Debug, RenderGraphPassKind::DebugOverlay) => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum RenderMaterialDomain {
    OpaqueLit,
    Terrain,
    Vegetation,
    ShadowCaster,
    Transparent,
    Water,
    Ui,
    PostFx,
    Debug,
    Custom,
}

impl Default for RenderMaterialDomain {
    #[inline]
    fn default() -> Self {
        Self::Custom
    }
}

impl RenderMaterialDomain {
    #[inline]
    pub const fn is_compatible_with_pass(self, pass: RenderGraphPassKind) -> bool {
        match (self, pass) {
            (Self::ShadowCaster, RenderGraphPassKind::ShadowMap | RenderGraphPassKind::ShadowCascadeMap | RenderGraphPassKind::DepthPrepass) => true,
            (Self::OpaqueLit | Self::Terrain | Self::Vegetation, RenderGraphPassKind::ForwardOpaque | RenderGraphPassKind::GBuffer) => true,
            (Self::Transparent, RenderGraphPassKind::Transparent) => true,
            (Self::Water, RenderGraphPassKind::Water) => true,
            (Self::Ui, RenderGraphPassKind::UiComposite) => true,
            (Self::PostFx, RenderGraphPassKind::PostFx | RenderGraphPassKind::BloomExtract | RenderGraphPassKind::BloomBlur | RenderGraphPassKind::TaaResolve | RenderGraphPassKind::MsaaResolve | RenderGraphPassKind::DeferredLighting | RenderGraphPassKind::UiBackdropBlur) => true,
            (Self::Debug, RenderGraphPassKind::DebugOverlay) => true,
            (Self::Custom, _) => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PipelineKey(pub String);

impl PipelineKey {
    #[inline]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawPacket {
    pub pass_kind: RenderGraphPassKind,
    pub draw_list_kind: RenderDrawListKind,
    pub material_domain: RenderMaterialDomain,
    pub pipeline_key: PipelineKey,
    pub sort_key: u64,
    pub commands: Vec<crate::RenderCommand>,
}

impl DrawPacket {
    #[inline]
    pub fn is_compatible_with_pass(&self, pass: RenderGraphPassKind) -> bool {
        self.pass_kind == pass
            && self.draw_list_kind.is_compatible_with_pass(pass)
            && self.material_domain.is_compatible_with_pass(pass)
    }
}



#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderGraphPassDomain {
    Unknown,
    Render3d,
    Render2d,
    PostProcess,
    Presentation,
}

impl Default for RenderGraphPassDomain {
    #[inline]
    fn default() -> Self { Self::Unknown }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RenderGraphPassFlags {
    #[serde(default)]
    pub allow_culling: bool,
    #[serde(default)]
    pub allow_async_compute: bool,
    #[serde(default)]
    pub debug_marker: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderGraphPassDesc {
    pub id: RenderGraphPassId,
    pub label: String,
    #[serde(default)]
    pub kind: RenderGraphPassKind,
    #[serde(default)]
    pub domain: RenderGraphPassDomain,
    #[serde(default)]
    pub queue: RenderGraphQueueKind,
    #[serde(default)]
    pub reads: Vec<RenderGraphResourceRef>,
    #[serde(default)]
    pub writes: Vec<RenderGraphResourceRef>,
    #[serde(default)]
    pub creates: Vec<RenderGraphResourceId>,
    #[serde(default)]
    pub draw_lists: Vec<RenderDrawListKind>,
    #[serde(default)]
    pub flags: RenderGraphPassFlags,
}

impl RenderGraphPassDesc {
    #[inline]
    pub fn new(id: RenderGraphPassId, label: impl Into<String>, kind: RenderGraphPassKind) -> Self {
        Self {
            id,
            label: label.into(),
            kind,
            domain: RenderGraphPassDomain::Unknown,
            queue: RenderGraphQueueKind::Graphics,
            reads: Vec::new(),
            writes: Vec::new(),
            creates: Vec::new(),
            draw_lists: Vec::new(),
            flags: RenderGraphPassFlags::default(),
        }
    }

    #[inline]
    pub fn with_domain(mut self, domain: RenderGraphPassDomain) -> Self {
        self.domain = domain;
        self
    }

    #[inline]
    pub fn reads(mut self, resource: RenderGraphResourceId, usage: RenderGraphResourceUsage) -> Self {
        self.reads.push(RenderGraphResourceRef::read(resource, usage));
        self
    }

    #[inline]
    pub fn writes(mut self, resource: RenderGraphResourceId, usage: RenderGraphResourceUsage) -> Self {
        self.writes.push(RenderGraphResourceRef::write(resource, usage));
        self
    }

    #[inline]
    pub fn draw_list(mut self, kind: RenderDrawListKind) -> Self {
        if !self.draw_lists.contains(&kind) {
            self.draw_lists.push(kind);
        }
        self
    }
}



#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct VisibilitySettings {
    #[serde(default)]
    pub gpu_visibility_enabled: bool,
    #[serde(default = "default_true")]
    pub hiz_enabled: bool,
    #[serde(default = "default_true")]
    pub pvs_sort_enabled: bool,
    #[serde(default = "default_true")]
    pub zone_cull_enabled: bool,
}

impl Default for VisibilitySettings {
    #[inline]
    fn default() -> Self {
        Self {
            gpu_visibility_enabled: false,
            hiz_enabled: true,
            pvs_sort_enabled: true,
            zone_cull_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FrameCameraContext {
    #[serde(default)]
    pub position_ws: [f32; 3],
    #[serde(default = "default_camera_forward")]
    pub forward_ws: [f32; 3],
    #[serde(default = "default_camera_up")]
    pub up_ws: [f32; 3],
    #[serde(default = "default_camera_fov_y")]
    pub fov_y: f32,
    #[serde(default = "default_camera_near")]
    pub near: f32,
    #[serde(default = "default_camera_far")]
    pub far: f32,
}

impl Default for FrameCameraContext {
    #[inline]
    fn default() -> Self {
        Self {
            position_ws: [0.0, 0.0, 0.0],
            forward_ws: default_camera_forward(),
            up_ws: default_camera_up(),
            fov_y: default_camera_fov_y(),
            near: default_camera_near(),
            far: default_camera_far(),
        }
    }
}

impl FrameCameraContext {
    #[inline]
    pub fn shadow_cache_bucket_hash(self) -> u32 {
        const POS_STEP_METERS: f32 = 0.5;
        const ANGLE_STEP_DEGREES: f32 = 1.0;

        fn quantize_position(value: f32) -> i32 {
            if !value.is_finite() {
                return 0;
            }
            (value / POS_STEP_METERS).round().clamp(i32::MIN as f32, i32::MAX as f32) as i32
        }

        fn normalize_or_default(v: [f32; 3], fallback: [f32; 3]) -> [f32; 3] {
            let len_sq = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
            if len_sq <= 1.0e-8 || !len_sq.is_finite() {
                return fallback;
            }
            let inv_len = len_sq.sqrt().recip();
            [v[0] * inv_len, v[1] * inv_len, v[2] * inv_len]
        }

        fn quantize_unit_component(value: f32) -> i16 {
            let degrees = value.clamp(-1.0, 1.0).asin().to_degrees();
            (degrees / ANGLE_STEP_DEGREES)
                .round()
                .clamp(i16::MIN as f32, i16::MAX as f32) as i16
        }

        fn mix(hash: &mut u32, value: u32) {
            *hash ^= value;
            *hash = hash.wrapping_mul(0x0100_0193);
        }

        let forward = normalize_or_default(self.forward_ws, default_camera_forward());
        let up = normalize_or_default(self.up_ws, default_camera_up());
        let mut hash = 0x811C_9DC5_u32;
        for value in [
            quantize_position(self.position_ws[0]) as u32,
            quantize_position(self.position_ws[1]) as u32,
            quantize_position(self.position_ws[2]) as u32,
            quantize_unit_component(forward[0]) as u32,
            quantize_unit_component(forward[1]) as u32,
            quantize_unit_component(forward[2]) as u32,
            quantize_unit_component(up[0]) as u32,
            quantize_unit_component(up[1]) as u32,
            quantize_unit_component(up[2]) as u32,
        ] {
            mix(&mut hash, value);
        }
        hash
    }
}

#[inline]
fn default_true() -> bool { true }
#[inline]
fn default_camera_forward() -> [f32; 3] { [0.0, 0.0, -1.0] }
#[inline]
fn default_camera_up() -> [f32; 3] { [0.0, 1.0, 0.0] }
#[inline]
fn default_camera_fov_y() -> f32 { 60.0_f32.to_radians() }
#[inline]
fn default_camera_near() -> f32 { 0.05 }
#[inline]
fn default_camera_far() -> f32 { 10_000.0 }


#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecondaryCommandBufferSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub shadow_cascades: bool,
    #[serde(default = "default_true")]
    pub postfx_passes: bool,
    #[serde(default = "default_true")]
    pub visibility_compute: bool,
    #[serde(default = "default_true")]
    pub water_reflection_scopes: bool,
}

impl Default for SecondaryCommandBufferSettings {
    #[inline]
    fn default() -> Self {
        Self {
            enabled: false,
            shadow_cascades: true,
            postfx_passes: true,
            visibility_compute: true,
            water_reflection_scopes: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimePacketBridgeSettings {
    #[serde(default)]
    pub entity_packets_ready: bool,
    #[serde(default)]
    pub light_packets_ready: bool,
    #[serde(default)]
    pub vegetation_instance_packets_ready: bool,
    #[serde(default)]
    pub reflection_zone_packets_ready: bool,
    #[serde(default)]
    pub visibility_object_bound_packets_ready: bool,
}

impl Default for RuntimePacketBridgeSettings {
    #[inline]
    fn default() -> Self {
        Self {
            entity_packets_ready: false,
            light_packets_ready: false,
            vegetation_instance_packets_ready: false,
            reflection_zone_packets_ready: false,
            visibility_object_bound_packets_ready: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindlessDescriptorSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_bindless_texture_capacity")]
    pub texture_capacity: u32,
    #[serde(default = "default_bindless_material_capacity")]
    pub material_capacity: u32,
    #[serde(default = "default_true")]
    pub vegetation_textures: bool,
    #[serde(default = "default_true")]
    pub instanced_materials: bool,
    #[serde(default = "default_true")]
    pub postfx_texture_chain: bool,
}

impl Default for BindlessDescriptorSettings {
    #[inline]
    fn default() -> Self {
        Self {
            enabled: false,
            texture_capacity: default_bindless_texture_capacity(),
            material_capacity: default_bindless_material_capacity(),
            vegetation_textures: true,
            instanced_materials: true,
            postfx_texture_chain: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RendererParitySettings {
    #[serde(default)]
    pub secondary_command_buffers: SecondaryCommandBufferSettings,
    #[serde(default)]
    pub runtime_packets: RuntimePacketBridgeSettings,
    #[serde(default)]
    pub bindless: BindlessDescriptorSettings,
}

impl Default for RendererParitySettings {
    #[inline]
    fn default() -> Self {
        Self {
            secondary_command_buffers: SecondaryCommandBufferSettings::default(),
            runtime_packets: RuntimePacketBridgeSettings::default(),
            bindless: BindlessDescriptorSettings::default(),
        }
    }
}

#[inline]
fn default_bindless_texture_capacity() -> u32 { 16_384 }
#[inline]
fn default_bindless_material_capacity() -> u32 { 8_192 }

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RenderGraphDesc {
    pub label: Option<String>,
    #[serde(default)]
    pub frame_index: u64,
    #[serde(default)]
    pub camera: FrameCameraContext,
    #[serde(default)]
    pub visibility: VisibilitySettings,
    #[serde(default)]
    pub parity: RendererParitySettings,
    #[serde(default)]
    pub resources: Vec<RenderGraphResourceDesc>,
    #[serde(default)]
    pub passes: Vec<RenderGraphPassDesc>,
}

impl RenderGraphDesc {
    #[inline]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: Some(label.into()),
            frame_index: 0,
            camera: FrameCameraContext::default(),
            visibility: VisibilitySettings::default(),
            parity: RendererParitySettings::default(),
            resources: Vec::new(),
            passes: Vec::new(),
        }
    }

    #[inline]
    pub fn add_resource(mut self, resource: RenderGraphResourceDesc) -> Self {
        self.resources.push(resource);
        self
    }

    #[inline]
    pub fn add_pass(mut self, pass: RenderGraphPassDesc) -> Self {
        self.passes.push(pass);
        self
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RenderGraphLifetimeStats {
    pub persistent: u32,
    pub transient: u32,
    pub retired: u32,
    pub destroyed: u32,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct RenderGraphBarrierStats {
    pub read_after_write: u32,
    pub write_after_read: u32,
    pub write_after_write: u32,
    pub external_imports: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RenderGraphDiagnosticsStats {
    pub compiled_graphs: u64,
    pub submitted_graphs: u64,
    pub skipped_passes: u64,
    pub lifetime: RenderGraphLifetimeStats,
    pub barriers: RenderGraphBarrierStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderGraphValidationIssue {
    pub code: String,
    pub message: String,
    pub pass: Option<RenderGraphPassId>,
    pub resource: Option<RenderGraphResourceId>,
}

impl RenderGraphValidationIssue {
    #[inline]
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            pass: None,
            resource: None,
        }
    }

    #[inline]
    pub fn with_pass(mut self, pass: RenderGraphPassId) -> Self {
        self.pass = Some(pass);
        self
    }

    #[inline]
    pub fn with_resource(mut self, resource: RenderGraphResourceId) -> Self {
        self.resource = Some(resource);
        self
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RenderGraphCompileReport {
    pub pass_count: u32,
    pub resource_count: u32,
    pub execution_order: Vec<RenderGraphPassId>,
    pub lifetime: RenderGraphLifetimeStats,
    pub barriers: RenderGraphBarrierStats,
    pub warnings: Vec<RenderGraphValidationIssue>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RecordedDrawListStats {
    pub draw_list: RenderDrawListKind,
    pub recorded_commands: u32,
    pub draw_calls: u32,
    pub indexed_draw_calls: u32,
    pub skipped_commands: u32,
    #[serde(default)]
    pub viewport_sets: u32,
    #[serde(default)]
    pub scissor_sets: u32,
    #[serde(default)]
    pub pipeline_binds: u32,
    #[serde(default)]
    pub vertex_buffer_binds: u32,
    #[serde(default)]
    pub index_buffer_binds: u32,
    #[serde(default)]
    pub bind_group_binds: u32,
    #[serde(default)]
    pub invalid_draw_calls: u32,
}


impl RecordedDrawListStats {
    #[inline]
    pub fn total_draw_calls(self) -> u32 {
        self.draw_calls.saturating_add(self.indexed_draw_calls)
    }

    #[inline]
    pub fn submitted_commands(self) -> u32 {
        self.recorded_commands.saturating_sub(self.skipped_commands)
    }
}

impl Default for RecordedDrawListStats {
    #[inline]
    fn default() -> Self {
        Self {
            draw_list: RenderDrawListKind::OpaqueForward,
            recorded_commands: 0,
            draw_calls: 0,
            indexed_draw_calls: 0,
            skipped_commands: 0,
            viewport_sets: 0,
            scissor_sets: 0,
            pipeline_binds: 0,
            vertex_buffer_binds: 0,
            index_buffer_binds: 0,
            bind_group_binds: 0,
            invalid_draw_calls: 0,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RenderGraphValidationReport {
    pub ok: bool,
    pub errors: Vec<RenderGraphValidationIssue>,
    pub warnings: Vec<RenderGraphValidationIssue>,
    pub compile: Option<RenderGraphCompileReport>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RenderGraphSubmitReport {
    pub cpu_record_ms: f32,
    pub gpu_submit_ms: f32,
    pub executed_passes: u32,
    pub skipped_passes: u32,
    pub uploads: UploadPumpReport,
    pub compile: RenderGraphCompileReport,
    #[serde(default)]
    pub draw_list_stats: Vec<RecordedDrawListStats>,
}

pub fn validate_and_compile_render_graph(graph: &RenderGraphDesc) -> RenderGraphValidationReport {
    match compile_render_graph(graph) {
        Ok(report) => RenderGraphValidationReport {
            ok: true,
            errors: Vec::new(),
            warnings: report.warnings.clone(),
            compile: Some(report),
        },
        Err(errors) => RenderGraphValidationReport {
            ok: false,
            errors,
            warnings: Vec::new(),
            compile: None,
        },
    }
}

pub fn compile_render_graph(
    graph: &RenderGraphDesc,
) -> Result<RenderGraphCompileReport, Vec<RenderGraphValidationIssue>> {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut resource_ids = BTreeSet::new();
    let mut pass_ids = BTreeSet::new();
    let mut lifetime = RenderGraphLifetimeStats::default();

    for resource in &graph.resources {
        if !resource_ids.insert(resource.id) {
            errors.push(
                RenderGraphValidationIssue::new(
                    "render_graph.duplicate_resource",
                    "render graph contains duplicate resource id",
                )
                .with_resource(resource.id),
            );
        }
        match resource.lifetime {
            RenderGraphResourceLifetime::Persistent | RenderGraphResourceLifetime::External => {
                lifetime.persistent = lifetime.persistent.saturating_add(1);
            }
            RenderGraphResourceLifetime::TransientFrame | RenderGraphResourceLifetime::Frames(_) => {
                lifetime.transient = lifetime.transient.saturating_add(1);
            }
        }
    }

    for pass in &graph.passes {
        if !pass_ids.insert(pass.id) {
            errors.push(
                RenderGraphValidationIssue::new(
                    "render_graph.duplicate_pass",
                    "render graph contains duplicate pass id",
                )
                .with_pass(pass.id),
            );
        }

        for access in pass.reads.iter().chain(pass.writes.iter()) {
            if !resource_ids.contains(&access.resource) {
                errors.push(
                    RenderGraphValidationIssue::new(
                        "render_graph.unknown_resource",
                        "render graph pass references a resource that is not declared",
                    )
                    .with_pass(pass.id)
                    .with_resource(access.resource),
                );
            }
        }

        for draw_list in &pass.draw_lists {
            if !draw_list.is_compatible_with_pass(pass.kind) {
                warnings.push(
                    RenderGraphValidationIssue::new(
                        "render_graph.draw_list_route_mismatch",
                        format!(
                            "draw-list '{}' is unusual for render pass kind {:?}",
                            draw_list.label(),
                            pass.kind
                        ),
                    )
                    .with_pass(pass.id),
                );
            }
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    let mut last_writer: BTreeMap<RenderGraphResourceId, RenderGraphPassId> = BTreeMap::new();
    let mut last_readers: BTreeMap<RenderGraphResourceId, BTreeSet<RenderGraphPassId>> = BTreeMap::new();
    let mut dependencies: BTreeMap<RenderGraphPassId, BTreeSet<RenderGraphPassId>> =
        graph.passes.iter().map(|p| (p.id, BTreeSet::new())).collect();
    let mut reverse_edges: BTreeMap<RenderGraphPassId, BTreeSet<RenderGraphPassId>> =
        graph.passes.iter().map(|p| (p.id, BTreeSet::new())).collect();
    let mut barriers = RenderGraphBarrierStats::default();

    for pass in &graph.passes {
        for created in &pass.creates {
            last_writer.insert(*created, pass.id);
        }

        for read in &pass.reads {
            if let Some(writer) = last_writer.get(&read.resource).copied() {
                if writer != pass.id {
                    dependencies.entry(pass.id).or_default().insert(writer);
                    reverse_edges.entry(writer).or_default().insert(pass.id);
                    barriers.read_after_write = barriers.read_after_write.saturating_add(1);
                }
            } else {
                barriers.external_imports = barriers.external_imports.saturating_add(1);
            }
            last_readers.entry(read.resource).or_default().insert(pass.id);
        }

        for write in &pass.writes {
            if let Some(writer) = last_writer.get(&write.resource).copied() {
                if writer != pass.id {
                    dependencies.entry(pass.id).or_default().insert(writer);
                    reverse_edges.entry(writer).or_default().insert(pass.id);
                    barriers.write_after_write = barriers.write_after_write.saturating_add(1);
                }
            }
            if let Some(readers) = last_readers.remove(&write.resource) {
                for reader in readers {
                    if reader != pass.id {
                        dependencies.entry(pass.id).or_default().insert(reader);
                        reverse_edges.entry(reader).or_default().insert(pass.id);
                        barriers.write_after_read = barriers.write_after_read.saturating_add(1);
                    }
                }
            }
            last_writer.insert(write.resource, pass.id);
        }
    }

    let mut indegree: BTreeMap<RenderGraphPassId, usize> = dependencies
        .iter()
        .map(|(pass, deps)| (*pass, deps.len()))
        .collect();
    let mut ready: VecDeque<RenderGraphPassId> = indegree
        .iter()
        .filter_map(|(pass, count)| (*count == 0).then_some(*pass))
        .collect();
    let mut execution_order = Vec::with_capacity(graph.passes.len());

    while let Some(pass) = ready.pop_front() {
        execution_order.push(pass);
        let Some(edges) = reverse_edges.get(&pass) else {
            continue;
        };
        for next in edges {
            let Some(count) = indegree.get_mut(next) else {
                continue;
            };
            *count = count.saturating_sub(1);
            if *count == 0 {
                let pos = ready
                    .iter()
                    .position(|queued| queued > next)
                    .unwrap_or(ready.len());
                ready.insert(pos, *next);
            }
        }
    }

    if execution_order.len() != graph.passes.len() {
        return Err(vec![RenderGraphValidationIssue::new(
            "render_graph.cycle",
            "render graph has a dependency cycle",
        )]);
    }

    Ok(RenderGraphCompileReport {
        pass_count: graph.passes.len() as u32,
        resource_count: graph.resources.len() as u32,
        execution_order,
        lifetime,
        barriers,
        warnings,
    })
}
