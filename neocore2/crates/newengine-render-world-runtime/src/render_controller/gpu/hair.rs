use newengine_core::render::{
    BindGroupDesc, BindGroupId, BindGroupLayoutDesc, BindGroupLayoutId, BindingKind, BufferBinding,
    BufferDesc, BufferId, BufferUsage, ComputePipelineDesc, DispatchArgs, DrawArgs, Extent2D,
    FilterMode, MemoryHint, PipelineBlendMode, PipelineDepthCompare, PipelineDepthMode,
    PipelineDesc, PipelineId, RasterCullMode, RectI32, RenderApi, RenderDrawListKind,
    RenderGraphPassKind, SamplerDesc, SamplerId, ShaderDesc, ShaderId, ShaderSourceKind,
    ShaderStage, TextureFormat, TextureId, Viewport,
};
use newengine_core::{EngineError, EngineResult};
use newengine_ecs::World;
use newengine_math::{Mat4, Vec3};
use newengine_render_api::{
    HairCollisionMode, HairGroomAssetV1, HairGroomRegistryV1, HairQualityTier, HairSceneV1,
    HairShaderSetV1, HairSimulationMode, HairSkinPoseRegistryV1, RenderBackendCapabilities,
    RenderFeature,
};
use newengine_render_feature_api::{ShadowFrame, MAX_DIRECTIONAL_SHADOW_CASCADES};

mod helpers;
use helpers::*;

#[path = "hair/resources.rs"]
mod resources;
#[path = "hair/shadows.rs"]
mod shadows;

const HAIR_POINT_CAPACITY: usize = 131_072;
const HAIR_STRAND_CAPACITY: usize = 32_768;
const HAIR_RENDER_SEGMENT_CAPACITY: usize = 262_144;
const HAIR_INSTANCE_CAPACITY: usize = 1_024;
const HAIR_INSTANCE_SLOT_COUNT: usize = 4;
const HAIR_SLOT_BYTES: usize = 64;
const HAIR_FRAME_UBO_BYTES: u64 = 544;
const HAIR_SHADOW_UBO_BYTES: u64 = 112;
const HAIR_FRAME_SLOTS: usize = 4;
const HAIR_SHADOW_UBO_SLOTS: usize = HAIR_FRAME_SLOTS * MAX_DIRECTIONAL_SHADOW_CASCADES;
const HAIR_WORKGROUP_SIZE: u32 = 64;

const POINT_A_BASE: usize = 0;
const POINT_B_BASE: usize = POINT_A_BASE + HAIR_POINT_CAPACITY;
const STRAND_BASE: usize = POINT_B_BASE + HAIR_POINT_CAPACITY;
const SEGMENT_BASE: usize = STRAND_BASE + HAIR_STRAND_CAPACITY;
const INSTANCE_BASE: usize = SEGMENT_BASE + HAIR_RENDER_SEGMENT_CAPACITY;
const HAIR_COLLISION_CAPACITY: usize = 8_192;
const HAIR_SKIN_MATRIX_CAPACITY: usize = 65_536;
const CAPSULE_BASE: usize = INSTANCE_BASE + HAIR_INSTANCE_CAPACITY * HAIR_INSTANCE_SLOT_COUNT;
const SKIN_MATRIX_BASE: usize = CAPSULE_BASE + HAIR_COLLISION_CAPACITY;
const HAIR_SLOT_CAPACITY: usize = SKIN_MATRIX_BASE + HAIR_SKIN_MATRIX_CAPACITY;
const HAIR_SSBO_BYTES: u64 = (HAIR_SLOT_CAPACITY * HAIR_SLOT_BYTES) as u64;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct HairGpuFrameReport {
    pub(crate) active_instances: u32,
    pub(crate) guide_points: u32,
    pub(crate) guide_strands: u32,
    pub(crate) rendered_segments: u32,
    pub(crate) shadow_cascades: u32,
    pub(crate) shadow_segments: u32,
    pub(crate) topology_uploads: u32,
}

#[derive(Clone, Copy, Debug, Default)]
struct HairTopologyCounts {
    point_count: usize,
    strand_count: usize,
    render_segment_count: usize,
    rendered_strand_count: usize,
}

#[derive(Clone, Copy, Debug, Default)]
struct HairSlot([f32; 16]);

impl HairSlot {
    #[inline]
    fn from_matrix(matrix: [f32; 16]) -> Self {
        Self(matrix)
    }

    #[inline]
    fn from_lanes(a: [f32; 4], b: [f32; 4], c: [f32; 4], d: [f32; 4]) -> Self {
        let mut values = [0.0; 16];
        values[0..4].copy_from_slice(&a);
        values[4..8].copy_from_slice(&b);
        values[8..12].copy_from_slice(&c);
        values[12..16].copy_from_slice(&d);
        Self(values)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct HairInstanceGpuRanges {
    palette_offset: usize,
    palette_count: usize,
    capsule_offset: usize,
    capsule_count: usize,
}

struct HairCpuTopology {
    points: Vec<HairSlot>,
    strands: Vec<HairSlot>,
    render_segments: Vec<HairSlot>,
    capsules: Vec<HairSlot>,
    instance_ranges: Vec<HairInstanceGpuRanges>,
    counts: HairTopologyCounts,
}

pub(crate) struct HairGpuRenderer {
    layout: Option<BindGroupLayoutId>,
    shadow_layout: Option<BindGroupLayoutId>,
    state_buffer: Option<BufferId>,
    frame_ubos: [Option<BufferId>; HAIR_FRAME_SLOTS],
    bind_groups: [Option<BindGroupId>; HAIR_FRAME_SLOTS],
    shadow_ubos: [Option<BufferId>; HAIR_SHADOW_UBO_SLOTS],
    shadow_bind_groups: [Option<BindGroupId>; HAIR_SHADOW_UBO_SLOTS],
    shadow_sampler: Option<SamplerId>,
    bound_shadow_texture: Option<TextureId>,
    compute_shader: Option<ShaderId>,
    vertex_shader: Option<ShaderId>,
    fragment_shader: Option<ShaderId>,
    shadow_vertex_shader: Option<ShaderId>,
    shadow_fragment_shader: Option<ShaderId>,
    compute_pipeline: Option<PipelineId>,
    shadow_pipeline: Option<PipelineId>,
    graphics_pipelines: Vec<(TextureFormat, PipelineId)>,
    shader_set: Option<HairShaderSetV1>,
    topology_key: Option<u64>,
    counts: HairTopologyCounts,
    instance_ranges: Vec<HairInstanceGpuRanges>,
    read_point_base: usize,
    write_point_base: usize,
    backend_supported: bool,
    backend_skinning_supported: bool,
    backend_capsules_supported: bool,
    backend_shadows_supported: bool,
}

impl Default for HairGpuRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl HairGpuRenderer {
    #[inline]
    pub(crate) fn new() -> Self {
        Self {
            layout: None,
            shadow_layout: None,
            state_buffer: None,
            frame_ubos: [None; HAIR_FRAME_SLOTS],
            bind_groups: [None; HAIR_FRAME_SLOTS],
            shadow_ubos: [None; HAIR_SHADOW_UBO_SLOTS],
            shadow_bind_groups: [None; HAIR_SHADOW_UBO_SLOTS],
            shadow_sampler: None,
            bound_shadow_texture: None,
            compute_shader: None,
            vertex_shader: None,
            fragment_shader: None,
            shadow_vertex_shader: None,
            shadow_fragment_shader: None,
            compute_pipeline: None,
            shadow_pipeline: None,
            graphics_pipelines: Vec::new(),
            shader_set: None,
            topology_key: None,
            counts: HairTopologyCounts::default(),
            instance_ranges: Vec::new(),
            read_point_base: POINT_A_BASE,
            write_point_base: POINT_B_BASE,
            backend_supported: false,
            backend_skinning_supported: false,
            backend_capsules_supported: false,
            backend_shadows_supported: false,
        }
    }

    pub(crate) fn apply_backend_capabilities(&mut self, capabilities: &RenderBackendCapabilities) {
        self.backend_supported = capabilities.supports(RenderFeature::StorageBuffers)
            && capabilities.supports(RenderFeature::HairStrands)
            && capabilities.supports(RenderFeature::HairGpuSimulation)
            && capabilities.limits.max_storage_buffer_range >= HAIR_SSBO_BYTES;
        self.backend_skinning_supported = capabilities.supports(RenderFeature::HairSkinning);
        self.backend_capsules_supported =
            capabilities.supports(RenderFeature::HairCollisionCapsules);
        self.backend_shadows_supported = capabilities.supports(RenderFeature::HairShadows)
            && capabilities.supports(RenderFeature::Shadows)
            && capabilities.supports(RenderFeature::CascadedShadowMaps);
    }

    #[inline]
    pub(crate) fn scene_ready(&self, world: &World) -> bool {
        if !self.backend_supported {
            return false;
        }
        let Some(scene) = world.resource::<HairSceneV1>() else {
            return false;
        };
        if !scene.is_active() {
            return false;
        }
        let Some(registry) = world.resource::<HairGroomRegistryV1>() else {
            return false;
        };
        let poses = world.resource::<HairSkinPoseRegistryV1>();
        scene_bindings_ready(
            scene,
            registry,
            poses,
            self.backend_skinning_supported,
            self.backend_capsules_supported,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn record_frame(
        &mut self,
        r: &mut dyn RenderApi,
        world: &World,
        frame_index: u64,
        dt: f32,
        view_projection: Mat4,
        view: Mat4,
        camera_position: Vec3,
        camera_forward: Vec3,
        shadow_frame: ShadowFrame,
        shadow_extent: Extent2D,
        render_shadow_map: bool,
        color_format: TextureFormat,
        viewport_width: u32,
        viewport_height: u32,
        directional_dir_intensity: [f32; 4],
        directional_color: [f32; 4],
        ambient_color: [f32; 4],
    ) -> EngineResult<HairGpuFrameReport> {
        let Some(scene) = world.resource::<HairSceneV1>() else {
            return Ok(HairGpuFrameReport::default());
        };
        let Some(registry) = world.resource::<HairGroomRegistryV1>() else {
            return Ok(HairGpuFrameReport::default());
        };
        let poses = world.resource::<HairSkinPoseRegistryV1>();

        let mut scene = scene
            .clone()
            .normalized()
            .map_err(|error| EngineError::other(format!("hair scene rejected: {error}")))?;
        scene
            .instances
            .retain(|instance| instance.quality != HairQualityTier::Off);
        if scene.instances.is_empty() {
            return Ok(HairGpuFrameReport::default());
        }
        if scene.instances.len() > HAIR_INSTANCE_CAPACITY {
            return Err(EngineError::other(format!(
                "hair scene instance count {} exceeds GPU capacity {}",
                scene.instances.len(),
                HAIR_INSTANCE_CAPACITY
            )));
        }
        if !scene_bindings_ready(
            &scene,
            registry,
            poses,
            self.backend_skinning_supported,
            self.backend_capsules_supported,
        ) {
            return Err(EngineError::other(
                "hair scene has unresolved groom/skin/capsule bindings or unsupported backend features",
            ));
        }

        self.ensure_resources(r, color_format, &scene.shaders, shadow_frame.texture)?;
        let state_buffer = self
            .state_buffer
            .ok_or_else(|| EngineError::other("hair state SSBO missing after resource creation"))?;

        let pose_layout_generation = poses
            .map(HairSkinPoseRegistryV1::layout_generation)
            .unwrap_or(0);
        let topology_key =
            topology_key(&scene, registry.generation(), pose_layout_generation, poses);
        let mut topology_uploads = 0u32;
        if self.topology_key != Some(topology_key) {
            let topology = build_topology(&scene, registry, poses)?;
            self.upload_topology(r, state_buffer, &topology)?;
            self.counts = topology.counts;
            self.instance_ranges = topology.instance_ranges.clone();
            self.topology_key = Some(topology_key);
            self.read_point_base = POINT_A_BASE;
            self.write_point_base = POINT_B_BASE;
            topology_uploads = 1;
        }

        let instance_slots = build_instance_slots(&scene, &self.instance_ranges);
        r.write_buffer(
            state_buffer,
            (INSTANCE_BASE * HAIR_SLOT_BYTES) as u64,
            &slots_to_bytes(&instance_slots),
        )?;
        let palette_slots = build_skin_palette_slots(&scene, poses, &self.instance_ranges)?;
        if !palette_slots.is_empty() {
            r.write_buffer(
                state_buffer,
                (SKIN_MATRIX_BASE * HAIR_SLOT_BYTES) as u64,
                &slots_to_bytes(&palette_slots),
            )?;
        }

        let frame_slot = frame_index as usize % HAIR_FRAME_SLOTS;
        let frame_ubo = self.frame_ubos[frame_slot]
            .ok_or_else(|| EngineError::other("hair frame UBO missing"))?;
        let bind_group = self.bind_groups[frame_slot]
            .ok_or_else(|| EngineError::other("hair frame bind group missing"))?;
        let inverse_view = view.inverse();
        let camera_right = inverse_view.x_axis.truncate().normalize_or_zero();
        let camera_up = inverse_view.y_axis.truncate().normalize_or_zero();
        let frame_bytes = encode_frame_ubo(
            view_projection,
            camera_position,
            camera_right,
            camera_up,
            sanitize_dt(dt),
            directional_dir_intensity,
            directional_color,
            ambient_color,
            self.counts,
            self.read_point_base,
            self.write_point_base,
            camera_forward,
            shadow_frame,
            shadow_extent,
            render_shadow_map && self.backend_shadows_supported,
        );
        r.write_buffer(frame_ubo, 0, &frame_bytes)?;

        if self.counts.strand_count > 0 {
            let compute = self
                .compute_pipeline
                .ok_or_else(|| EngineError::other("hair compute pipeline missing"))?;
            r.begin_render_phase(RenderGraphPassKind::HairSimulation)?;
            r.set_pipeline(compute)?;
            r.set_bind_group(0, bind_group)?;
            r.dispatch(DispatchArgs::one_dimensional(
                (self.counts.strand_count as u32).div_ceil(HAIR_WORKGROUP_SIZE),
            ))?;
            r.end_render_phase()?;
        }

        let (shadow_cascades, shadow_segments) = self.record_directional_shadows(
            r,
            &scene,
            frame_slot,
            shadow_frame,
            shadow_extent,
            render_shadow_map,
            directional_dir_intensity,
        )?;

        if self.counts.render_segment_count > 0 {
            let graphics = self
                .graphics_pipeline(color_format)
                .ok_or_else(|| EngineError::other("hair strand graphics pipeline missing"))?;
            r.begin_draw_list(RenderDrawListKind::Transparent)?;
            let extent = newengine_core::render::Extent2D::new(
                viewport_width.max(1),
                viewport_height.max(1),
            );
            r.set_viewport(Viewport::full(extent))?;
            r.set_scissor(RectI32::new(
                0,
                0,
                viewport_width.max(1).min(i32::MAX as u32) as i32,
                viewport_height.max(1).min(i32::MAX as u32) as i32,
            ))?;
            r.set_pipeline(graphics)?;
            r.set_bind_group(0, bind_group)?;
            r.draw(DrawArgs {
                vertex_count: 6,
                instance_count: self.counts.render_segment_count.min(u32::MAX as usize) as u32,
                first_vertex: 0,
                first_instance: 0,
            })?;
            r.end_draw_list()?;
        }

        std::mem::swap(&mut self.read_point_base, &mut self.write_point_base);

        Ok(HairGpuFrameReport {
            active_instances: scene.instances.len() as u32,
            guide_points: self.counts.point_count as u32,
            guide_strands: self.counts.strand_count as u32,
            rendered_segments: self.counts.render_segment_count as u32,
            shadow_cascades,
            shadow_segments,
            topology_uploads,
        })
    }
}
#[cfg(test)]
#[path = "hair/tests.rs"]
mod tests;
