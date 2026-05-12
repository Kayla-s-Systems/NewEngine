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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderGraphPassKind {
    DepthPrepass,
    ShadowMap,
    GBuffer,
    DeferredLighting,
    ForwardOpaque,
    Transparent,
    Water,
    PostFx,
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
            usage,
            lifetime: RenderGraphResourceLifetime::External,
            extent: None,
            format: None,
            external: Some(RenderGraphExternalResource::Texture(texture)),
        }
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
            (Self::ShadowCasters, RenderGraphPassKind::ShadowMap) => true,
            (Self::OpaqueForward, RenderGraphPassKind::ForwardOpaque | RenderGraphPassKind::GBuffer) => true,
            (Self::Transparent, RenderGraphPassKind::Transparent) => true,
            (Self::Ui, RenderGraphPassKind::UiComposite) => true,
            (Self::Debug, RenderGraphPassKind::DebugOverlay) => true,
            _ => false,
        }
    }
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
            queue: RenderGraphQueueKind::Graphics,
            reads: Vec::new(),
            writes: Vec::new(),
            creates: Vec::new(),
            draw_lists: Vec::new(),
            flags: RenderGraphPassFlags::default(),
        }
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


#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RenderGraphDesc {
    pub label: Option<String>,
    #[serde(default)]
    pub frame_index: u64,
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
