use newengine_core::render::{
    BindGroupDesc, BindGroupId, BindGroupLayoutDesc, BindGroupLayoutId, BindingKind, BufferBinding,
    BufferDesc, BufferId, BufferUsage, ComputePipelineDesc, DispatchArgs, DrawArgs, MemoryHint,
    PipelineBlendMode, PipelineDepthCompare, PipelineDepthMode, PipelineDesc, PipelineId,
    RasterCullMode, RectI32, RenderApi, RenderDrawListKind, RenderGraphPassKind, ShaderDesc,
    ShaderId, ShaderSourceKind, ShaderStage, TextureFormat, Viewport,
};
use newengine_core::{EngineError, EngineResult};
use newengine_ecs::World;
use newengine_math::{Mat4, Vec3};
use newengine_vfx_api::{VfxGpuParticleBridge, VfxGpuParticleKind, VfxGpuParticleSpawnV1};

pub(super) const VFX_GPU_PARTICLE_CAPACITY: usize = 262_144;
const PARTICLE_SLOT_BYTES: usize = 96;
const PARTICLE_SSBO_BYTES: u64 = (VFX_GPU_PARTICLE_CAPACITY * PARTICLE_SLOT_BYTES) as u64;
const PARTICLE_FRAME_UBO_BYTES: u64 = 128;
const PARTICLE_FRAME_SLOTS: usize = 4;
const PARTICLE_WORKGROUP_SIZE: u32 = 64;
const MAX_SPAWN_UPLOADS_PER_FRAME: usize = 4_096;
const MAX_KILLS_PER_FRAME: usize = 4_096;
const PARTICLE_SIM_SHADER: &str = "shaders/vfx/particle_sim.comp";
const PARTICLE_VERTEX_SHADER: &str = "shaders/vfx/particle_billboard.vert";
const PARTICLE_FRAGMENT_SHADER: &str = "shaders/vfx/particle_billboard.frag";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct VfxGpuFrameReport {
    pub(crate) high_water: u32,
    pub(crate) uploaded_spawns: u32,
    pub(crate) killed_particles: u32,
    pub(crate) capacity_drops: u32,
}

pub(crate) struct VfxGpuRenderer {
    layout: Option<BindGroupLayoutId>,
    particle_buffer: Option<BufferId>,
    frame_ubos: [Option<BufferId>; PARTICLE_FRAME_SLOTS],
    bind_groups: [Option<BindGroupId>; PARTICLE_FRAME_SLOTS],
    compute_shader: Option<ShaderId>,
    vertex_shader: Option<ShaderId>,
    fragment_shader: Option<ShaderId>,
    compute_pipeline: Option<PipelineId>,
    graphics_pipelines: Vec<(TextureFormat, PipelineId)>,
    slot_deadlines: Vec<f64>,
    slot_instance_ids: Vec<u64>,
    next_slot: usize,
    high_water: usize,
    elapsed_seconds: f64,
    dropped_for_capacity: u64,
}

impl Default for VfxGpuRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl VfxGpuRenderer {
    #[inline]
    pub(crate) fn new() -> Self {
        Self {
            layout: None,
            particle_buffer: None,
            frame_ubos: [None; PARTICLE_FRAME_SLOTS],
            bind_groups: [None; PARTICLE_FRAME_SLOTS],
            compute_shader: None,
            vertex_shader: None,
            fragment_shader: None,
            compute_pipeline: None,
            graphics_pipelines: Vec::new(),
            slot_deadlines: vec![0.0; VFX_GPU_PARTICLE_CAPACITY],
            slot_instance_ids: vec![0; VFX_GPU_PARTICLE_CAPACITY],
            next_slot: 0,
            high_water: 0,
            elapsed_seconds: 0.0,
            dropped_for_capacity: 0,
        }
    }

    pub(crate) fn record_frame(
        &mut self,
        r: &mut dyn RenderApi,
        world: &World,
        frame_index: u64,
        dt: f32,
        view_projection: Mat4,
        view: Mat4,
        camera_position: Vec3,
        color_format: TextureFormat,
        viewport_width: u32,
        viewport_height: u32,
    ) -> EngineResult<VfxGpuFrameReport> {
        let dt = if dt.is_finite() {
            dt.clamp(0.0, 0.25)
        } else {
            0.0
        };
        self.elapsed_seconds += f64::from(dt);
        self.trim_expired_tail();

        let Some(bridge) = world.resource::<VfxGpuParticleBridge>() else {
            return Ok(VfxGpuFrameReport::default());
        };
        let pending = bridge.stats();
        if self.high_water == 0 && pending.pending_spawns == 0 && pending.pending_kills == 0 {
            return Ok(VfxGpuFrameReport::default());
        }

        newengine_ulog_api::ulog::error!("VFXDIAG ensure_resources begin");
        self.ensure_resources(r, color_format)?;
        newengine_ulog_api::ulog::error!("VFXDIAG ensure_resources done");
        let particle_buffer = self.particle_buffer.ok_or_else(|| {
            EngineError::other("VFX GPU particle buffer missing after resource creation")
        })?;

        newengine_ulog_api::ulog::error!("VFXDIAG process_kills begin");
        let killed_particles = self.process_kills(r, bridge, particle_buffer)?;
        newengine_ulog_api::ulog::error!("VFXDIAG process_kills done");
        newengine_ulog_api::ulog::error!("VFXDIAG process_spawns begin pending={}", pending.pending_spawns);
        let (uploaded_spawns, capacity_drops) = self.process_spawns(r, bridge, particle_buffer)?;
        newengine_ulog_api::ulog::error!("VFXDIAG process_spawns done uploaded={uploaded_spawns}");
        self.trim_expired_tail();

        if self.high_water == 0 {
            return Ok(VfxGpuFrameReport {
                uploaded_spawns,
                killed_particles,
                capacity_drops,
                ..VfxGpuFrameReport::default()
            });
        }

        let frame_slot = frame_index as usize % PARTICLE_FRAME_SLOTS;
        let frame_ubo = self.frame_ubos[frame_slot]
            .ok_or_else(|| EngineError::other("VFX GPU frame UBO missing"))?;
        let bind_group = self.bind_groups[frame_slot]
            .ok_or_else(|| EngineError::other("VFX GPU bind group missing"))?;
        let inverse_view = view.inverse();
        let camera_right = inverse_view.x_axis.truncate().normalize_or_zero();
        let camera_up = inverse_view.y_axis.truncate().normalize_or_zero();
        let frame_bytes = encode_frame_ubo(
            view_projection,
            camera_right,
            camera_up,
            camera_position,
            dt,
            self.high_water,
        );
        newengine_ulog_api::ulog::error!("VFXDIAG frame_ubo write begin");
        r.write_buffer(frame_ubo, 0, &frame_bytes)?;
        newengine_ulog_api::ulog::error!("VFXDIAG frame_ubo write done");

        let compute = self
            .compute_pipeline
            .ok_or_else(|| EngineError::other("VFX GPU compute pipeline missing"))?;
        newengine_ulog_api::ulog::error!("VFXDIAG compute begin_render_phase");
        r.begin_render_phase(RenderGraphPassKind::ParticleSimulation)?;
        newengine_ulog_api::ulog::error!("VFXDIAG compute set_pipeline");
        r.set_pipeline(compute)?;
        r.set_bind_group(0, bind_group)?;
        let groups = (self.high_water as u32).div_ceil(PARTICLE_WORKGROUP_SIZE);
        newengine_ulog_api::ulog::error!("VFXDIAG compute dispatch groups={groups}");
        r.dispatch(DispatchArgs::one_dimensional(groups))?;
        r.end_render_phase()?;
        newengine_ulog_api::ulog::error!("VFXDIAG compute end");

        let graphics = self
            .graphics_pipeline(color_format)
            .ok_or_else(|| EngineError::other("VFX GPU billboard pipeline missing"))?;
        newengine_ulog_api::ulog::error!("VFXDIAG graphics begin_draw_list");
        r.begin_draw_list(RenderDrawListKind::Transparent)?;
        let extent =
            newengine_core::render::Extent2D::new(viewport_width.max(1), viewport_height.max(1));
        r.set_viewport(Viewport::full(extent))?;
        r.set_scissor(RectI32::new(
            0,
            0,
            viewport_width.max(1).min(i32::MAX as u32) as i32,
            viewport_height.max(1).min(i32::MAX as u32) as i32,
        ))?;
        newengine_ulog_api::ulog::error!("VFXDIAG graphics set_pipeline");
        r.set_pipeline(graphics)?;
        r.set_bind_group(0, bind_group)?;
        newengine_ulog_api::ulog::error!("VFXDIAG graphics draw instances={}", self.high_water);
        r.draw(DrawArgs {
            vertex_count: 6,
            instance_count: self.high_water.min(u32::MAX as usize) as u32,
            first_vertex: 0,
            first_instance: 0,
        })?;
        r.end_draw_list()?;
        newengine_ulog_api::ulog::error!("VFXDIAG graphics end");

        Ok(VfxGpuFrameReport {
            high_water: self.high_water as u32,
            uploaded_spawns,
            killed_particles,
            capacity_drops,
        })
    }

    fn ensure_resources(
        &mut self,
        r: &mut dyn RenderApi,
        color_format: TextureFormat,
    ) -> EngineResult<()> {
        newengine_ulog_api::ulog::error!("VFXDIAG resource layout");
        let layout = match self.layout {
            Some(layout) => layout,
            None => {
                let layout = r.create_bind_group_layout(
                    BindGroupLayoutDesc::new(vec![
                        BindingKind::UniformBuffer,
                        BindingKind::StorageBuffer,
                    ])
                    .with_label("vfx.gpu_particles.layout"),
                )?;
                self.layout = Some(layout);
                layout
            }
        };

        newengine_ulog_api::ulog::error!("VFXDIAG resource particle_buffer");
        let particle_buffer = match self.particle_buffer {
            Some(buffer) => buffer,
            None => {
                let buffer = r.create_buffer(
                    BufferDesc::new(
                        PARTICLE_SSBO_BYTES,
                        BufferUsage::Storage,
                        MemoryHint::GpuOnly,
                    )
                    .with_label("vfx.gpu_particles.state_ssbo"),
                )?;
                self.particle_buffer = Some(buffer);
                buffer
            }
        };

        newengine_ulog_api::ulog::error!("VFXDIAG resource ubos_bindgroups");
        for slot in 0..PARTICLE_FRAME_SLOTS {
            if self.frame_ubos[slot].is_none() {
                self.frame_ubos[slot] = Some(
                    r.create_buffer(
                        BufferDesc::new(
                            PARTICLE_FRAME_UBO_BYTES,
                            BufferUsage::Uniform,
                            MemoryHint::CpuToGpu,
                        )
                        .with_label(format!("vfx.gpu_particles.frame_ubo.{slot}")),
                    )?,
                );
            }
            if self.bind_groups[slot].is_none() {
                let ubo = self.frame_ubos[slot].expect("VFX UBO created above");
                self.bind_groups[slot] = Some(
                    r.create_bind_group(
                        BindGroupDesc::new(layout)
                            .with_label(format!("vfx.gpu_particles.bind_group.{slot}"))
                            .with_uniform0(BufferBinding::new(ubo, 0, PARTICLE_FRAME_UBO_BYTES))
                            .with_storage0(BufferBinding::new(
                                particle_buffer,
                                0,
                                PARTICLE_SSBO_BYTES,
                            )),
                    )?,
                );
            }
        }

        newengine_ulog_api::ulog::error!("VFXDIAG resource compute_shader");
        if self.compute_shader.is_none() {
            self.compute_shader = Some(
                r.create_shader(
                    ShaderDesc::from_asset(
                        ShaderStage::Compute,
                        "main",
                        PARTICLE_SIM_SHADER,
                        ShaderSourceKind::Glsl,
                    )
                    .with_label("vfx.gpu_particles.sim"),
                )?,
            );
        }
        newengine_ulog_api::ulog::error!("VFXDIAG resource vertex_shader");
        if self.vertex_shader.is_none() {
            self.vertex_shader = Some(
                r.create_shader(
                    ShaderDesc::from_asset(
                        ShaderStage::Vertex,
                        "main",
                        PARTICLE_VERTEX_SHADER,
                        ShaderSourceKind::Glsl,
                    )
                    .with_label("vfx.gpu_particles.billboard.vs"),
                )?,
            );
        }
        newengine_ulog_api::ulog::error!("VFXDIAG resource fragment_shader");
        if self.fragment_shader.is_none() {
            self.fragment_shader = Some(
                r.create_shader(
                    ShaderDesc::from_asset(
                        ShaderStage::Fragment,
                        "main",
                        PARTICLE_FRAGMENT_SHADER,
                        ShaderSourceKind::Glsl,
                    )
                    .with_label("vfx.gpu_particles.billboard.fs"),
                )?,
            );
        }

        newengine_ulog_api::ulog::error!("VFXDIAG resource compute_pipeline");
        if self.compute_pipeline.is_none() {
            let shader = self.compute_shader.expect("compute shader created above");
            self.compute_pipeline = Some(
                r.create_compute_pipeline(
                    ComputePipelineDesc::new(shader)
                        .with_label("vfx.gpu_particles.compute")
                        .with_bind_group_layouts(vec![layout])
                        .with_cache_key("vfx.gpu_particles.compute.v1"),
                )?,
            );
        }

        newengine_ulog_api::ulog::error!("VFXDIAG resource graphics_pipeline format={color_format:?}");
        if self.graphics_pipeline(color_format).is_none() {
            let vs = self.vertex_shader.expect("vertex shader created above");
            let fs = self.fragment_shader.expect("fragment shader created above");
            let pipeline = r.create_pipeline(
                PipelineDesc::new(vs, fs, color_format)
                    .with_label("vfx.gpu_particles.billboard")
                    .with_bind_group_layouts(vec![layout])
                    .with_depth_state(
                        TextureFormat::Depth32Float,
                        PipelineDepthMode::new(true, false, PipelineDepthCompare::LessOrEqual),
                    )
                    .with_cull_mode(RasterCullMode::None)
                    .with_blend_mode(PipelineBlendMode::Alpha)
                    .with_cache_key(format!("vfx.gpu_particles.billboard.v1.{color_format:?}")),
            )?;
            self.graphics_pipelines.push((color_format, pipeline));
        }
        Ok(())
    }

    fn process_kills(
        &mut self,
        r: &mut dyn RenderApi,
        bridge: &VfxGpuParticleBridge,
        particle_buffer: BufferId,
    ) -> EngineResult<u32> {
        let kills = bridge.drain_kills(MAX_KILLS_PER_FRAME);
        let mut killed = 0u32;
        for instance_id in kills {
            for slot in 0..self.high_water {
                if self.slot_instance_ids[slot] != instance_id {
                    continue;
                }
                self.slot_instance_ids[slot] = 0;
                self.slot_deadlines[slot] = 0.0;
                let lifetime_offset = slot as u64 * PARTICLE_SLOT_BYTES as u64 + 28;
                r.write_buffer(particle_buffer, lifetime_offset, &0.0f32.to_ne_bytes())?;
                killed = killed.saturating_add(1);
            }
        }
        Ok(killed)
    }

    fn process_spawns(
        &mut self,
        r: &mut dyn RenderApi,
        bridge: &VfxGpuParticleBridge,
        particle_buffer: BufferId,
    ) -> EngineResult<(u32, u32)> {
        let spawns = bridge.drain_spawns(MAX_SPAWN_UPLOADS_PER_FRAME);
        if spawns.is_empty() {
            return Ok((0, 0));
        }
        let mut uploads = Vec::<(usize, [u8; PARTICLE_SLOT_BYTES])>::with_capacity(spawns.len());
        let mut dropped = 0u32;
        for spawn in spawns {
            let Some(slot) = self.allocate_slot(spawn.instance_id, spawn.lifetime_seconds) else {
                dropped = dropped.saturating_add(1);
                self.dropped_for_capacity = self.dropped_for_capacity.saturating_add(1);
                continue;
            };
            uploads.push((slot, encode_particle_slot(spawn)));
        }
        uploads.sort_unstable_by_key(|(slot, _)| *slot);
        self.write_upload_runs(r, particle_buffer, &uploads)?;
        Ok((uploads.len() as u32, dropped))
    }

    fn write_upload_runs(
        &self,
        r: &mut dyn RenderApi,
        particle_buffer: BufferId,
        uploads: &[(usize, [u8; PARTICLE_SLOT_BYTES])],
    ) -> EngineResult<()> {
        let mut cursor = 0usize;
        while cursor < uploads.len() {
            let first_slot = uploads[cursor].0;
            let mut bytes = Vec::with_capacity(PARTICLE_SLOT_BYTES * 8);
            bytes.extend_from_slice(&uploads[cursor].1);
            cursor += 1;
            let mut expected_slot = first_slot + 1;
            while cursor < uploads.len() && uploads[cursor].0 == expected_slot {
                bytes.extend_from_slice(&uploads[cursor].1);
                cursor += 1;
                expected_slot += 1;
            }
            r.write_buffer(
                particle_buffer,
                first_slot as u64 * PARTICLE_SLOT_BYTES as u64,
                &bytes,
            )?;
        }
        Ok(())
    }

    fn allocate_slot(&mut self, instance_id: u64, lifetime_seconds: f32) -> Option<usize> {
        for offset in 0..VFX_GPU_PARTICLE_CAPACITY {
            let slot = (self.next_slot + offset) % VFX_GPU_PARTICLE_CAPACITY;
            if self.slot_deadlines[slot] > self.elapsed_seconds {
                continue;
            }
            self.slot_deadlines[slot] =
                self.elapsed_seconds + f64::from(lifetime_seconds.max(0.001));
            self.slot_instance_ids[slot] = instance_id;
            self.next_slot = (slot + 1) % VFX_GPU_PARTICLE_CAPACITY;
            self.high_water = self.high_water.max(slot + 1);
            return Some(slot);
        }
        None
    }

    fn trim_expired_tail(&mut self) {
        while self.high_water > 0
            && self.slot_deadlines[self.high_water - 1] <= self.elapsed_seconds
        {
            self.slot_instance_ids[self.high_water - 1] = 0;
            self.high_water -= 1;
        }
    }

    fn graphics_pipeline(&self, format: TextureFormat) -> Option<PipelineId> {
        self.graphics_pipelines
            .iter()
            .find_map(|(candidate, pipeline)| (*candidate == format).then_some(*pipeline))
    }
}

fn encode_frame_ubo(
    view_projection: Mat4,
    camera_right: Vec3,
    camera_up: Vec3,
    camera_position: Vec3,
    dt: f32,
    high_water: usize,
) -> [u8; PARTICLE_FRAME_UBO_BYTES as usize] {
    let mut values = [0.0f32; 32];
    values[..16].copy_from_slice(&view_projection.to_cols_array());
    values[16..20].copy_from_slice(&[camera_right.x, camera_right.y, camera_right.z, 0.0]);
    values[20..24].copy_from_slice(&[camera_up.x, camera_up.y, camera_up.z, 0.0]);
    values[24..28].copy_from_slice(&[camera_position.x, camera_position.y, camera_position.z, dt]);
    values[28..32].copy_from_slice(&[
        high_water as f32,
        VFX_GPU_PARTICLE_CAPACITY as f32,
        0.0,
        0.0,
    ]);
    f32_array_bytes(values)
}

fn encode_particle_slot(spawn: VfxGpuParticleSpawnV1) -> [u8; PARTICLE_SLOT_BYTES] {
    let mut values = [0.0f32; 24];
    values[0..4].copy_from_slice(&[spawn.position[0], spawn.position[1], spawn.position[2], 0.0]);
    values[4..8].copy_from_slice(&[
        spawn.velocity[0],
        spawn.velocity[1],
        spawn.velocity[2],
        spawn.lifetime_seconds,
    ]);
    values[8..12].copy_from_slice(&[
        spawn.acceleration[0],
        spawn.acceleration[1],
        spawn.acceleration[2],
        spawn.size[0],
    ]);
    values[12..16].copy_from_slice(&[
        spawn.growth_per_second[0],
        spawn.growth_per_second[1],
        spawn.size[1],
        spawn.fade_start_fraction,
    ]);
    values[16..20].copy_from_slice(&spawn.color);
    values[20] = match spawn.kind {
        VfxGpuParticleKind::Smoke => 1.0,
        VfxGpuParticleKind::Spark => 2.0,
    };
    f32_array_bytes(values)
}

fn f32_array_bytes<const N: usize, const B: usize>(values: [f32; N]) -> [u8; B] {
    debug_assert_eq!(B, N * 4);
    let mut out = [0u8; B];
    for (index, value) in values.into_iter().enumerate() {
        let offset = index * 4;
        out[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spawn(instance_id: u64, lifetime_seconds: f32) -> VfxGpuParticleSpawnV1 {
        VfxGpuParticleSpawnV1 {
            instance_id,
            kind: VfxGpuParticleKind::Spark,
            position: [1.0, 2.0, 3.0],
            velocity: [4.0, 5.0, 6.0],
            acceleration: [0.0, -9.81, 0.0],
            size: [0.02, 0.10],
            growth_per_second: [0.0; 2],
            color: [1.0, 0.8, 0.4, 1.0],
            lifetime_seconds,
            fade_start_fraction: 0.6,
        }
    }

    #[test]
    fn particle_slot_encoding_matches_std430_contract() {
        let bytes = encode_particle_slot(spawn(9, 2.0));
        assert_eq!(bytes.len(), PARTICLE_SLOT_BYTES);
        let lifetime = f32::from_ne_bytes(bytes[28..32].try_into().unwrap());
        let width = f32::from_ne_bytes(bytes[44..48].try_into().unwrap());
        let height = f32::from_ne_bytes(bytes[56..60].try_into().unwrap());
        let kind = f32::from_ne_bytes(bytes[80..84].try_into().unwrap());
        assert_eq!(lifetime, 2.0);
        assert_eq!(width, 0.02);
        assert_eq!(height, 0.10);
        assert_eq!(kind, 2.0);
    }

    #[test]
    fn slot_allocator_does_not_overwrite_live_particles() {
        let mut renderer = VfxGpuRenderer::new();
        renderer.slot_deadlines.fill(10.0);
        renderer.high_water = VFX_GPU_PARTICLE_CAPACITY;
        assert!(renderer.allocate_slot(1, 1.0).is_none());
        renderer.slot_deadlines[17] = 0.0;
        renderer.next_slot = 17;
        assert_eq!(renderer.allocate_slot(2, 1.0), Some(17));
        assert_eq!(renderer.slot_instance_ids[17], 2);
    }
}
