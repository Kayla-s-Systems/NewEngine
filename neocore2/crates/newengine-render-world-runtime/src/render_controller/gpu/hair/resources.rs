use super::*;

impl HairGpuRenderer {
    pub(super) fn ensure_resources(
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

    pub(super) fn destroy_shader_pipelines(&mut self, r: &mut dyn RenderApi) {
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

    pub(super) fn upload_topology(
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
    pub(super) fn graphics_pipeline(&self, format: TextureFormat) -> Option<PipelineId> {
        self.graphics_pipelines
            .iter()
            .find_map(|(candidate, pipeline)| (*candidate == format).then_some(*pipeline))
    }
}
