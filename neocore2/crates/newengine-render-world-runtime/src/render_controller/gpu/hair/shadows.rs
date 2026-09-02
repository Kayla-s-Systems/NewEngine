use super::*;

impl HairGpuRenderer {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn record_directional_shadows(
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
}
