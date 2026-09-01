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

    #[allow(clippy::too_many_arguments)]
    fn record_directional_shadows(
        &mut self,
        r: &mut dyn RenderApi,
        scene: &HairSceneV1,
        frame_slot: usize,
        shadow_frame: ShadowFrame,
        shadow_extent: Extent2D,
        render_shadow_map: bool,
        directional_dir_intensity: [f32; 4],
    ) -> EngineResult<(u32, u32)> {
        if !self.backend_shadows_supported
            || !render_shadow_map
            || !scene.shaders.has_shadows()
            || self.counts.render_segment_count == 0
            || shadow_frame.params[0] < 0.5
            || !scene
                .instances
                .iter()
                .any(|instance| instance.casts_shadows)
        {
            return Ok((0, 0));
        }
        let pipeline = self
            .shadow_pipeline
            .ok_or_else(|| EngineError::other("hair shadow pipeline missing"))?;
        let cascade_count = shadow_frame
            .cascade_count
            .clamp(1, MAX_DIRECTIONAL_SHADOW_CASCADES as u32) as usize;
        let phase = if cascade_count > 1 {
            RenderGraphPassKind::ShadowCascadeMap
        } else {
            RenderGraphPassKind::ShadowMap
        };
        let instance_count = self.counts.render_segment_count.min(u32::MAX as usize) as u32;

        for cascade_index in 0..cascade_count {
            let cascade = shadow_frame.cascade(cascade_index);
            let slot = frame_slot * MAX_DIRECTIONAL_SHADOW_CASCADES + cascade_index;
            let ubo = self.shadow_ubos[slot]
                .ok_or_else(|| EngineError::other("hair shadow UBO missing"))?;
            let bind_group = self.shadow_bind_groups[slot]
                .ok_or_else(|| EngineError::other("hair shadow bind group missing"))?;
            let bytes = encode_shadow_ubo(
                cascade.light_mvp,
                directional_dir_intensity,
                self.counts.render_segment_count,
                self.write_point_base,
                cascade_index,
            );
            r.write_buffer(ubo, 0, &bytes)?;

            r.begin_render_phase(phase)?;
            if cascade_count > 1 {
                r.set_viewport(cascade.viewport)?;
                r.set_scissor(cascade.scissor)?;
            } else {
                r.set_viewport(Viewport::full(Extent2D::new(
                    shadow_extent.width.max(1),
                    shadow_extent.height.max(1),
                )))?;
                r.set_scissor(RectI32::new(
                    0,
                    0,
                    shadow_extent.width.max(1).min(i32::MAX as u32) as i32,
                    shadow_extent.height.max(1).min(i32::MAX as u32) as i32,
                ))?;
            }
            r.set_pipeline(pipeline)?;
            r.set_bind_group(0, bind_group)?;
            r.draw(DrawArgs {
                vertex_count: 6,
                instance_count,
                first_vertex: 0,
                first_instance: 0,
            })?;
            r.end_render_phase()?;
        }

        Ok((
            cascade_count as u32,
            instance_count.saturating_mul(cascade_count as u32),
        ))
    }

    fn ensure_resources(
        &mut self,
        r: &mut dyn RenderApi,
        color_format: TextureFormat,
        shaders: &HairShaderSetV1,
        shadow_texture: TextureId,
    ) -> EngineResult<()> {
        let shaders = shaders
            .clone()
            .normalized()
            .map_err(|error| EngineError::other(format!("hair shader set rejected: {error}")))?;
        let layout = match self.layout {
            Some(layout) => layout,
            None => {
                let layout = r.create_bind_group_layout(
                    BindGroupLayoutDesc::new(vec![
                        BindingKind::UniformBuffer,
                        BindingKind::StorageBuffer,
                        BindingKind::Texture2D,
                        BindingKind::Sampler,
                    ])
                    .with_label("hair.guide_strands.layout"),
                )?;
                self.layout = Some(layout);
                layout
            }
        };
        let shadow_layout = match self.shadow_layout {
            Some(layout) => layout,
            None => {
                let layout = r.create_bind_group_layout(
                    BindGroupLayoutDesc::new(vec![
                        BindingKind::UniformBuffer,
                        BindingKind::StorageBuffer,
                    ])
                    .with_label("hair.strand_shadow.layout"),
                )?;
                self.shadow_layout = Some(layout);
                layout
            }
        };
        let state_buffer = match self.state_buffer {
            Some(buffer) => buffer,
            None => {
                let buffer = r.create_buffer(
                    BufferDesc::new(HAIR_SSBO_BYTES, BufferUsage::Storage, MemoryHint::GpuOnly)
                        .with_label("hair.guide_strands.state_ssbo"),
                )?;
                self.state_buffer = Some(buffer);
                buffer
            }
        };
        let shadow_sampler = match self.shadow_sampler {
            Some(sampler) => sampler,
            None => {
                let sampler = r.create_sampler(
                    SamplerDesc::default()
                        .with_label("hair.shadow_sampler")
                        .with_min_filter(FilterMode::Nearest)
                        .with_mag_filter(FilterMode::Nearest)
                        .with_mip_filter(FilterMode::Nearest),
                )?;
                self.shadow_sampler = Some(sampler);
                sampler
            }
        };

        if self.bound_shadow_texture != Some(shadow_texture) {
            for group in &mut self.bind_groups {
                if let Some(group) = group.take() {
                    r.destroy_bind_group(group);
                }
            }
            self.bound_shadow_texture = Some(shadow_texture);
        }

        for slot in 0..HAIR_FRAME_SLOTS {
            if self.frame_ubos[slot].is_none() {
                self.frame_ubos[slot] = Some(
                    r.create_buffer(
                        BufferDesc::new(
                            HAIR_FRAME_UBO_BYTES,
                            BufferUsage::Uniform,
                            MemoryHint::CpuToGpu,
                        )
                        .with_label(format!("hair.frame_ubo.{slot}")),
                    )?,
                );
            }
            if self.bind_groups[slot].is_none() {
                let ubo = self.frame_ubos[slot].expect("hair UBO created above");
                self.bind_groups[slot] = Some(
                    r.create_bind_group(
                        BindGroupDesc::new(layout)
                            .with_label(format!("hair.bind_group.{slot}"))
                            .with_uniform0(BufferBinding::new(ubo, 0, HAIR_FRAME_UBO_BYTES))
                            .with_storage0(BufferBinding::new(state_buffer, 0, HAIR_SSBO_BYTES))
                            .with_texture0(shadow_texture)
                            .with_sampler0(shadow_sampler),
                    )?,
                );
            }
        }

        if shaders.has_shadows() {
            for slot in 0..HAIR_SHADOW_UBO_SLOTS {
                if self.shadow_ubos[slot].is_none() {
                    self.shadow_ubos[slot] = Some(
                        r.create_buffer(
                            BufferDesc::new(
                                HAIR_SHADOW_UBO_BYTES,
                                BufferUsage::Uniform,
                                MemoryHint::CpuToGpu,
                            )
                            .with_label(format!("hair.shadow_ubo.{slot}")),
                        )?,
                    );
                }
                if self.shadow_bind_groups[slot].is_none() {
                    let ubo = self.shadow_ubos[slot].expect("hair shadow UBO created above");
                    self.shadow_bind_groups[slot] = Some(
                        r.create_bind_group(
                            BindGroupDesc::new(shadow_layout)
                                .with_label(format!("hair.shadow_bind_group.{slot}"))
                                .with_uniform0(BufferBinding::new(ubo, 0, HAIR_SHADOW_UBO_BYTES))
                                .with_storage0(BufferBinding::new(
                                    state_buffer,
                                    0,
                                    HAIR_SSBO_BYTES,
                                )),
                        )?,
                    );
                }
            }
        }

        if self.shader_set.as_ref() != Some(&shaders) {
            self.destroy_shader_pipelines(r);
            self.shader_set = Some(shaders.clone());
        }
        if self.compute_shader.is_none() {
            self.compute_shader = Some(
                r.create_shader(
                    ShaderDesc::from_asset(
                        ShaderStage::Compute,
                        "main",
                        shaders.simulation.clone(),
                        ShaderSourceKind::Glsl,
                    )
                    .with_label("hair.guide_simulation"),
                )?,
            );
        }
        if self.vertex_shader.is_none() {
            self.vertex_shader = Some(
                r.create_shader(
                    ShaderDesc::from_asset(
                        ShaderStage::Vertex,
                        "main",
                        shaders.strands_vertex.clone(),
                        ShaderSourceKind::Glsl,
                    )
                    .with_label("hair.strand_ribbon.vs"),
                )?,
            );
        }
        if self.fragment_shader.is_none() {
            self.fragment_shader = Some(
                r.create_shader(
                    ShaderDesc::from_asset(
                        ShaderStage::Fragment,
                        "main",
                        shaders.strands_fragment.clone(),
                        ShaderSourceKind::Glsl,
                    )
                    .with_label("hair.strand_ribbon.fs"),
                )?,
            );
        }
        if shaders.has_shadows() && self.shadow_vertex_shader.is_none() {
            let asset = shaders
                .shadow_vertex
                .clone()
                .ok_or_else(|| EngineError::other("hair shadow vertex shader missing"))?;
            self.shadow_vertex_shader = Some(
                r.create_shader(
                    ShaderDesc::from_asset(
                        ShaderStage::Vertex,
                        "main",
                        asset,
                        ShaderSourceKind::Glsl,
                    )
                    .with_label("hair.strand_shadow.vs"),
                )?,
            );
        }
        if shaders.has_shadows() && self.shadow_fragment_shader.is_none() {
            let asset = shaders
                .shadow_fragment
                .clone()
                .ok_or_else(|| EngineError::other("hair shadow fragment shader missing"))?;
            self.shadow_fragment_shader = Some(
                r.create_shader(
                    ShaderDesc::from_asset(
                        ShaderStage::Fragment,
                        "main",
                        asset,
                        ShaderSourceKind::Glsl,
                    )
                    .with_label("hair.strand_shadow.fs"),
                )?,
            );
        }

        if self.compute_pipeline.is_none() {
            self.compute_pipeline = Some(
                r.create_compute_pipeline(
                    ComputePipelineDesc::new(
                        self.compute_shader
                            .expect("hair compute shader created immediately above"),
                    )
                    .with_label("hair.guide_simulation")
                    .with_bind_group_layouts(vec![layout])
                    .with_cache_key(format!(
                        "hair.guide_simulation.v1.{:016x}",
                        shader_set_key(&shaders)
                    )),
                )?,
            );
        }
        if shaders.has_shadows() && self.shadow_pipeline.is_none() {
            self.shadow_pipeline = Some(
                r.create_pipeline(
                    PipelineDesc::new(
                        self.shadow_vertex_shader
                            .expect("hair shadow vertex shader created immediately above"),
                        self.shadow_fragment_shader
                            .expect("hair shadow fragment shader created immediately above"),
                        TextureFormat::R32Float,
                    )
                    .with_label("hair.strand_shadow")
                    .with_bind_group_layouts(vec![shadow_layout])
                    .with_depth_state(
                        TextureFormat::Depth32Float,
                        PipelineDepthMode::new(true, true, PipelineDepthCompare::LessOrEqual),
                    )
                    .with_cull_mode(RasterCullMode::None)
                    .with_blend_mode(PipelineBlendMode::Opaque)
                    .with_cache_key(format!(
                        "hair.strand_shadow.v1.{:016x}",
                        shader_set_key(&shaders)
                    )),
                )?,
            );
        }

        if self.graphics_pipeline(color_format).is_none() {
            let pipeline = r.create_pipeline(
                PipelineDesc::new(
                    self.vertex_shader
                        .expect("hair vertex shader created immediately above"),
                    self.fragment_shader
                        .expect("hair fragment shader created immediately above"),
                    color_format,
                )
                .with_label("hair.strand_ribbon")
                .with_bind_group_layouts(vec![layout])
                .with_depth_state(
                    TextureFormat::Depth32Float,
                    PipelineDepthMode::new(true, false, PipelineDepthCompare::LessOrEqual),
                )
                .with_cull_mode(RasterCullMode::None)
                .with_blend_mode(PipelineBlendMode::Alpha)
                .with_cache_key(format!(
                    "hair.strand_ribbon.v1.{:016x}.{color_format:?}",
                    shader_set_key(&shaders)
                )),
            )?;
            self.graphics_pipelines.push((color_format, pipeline));
        }
        Ok(())
    }

    fn destroy_shader_pipelines(&mut self, r: &mut dyn RenderApi) {
        for (_, pipeline) in self.graphics_pipelines.drain(..) {
            r.destroy_pipeline(pipeline);
        }
        if let Some(pipeline) = self.shadow_pipeline.take() {
            r.destroy_pipeline(pipeline);
        }
        if let Some(pipeline) = self.compute_pipeline.take() {
            r.destroy_pipeline(pipeline);
        }
        if let Some(shader) = self.compute_shader.take() {
            r.destroy_shader(shader);
        }
        if let Some(shader) = self.vertex_shader.take() {
            r.destroy_shader(shader);
        }
        if let Some(shader) = self.fragment_shader.take() {
            r.destroy_shader(shader);
        }
        if let Some(shader) = self.shadow_vertex_shader.take() {
            r.destroy_shader(shader);
        }
        if let Some(shader) = self.shadow_fragment_shader.take() {
            r.destroy_shader(shader);
        }
    }

    fn upload_topology(
        &mut self,
        r: &mut dyn RenderApi,
        state_buffer: BufferId,
        topology: &HairCpuTopology,
    ) -> EngineResult<()> {
        let point_bytes = slots_to_bytes(&topology.points);
        r.write_buffer(
            state_buffer,
            (POINT_A_BASE * HAIR_SLOT_BYTES) as u64,
            &point_bytes,
        )?;
        r.write_buffer(
            state_buffer,
            (POINT_B_BASE * HAIR_SLOT_BYTES) as u64,
            &point_bytes,
        )?;
        r.write_buffer(
            state_buffer,
            (STRAND_BASE * HAIR_SLOT_BYTES) as u64,
            &slots_to_bytes(&topology.strands),
        )?;
        r.write_buffer(
            state_buffer,
            (SEGMENT_BASE * HAIR_SLOT_BYTES) as u64,
            &slots_to_bytes(&topology.render_segments),
        )?;
        if !topology.capsules.is_empty() {
            r.write_buffer(
                state_buffer,
                (CAPSULE_BASE * HAIR_SLOT_BYTES) as u64,
                &slots_to_bytes(&topology.capsules),
            )?;
        }
        Ok(())
    }

    #[inline]
    fn graphics_pipeline(&self, format: TextureFormat) -> Option<PipelineId> {
        self.graphics_pipelines
            .iter()
            .find_map(|(candidate, pipeline)| (*candidate == format).then_some(*pipeline))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_render_api::{
        HairGroomAssetV1, HairGroomRef, HairGuidePointV1, HairGuideStrandV1,
        HairSimulationSettingsV1,
    };

    fn tiny_scene_and_registry() -> (HairSceneV1, HairGroomRegistryV1) {
        let groom_ref = HairGroomRef::new("characters/test/hair.groom");
        let mut registry = HairGroomRegistryV1::default();
        registry
            .insert(HairGroomAssetV1 {
                groom: groom_ref.clone(),
                guide_points: vec![
                    HairGuidePointV1 {
                        rest_position: [0.0, 0.0, 0.0],
                        inverse_mass: 0.0,
                    },
                    HairGuidePointV1 {
                        rest_position: [0.0, -0.1, 0.0],
                        inverse_mass: 1.0,
                    },
                    HairGuidePointV1 {
                        rest_position: [0.0, -0.2, 0.0],
                        inverse_mass: 1.0,
                    },
                ],
                guide_strands: vec![HairGuideStrandV1 {
                    first_point: 0,
                    point_count: 3,
                    group: 0,
                    root_uv: [0.5, 0.5],
                    root_joint_index: 0,
                }],
                collision_capsules: Vec::new(),
                follow_strands_per_guide: 2,
            })
            .unwrap();
        let mut instance = newengine_render_api::HairInstanceDescV1 {
            instance_id: 1,
            groom: groom_ref,
            simulation: HairSimulationSettingsV1::default(),
            ..Default::default()
        };
        instance.material.strand_width_mm = 0.08;
        let mut scene = HairSceneV1::new(HairShaderSetV1::new(
            "shaders/hair/guide_sim.comp",
            "shaders/hair/strand_ribbon.vert",
            "shaders/hair/strand_ribbon.frag",
        ));
        scene.instances.push(instance);
        (scene, registry)
    }

    #[test]
    fn backend_must_explicitly_negotiate_hair_compute() {
        let mut renderer = HairGpuRenderer::new();
        let mut caps = RenderBackendCapabilities::raster_default();
        renderer.apply_backend_capabilities(&caps);
        assert!(!renderer.backend_supported);

        caps.features.push(RenderFeature::HairStrands);
        caps.features.push(RenderFeature::HairGpuSimulation);
        renderer.apply_backend_capabilities(&caps);
        assert!(renderer.backend_supported);
        assert!(!renderer.backend_shadows_supported);

        caps.features.push(RenderFeature::HairShadows);
        renderer.apply_backend_capabilities(&caps);
        assert!(renderer.backend_shadows_supported);

        caps.limits.max_storage_buffer_range = HAIR_SSBO_BYTES - 1;
        renderer.apply_backend_capabilities(&caps);
        assert!(!renderer.backend_supported);
    }

    fn f32_at(bytes: &[u8], index: usize) -> f32 {
        let offset = index * 4;
        f32::from_ne_bytes(bytes[offset..offset + 4].try_into().unwrap())
    }

    fn test_shadow_frame() -> ShadowFrame {
        let mut frame = ShadowFrame::disabled(TextureId::new(1));
        frame.params = [1.0, 0.0025, 0.85, 1.0];
        frame.cascade_count = 1;
        frame.cascade_splits = [48.0, 48.0, 48.0, 48.0];
        frame.cascades[0].light_mvp = Mat4::orthographic_rh(-8.0, 8.0, -8.0, 8.0, 0.1, 64.0);
        frame.cascade_light_mvp[0] = frame.cascades[0].light_mvp;
        frame
    }

    #[test]
    fn hair_shadow_ubo_uses_current_simulation_write_buffer() {
        let bytes = encode_shadow_ubo(Mat4::IDENTITY, [0.0, -1.0, 0.0, 3.0], 321, POINT_B_BASE, 2);
        assert_eq!(bytes.len(), HAIR_SHADOW_UBO_BYTES as usize);
        assert_eq!(f32_at(&bytes, 20), 321.0);
        assert_eq!(f32_at(&bytes, 21), POINT_B_BASE as f32);
        assert_eq!(f32_at(&bytes, 22), SEGMENT_BASE as f32);
        assert_eq!(f32_at(&bytes, 23), INSTANCE_BASE as f32);
        assert_eq!(f32_at(&bytes, 24), HAIR_INSTANCE_SLOT_COUNT as f32);
        assert_eq!(f32_at(&bytes, 25), 2.0);
    }

    #[test]
    fn hair_frame_shadow_payload_is_append_only_and_texel_scaled() {
        let frame = test_shadow_frame();
        let bias = hair_shadow_receiver_bias(frame, 0, Extent2D::new(2048, 2048));
        assert!(bias.is_finite());
        assert!((0.000002..=0.002).contains(&bias));

        let bytes = encode_frame_ubo(
            Mat4::IDENTITY,
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::X,
            Vec3::Y,
            1.0 / 60.0,
            [0.0, -1.0, 0.0, 2.0],
            [1.0; 4],
            [0.1; 4],
            HairTopologyCounts {
                point_count: 10,
                strand_count: 2,
                render_segment_count: 8,
                rendered_strand_count: 4,
            },
            POINT_A_BASE,
            POINT_B_BASE,
            Vec3::new(0.0, 0.0, -1.0),
            frame,
            Extent2D::new(2048, 2048),
            true,
        );
        assert_eq!(bytes.len(), HAIR_FRAME_UBO_BYTES as usize);
        // Original V1 prefix remains byte/float aligned through index 51.
        assert_eq!(f32_at(&bytes, 40), 10.0);
        assert_eq!(f32_at(&bytes, 44), POINT_A_BASE as f32);
        assert_eq!(f32_at(&bytes, 45), POINT_B_BASE as f32);
        assert_eq!(f32_at(&bytes, 48), INSTANCE_BASE as f32);
        // CSM payload starts only after the old prefix.
        assert_eq!(f32_at(&bytes, 52), 1.0);
        assert_eq!(f32_at(&bytes, 53), 1.0);
        assert_eq!(f32_at(&bytes, 56), 2048.0);
        assert!((f32_at(&bytes, 64) - bias).abs() < 1.0e-8);
        assert_eq!(f32_at(&bytes, 70), -1.0);
    }

    #[test]
    fn shadow_shader_pair_changes_pipeline_cache_identity() {
        let base = HairShaderSetV1::new(
            "shaders/hair/guide_sim.comp",
            "shaders/hair/strand_ribbon.vert",
            "shaders/hair/strand_ribbon.frag",
        );
        let shadowed = base.clone().with_shadows(
            "shaders/hair/strand_shadow.vert",
            "shaders/hair/strand_shadow.frag",
        );
        assert_ne!(shader_set_key(&base), shader_set_key(&shadowed));
    }

    #[test]
    fn topology_expands_followers_without_duplicating_simulation_points() {
        let (scene, registry) = tiny_scene_and_registry();
        let topology = build_topology(&scene, &registry, None).unwrap();
        assert_eq!(topology.counts.point_count, 3);
        assert_eq!(topology.counts.strand_count, 1);
        assert_eq!(topology.counts.rendered_strand_count, 3);
        assert_eq!(topology.counts.render_segment_count, 6);
    }

    #[test]
    fn instance_record_is_four_std430_slots() {
        let (scene, registry) = tiny_scene_and_registry();
        let topology = build_topology(&scene, &registry, None).unwrap();
        let slots = build_instance_slots(&scene, &topology.instance_ranges);
        assert_eq!(slots.len(), HAIR_INSTANCE_SLOT_COUNT);
        assert_eq!(
            slots_to_bytes(&slots).len(),
            HAIR_INSTANCE_SLOT_COUNT * HAIR_SLOT_BYTES
        );
    }

    #[test]
    fn topology_uses_skin_pose_without_hashing_animation_revision() {
        let (mut scene, mut registry) = tiny_scene_and_registry();
        let mut groom = registry
            .get(&scene.instances[0].groom)
            .cloned()
            .expect("test groom");
        groom.guide_strands[0].root_joint_index = 1;
        registry.insert(groom).unwrap();
        scene.instances[0].skin_pose_id = Some(7);

        let identity = Mat4::IDENTITY.to_cols_array();
        let translated = Mat4::from_translation(Vec3::new(1.0, 0.0, 0.0)).to_cols_array();
        let mut poses = HairSkinPoseRegistryV1::default();
        poses
            .upsert(newengine_render_api::HairSkinPoseV1 {
                pose_id: 7,
                revision: 1,
                joint_deforms: vec![identity, translated],
            })
            .unwrap();
        let topology = build_topology(&scene, &registry, Some(&poses)).unwrap();
        assert!((topology.points[0].0[0] - 1.0).abs() < 1.0e-5);
        assert_eq!(topology.instance_ranges[0].palette_count, 2);
        assert_eq!(
            build_skin_palette_slots(&scene, Some(&poses), &topology.instance_ranges)
                .unwrap()
                .len(),
            2
        );
        let key_v1 = topology_key(
            &scene,
            registry.generation(),
            poses.layout_generation(),
            Some(&poses),
        );
        poses
            .upsert(newengine_render_api::HairSkinPoseV1 {
                pose_id: 7,
                revision: 2,
                joint_deforms: vec![
                    identity,
                    Mat4::from_translation(Vec3::new(2.0, 0.0, 0.0)).to_cols_array(),
                ],
            })
            .unwrap();
        let key_v2 = topology_key(
            &scene,
            registry.generation(),
            poses.layout_generation(),
            Some(&poses),
        );
        assert_eq!(
            key_v1, key_v2,
            "animation matrix changes must not rebuild topology"
        );
    }

    #[test]
    fn topology_key_ignores_pose_but_tracks_groom_generation() {
        let (mut scene, registry) = tiny_scene_and_registry();
        let before = topology_key(&scene, registry.generation(), 0, None);
        scene.instances[0].root_transform[12] = 10.0;
        let moved = topology_key(&scene, registry.generation(), 0, None);
        assert_eq!(before, moved);
        assert_ne!(
            before,
            topology_key(&scene, registry.generation() + 1, 0, None)
        );
    }
}
