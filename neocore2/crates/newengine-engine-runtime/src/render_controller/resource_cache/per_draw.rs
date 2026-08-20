use newengine_core::render::{SamplerId, TextureId};

use super::super::controller::RuntimeRenderController;
use super::super::gpu::{LitPipeline, LIT_UBO_SIZE};
use super::super::state::PerDrawUbo;

const PER_DRAW_UBO_GC_INTERVAL_FRAMES: u64 = 120;
const PER_DRAW_UBO_IDLE_FRAMES: u64 = 240;

impl RuntimeRenderController {
    pub(in crate::render_controller) fn ensure_per_draw_ubo_with_binding(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
        lit: LitPipeline,
        key: u64,
        base_texture: TextureId,
        normal_texture: TextureId,
        roughness_texture: TextureId,
        shadow_texture: TextureId,
        local_shadow_texture: TextureId,
        sampler: SamplerId,
    ) -> newengine_core::EngineResult<PerDrawUbo> {
        if let Some(mut e) = self.gpu.material.per_draw_ubo.get(&key).copied() {
            e.last_seen_frame = self.frame.frame_index;
            if e.base_texture == base_texture
                && e.normal_texture == normal_texture
                && e.roughness_texture == roughness_texture
                && e.shadow_texture == shadow_texture
                && e.local_shadow_texture == local_shadow_texture
                && e.sampler == sampler
            {
                self.gpu.material.per_draw_ubo.insert(key, e);
                return Ok(e);
            }

            let old_bg = e.bg;
            let bg = r.create_bind_group(
                newengine_core::render::BindGroupDesc::new(lit.bgl)
                    .with_label("material_lit_entity_bg")
                    .with_uniform0(newengine_core::render::BufferBinding::new(
                        e.ubo,
                        0,
                        LIT_UBO_SIZE,
                    ))
                    .with_texture0(base_texture)
                    .with_texture1(normal_texture)
                    .with_texture2(roughness_texture)
                    .with_texture3(shadow_texture)
                    .with_sampler0(sampler)
                    .with_texture4(local_shadow_texture),
            )?;
            self.gpu
                .lifetimes
                .resources
                .retire_bind_group_after_frame(old_bg, self.frame.frame_index);
            e.bg = bg;
            e.base_texture = base_texture;
            e.normal_texture = normal_texture;
            e.roughness_texture = roughness_texture;
            e.shadow_texture = shadow_texture;
            e.local_shadow_texture = local_shadow_texture;
            e.sampler = sampler;
            self.gpu.material.per_draw_ubo.insert(key, e);
            return Ok(e);
        }

        let ubo = r.create_buffer(
            newengine_core::render::BufferDesc::new(
                LIT_UBO_SIZE,
                newengine_core::render::BufferUsage::Uniform,
                newengine_core::render::MemoryHint::CpuToGpu,
            )
            .with_label("material_lit_entity_ubo"),
        )?;

        let bg = r.create_bind_group(
            newengine_core::render::BindGroupDesc::new(lit.bgl)
                .with_label("material_lit_entity_bg")
                .with_uniform0(newengine_core::render::BufferBinding::new(
                    ubo,
                    0,
                    LIT_UBO_SIZE,
                ))
                .with_texture0(base_texture)
                .with_texture1(normal_texture)
                .with_texture2(roughness_texture)
                .with_texture3(shadow_texture)
                .with_sampler0(sampler)
                .with_texture4(local_shadow_texture),
        )?;

        let entry = PerDrawUbo {
            ubo,
            bg,
            base_texture,
            normal_texture,
            roughness_texture,
            shadow_texture,
            local_shadow_texture,
            sampler,
            last_seen_frame: self.frame.frame_index,
        };
        self.gpu.material.per_draw_ubo.insert(key, entry);
        Ok(entry)
    }

    pub(in crate::render_controller) fn gc_per_draw_ubos(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
    ) {
        self.collect_render_lifetime_events(r);

        let frame = self.frame.frame_index;
        if !frame.is_multiple_of(PER_DRAW_UBO_GC_INTERVAL_FRAMES) {
            return;
        }
        let cutoff = frame.saturating_sub(PER_DRAW_UBO_IDLE_FRAMES);
        let stale = self
            .gpu
            .material
            .per_draw_ubo
            .iter()
            .filter_map(|(key, entry)| (entry.last_seen_frame < cutoff).then_some(*key))
            .collect::<Vec<_>>();
        if stale.is_empty() {
            return;
        }

        let mut retired = 0usize;
        for key in stale {
            let Some(entry) = self.gpu.material.per_draw_ubo.remove(&key) else {
                continue;
            };
            self.gpu
                .lifetimes
                .resources
                .retire_bind_group_after_frame(entry.bg, frame);
            self.gpu
                .lifetimes
                .resources
                .retire_buffer_after_frame(entry.ubo, frame);
            retired += 1;
        }
        newengine_ulog_api::ulog::debug!(
            "render material cache: retired stale per-draw UBOs frame={} retired={} remaining={} idle_frames={}",
            frame,
            retired,
            self.gpu.material.per_draw_ubo.len(),
            PER_DRAW_UBO_IDLE_FRAMES,
        );
    }
}
