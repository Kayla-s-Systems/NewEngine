use crate::{
    BufferId, DrawArgs, DrawIndexedArgs, PipelineDesc, PipelineId, RectI32, RenderWorkBudget,
    TextureDesc, TextureId, UploadPumpReport, Viewport,
};
use serde::{Deserialize, Serialize};

/// Opaque logical resource id inside a render graph.
///
/// This is intentionally separate from backend/native ids. A graph resource can be
/// transient, persistent or imported from an existing Render API resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RenderGraphResourceId(pub u64);

impl RenderGraphResourceId {
    #[inline]
    pub const fn new(id: u64) -> Self {
        Self(id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RenderGraphPassId(pub u64);

impl RenderGraphPassId {
    #[inline]
    pub const fn new(id: u64) -> Self {
        Self(id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderGraphResourceLifetime {
    /// Backing memory survives across frames and is owned by the renderer/backend.
    Persistent,
    /// Backing memory is valid only for one frame and may be aliased.
    TransientFrame,
    /// Backing memory survives for a bounded frame window.
    Frames(u32),
    /// Existing resource imported from outside the graph. The graph never owns it.
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
    TransferSrc,
    TransferDst,
    VertexBuffer,
    IndexBuffer,
    UniformBuffer,
    StorageBuffer,
    IndirectBuffer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RenderGraphImportedResource {
    Texture(TextureId),
    Buffer(BufferId),
    SwapchainColor,
    SwapchainDepth,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RenderGraphResourceKind {
    Texture(TextureDesc),
    Buffer { size: u64, usage: RenderGraphResourceUsage },
    Imported(RenderGraphImportedResource),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderGraphResourceDesc {
    pub id: RenderGraphResourceId,
    pub label: Option<String>,
    pub kind: RenderGraphResourceKind,
    pub lifetime: RenderGraphResourceLifetime,
    #[serde(default)]
    pub allow_aliasing: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderGraphQueueClass {
    Graphics,
    Compute,
    Transfer,
}

impl Default for RenderGraphQueueClass {
    #[inline]
    fn default() -> Self {
        Self::Graphics
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderGraphAccessKind {
    Read,
    Write,
    ReadWrite,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderGraphResourceAccess {
    pub resource: RenderGraphResourceId,
    pub usage: RenderGraphResourceUsage,
    pub access: RenderGraphAccessKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RenderGraphCommand {
    /// Begin a backend render pass over graph resources.
    BeginRenderPass {
        color: Vec<RenderGraphResourceId>,
        depth: Option<RenderGraphResourceId>,
        clear_color: Option<[f32; 4]>,
        clear_depth: Option<f32>,
    },
    EndRenderPass,
    SetViewport(Viewport),
    SetScissor(RectI32),
    SetPipeline(PipelineId),
    Draw(DrawArgs),
    DrawIndexed(DrawIndexedArgs),
    CopyBufferToTexture {
        src: RenderGraphResourceId,
        dst: RenderGraphResourceId,
        bytes: u64,
    },
    CopyBuffer {
        src: RenderGraphResourceId,
        dst: RenderGraphResourceId,
        bytes: u64,
    },
    /// Backend may compile lazily and cache by the pipeline desc hash.
    UseDynamicPipeline(PipelineDesc),
    /// Placeholder node for backends that record native commands through an adapter.
    ExternalMarker { label: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderGraphPassDesc {
    pub id: RenderGraphPassId,
    pub label: String,
    #[serde(default)]
    pub queue: RenderGraphQueueClass,
    #[serde(default)]
    pub reads: Vec<RenderGraphResourceAccess>,
    #[serde(default)]
    pub writes: Vec<RenderGraphResourceAccess>,
    #[serde(default)]
    pub commands: Vec<RenderGraphCommand>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderGraphDesc {
    pub label: String,
    #[serde(default)]
    pub frame_index: u64,
    #[serde(default)]
    pub resources: Vec<RenderGraphResourceDesc>,
    #[serde(default)]
    pub passes: Vec<RenderGraphPassDesc>,
    #[serde(default)]
    pub upload_budget: RenderWorkBudget,
    #[serde(default)]
    pub allow_transient_aliasing: bool,
}

impl Default for RenderGraphDesc {
    #[inline]
    fn default() -> Self {
        Self {
            label: String::new(),
            frame_index: 0,
            resources: Vec::new(),
            passes: Vec::new(),
            upload_budget: RenderWorkBudget::default(),
            allow_transient_aliasing: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct RenderGraphBarrierStats {
    pub texture_barriers: u32,
    pub buffer_barriers: u32,
    pub queue_ownership_transfers: u32,
    pub timeline_waits: u32,
    pub timeline_signals: u32,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct RenderGraphLifetimeStats {
    pub persistent: u32,
    pub transient: u32,
    pub external: u32,
    pub aliased: u32,
    pub retired: u32,
    pub destroyed: u32,
    pub estimated_transient_bytes: u64,
    pub estimated_aliasing_saved_bytes: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RenderGraphCompileReport {
    pub graph_label: String,
    pub frame_index: u64,
    pub pass_count: u32,
    pub resource_count: u32,
    pub execution_order: Vec<RenderGraphPassId>,
    pub lifetime: RenderGraphLifetimeStats,
    pub barriers: RenderGraphBarrierStats,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RenderGraphSubmitReport {
    pub compile: RenderGraphCompileReport,
    pub uploads: UploadPumpReport,
    pub executed_passes: u32,
    pub skipped_passes: u32,
    pub cpu_record_ms: f32,
    pub gpu_submit_ms: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderGraphValidationErrorKind {
    DuplicateResource,
    DuplicatePass,
    MissingResource,
    DependencyCycle,
    EmptyGraph,
    UnsupportedQueue,
    UnsupportedCommand,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderGraphValidationError {
    pub kind: RenderGraphValidationErrorKind,
    pub label: String,
    pub detail: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RenderGraphValidationReport {
    pub ok: bool,
    pub errors: Vec<RenderGraphValidationError>,
    pub compile: Option<RenderGraphCompileReport>,
}

impl RenderGraphValidationReport {
    #[inline]
    pub fn ok(compile: RenderGraphCompileReport) -> Self {
        Self { ok: true, errors: Vec::new(), compile: Some(compile) }
    }

    #[inline]
    pub fn err(errors: Vec<RenderGraphValidationError>) -> Self {
        Self { ok: false, errors, compile: None }
    }
}

#[inline]
fn estimated_resource_bytes(resource: &RenderGraphResourceDesc) -> u64 {
    match &resource.kind {
        RenderGraphResourceKind::Texture(desc) => {
            let bytes_per_pixel = match desc.format {
                crate::TextureFormat::Rgba16Float => 8_u64,
                crate::TextureFormat::Depth24Stencil8 | crate::TextureFormat::Depth32Float => 4_u64,
                _ => 4_u64,
            };
            u64::from(desc.extent.width)
                .saturating_mul(u64::from(desc.extent.height))
                .saturating_mul(bytes_per_pixel)
        }
        RenderGraphResourceKind::Buffer { size, .. } => *size,
        RenderGraphResourceKind::Imported(_) => 0,
    }
}

/// Validates a graph and produces a deterministic execution order from resource
/// producer/consumer dependencies. Backends can use this as a front-end before
/// converting the graph to native barriers/semaphores.
pub fn validate_and_compile_render_graph(graph: &RenderGraphDesc) -> RenderGraphValidationReport {
    use std::collections::{HashMap, HashSet, VecDeque};

    let mut errors = Vec::new();
    if graph.passes.is_empty() {
        errors.push(RenderGraphValidationError {
            kind: RenderGraphValidationErrorKind::EmptyGraph,
            label: graph.label.clone(),
            detail: "render graph has no passes".to_string(),
        });
    }

    let mut resources = HashMap::<RenderGraphResourceId, &RenderGraphResourceDesc>::new();
    for resource in &graph.resources {
        if resources.insert(resource.id, resource).is_some() {
            errors.push(RenderGraphValidationError {
                kind: RenderGraphValidationErrorKind::DuplicateResource,
                label: resource.label.clone().unwrap_or_else(|| format!("resource#{}", resource.id.0)),
                detail: "duplicate render graph resource id".to_string(),
            });
        }
    }

    let mut pass_ids = HashSet::<RenderGraphPassId>::new();
    for pass in &graph.passes {
        if !pass_ids.insert(pass.id) {
            errors.push(RenderGraphValidationError {
                kind: RenderGraphValidationErrorKind::DuplicatePass,
                label: pass.label.clone(),
                detail: "duplicate render graph pass id".to_string(),
            });
        }
        for access in pass.reads.iter().chain(pass.writes.iter()) {
            if !resources.contains_key(&access.resource) {
                errors.push(RenderGraphValidationError {
                    kind: RenderGraphValidationErrorKind::MissingResource,
                    label: pass.label.clone(),
                    detail: format!("pass references missing resource {}", access.resource.0),
                });
            }
        }
    }

    if !errors.is_empty() {
        return RenderGraphValidationReport::err(errors);
    }

    let mut producers = HashMap::<RenderGraphResourceId, RenderGraphPassId>::new();
    for pass in &graph.passes {
        for write in &pass.writes {
            producers.insert(write.resource, pass.id);
        }
    }

    let mut indegree = HashMap::<RenderGraphPassId, u32>::new();
    let mut edges = HashMap::<RenderGraphPassId, Vec<RenderGraphPassId>>::new();
    for pass in &graph.passes {
        indegree.insert(pass.id, 0);
        edges.insert(pass.id, Vec::new());
    }

    for pass in &graph.passes {
        for read in &pass.reads {
            if let Some(prod) = producers.get(&read.resource).copied() {
                if prod != pass.id {
                    edges.get_mut(&prod).expect("producer pass present").push(pass.id);
                    *indegree.get_mut(&pass.id).expect("consumer pass present") += 1;
                }
            }
        }
    }

    let mut ready: Vec<_> = indegree
        .iter()
        .filter_map(|(id, degree)| (*degree == 0).then_some(*id))
        .collect();
    ready.sort_by_key(|id| id.0);
    let mut queue: VecDeque<_> = ready.into();
    let mut order = Vec::with_capacity(graph.passes.len());

    while let Some(pass_id) = queue.pop_front() {
        order.push(pass_id);
        let mut next = edges.remove(&pass_id).unwrap_or_default();
        next.sort_by_key(|id| id.0);
        for child in next {
            let degree = indegree.get_mut(&child).expect("child pass present");
            *degree = degree.saturating_sub(1);
            if *degree == 0 {
                let pos = queue.iter().position(|id| id.0 > child.0).unwrap_or(queue.len());
                queue.insert(pos, child);
            }
        }
    }

    if order.len() != graph.passes.len() {
        return RenderGraphValidationReport::err(vec![RenderGraphValidationError {
            kind: RenderGraphValidationErrorKind::DependencyCycle,
            label: graph.label.clone(),
            detail: "render graph has a dependency cycle".to_string(),
        }]);
    }

    let mut lifetime = RenderGraphLifetimeStats::default();
    let mut transient_bytes = 0_u64;
    let mut aliasable_bytes = 0_u64;
    for resource in &graph.resources {
        match resource.lifetime {
            RenderGraphResourceLifetime::Persistent | RenderGraphResourceLifetime::Frames(_) => {
                lifetime.persistent = lifetime.persistent.saturating_add(1)
            }
            RenderGraphResourceLifetime::TransientFrame => {
                lifetime.transient = lifetime.transient.saturating_add(1);
                let bytes = estimated_resource_bytes(resource);
                transient_bytes = transient_bytes.saturating_add(bytes);
                if graph.allow_transient_aliasing && resource.allow_aliasing {
                    lifetime.aliased = lifetime.aliased.saturating_add(1);
                    aliasable_bytes = aliasable_bytes.saturating_add(bytes);
                }
            }
            RenderGraphResourceLifetime::External => {
                lifetime.external = lifetime.external.saturating_add(1)
            }
        }
    }
    lifetime.estimated_transient_bytes = transient_bytes;
    lifetime.estimated_aliasing_saved_bytes = aliasable_bytes;

    let mut barriers = RenderGraphBarrierStats::default();
    for pass in &graph.passes {
        for access in pass.reads.iter().chain(pass.writes.iter()) {
            match resources.get(&access.resource).map(|r| &r.kind) {
                Some(RenderGraphResourceKind::Texture(_) | RenderGraphResourceKind::Imported(RenderGraphImportedResource::Texture(_)) | RenderGraphResourceKind::Imported(RenderGraphImportedResource::SwapchainColor) | RenderGraphResourceKind::Imported(RenderGraphImportedResource::SwapchainDepth)) => {
                    barriers.texture_barriers = barriers.texture_barriers.saturating_add(1);
                }
                Some(RenderGraphResourceKind::Buffer { .. } | RenderGraphResourceKind::Imported(RenderGraphImportedResource::Buffer(_))) => {
                    barriers.buffer_barriers = barriers.buffer_barriers.saturating_add(1);
                }
                None => {}
            }
        }
        if !matches!(pass.queue, RenderGraphQueueClass::Graphics) {
            barriers.queue_ownership_transfers = barriers.queue_ownership_transfers.saturating_add(1);
        }
    }

    let compile = RenderGraphCompileReport {
        graph_label: graph.label.clone(),
        frame_index: graph.frame_index,
        pass_count: graph.passes.len() as u32,
        resource_count: graph.resources.len() as u32,
        execution_order: order,
        lifetime,
        barriers,
        notes: Vec::new(),
    };

    RenderGraphValidationReport::ok(compile)
}

#[inline]
pub fn compile_render_graph(graph: &RenderGraphDesc) -> Result<RenderGraphCompileReport, Vec<RenderGraphValidationError>> {
    let report = validate_and_compile_render_graph(graph);
    if report.ok {
        Ok(report.compile.expect("ok validation carries compile report"))
    } else {
        Err(report.errors)
    }
}
