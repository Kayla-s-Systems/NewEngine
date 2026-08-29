#![forbid(unsafe_op_in_unsafe_fn)]

use crate::gameplay::PreparedRenderMesh;
use newengine_core::render::{RenderApi, TextureFormat};
use newengine_core::{EngineResult, ThreadPoolHandle};
use newengine_primitives::{Primitive, PrimitiveId};
use newengine_procedural_noise::ProceduralTerrain;
use newengine_scene::Scene;
use std::collections::BTreeSet;

use super::super::gpu::{ensure_primitive_gpu, upload_primitive_mesh};
use super::RuntimeRenderController;

impl RuntimeRenderController {
    /// Warms the immutable scene pipeline while the loading projection is active.
    ///
    /// Mesh residency is intentionally excluded from this method. Terrain,
    /// imported-model and primitive uploads are admitted by
    /// `pump_scene_gpu_residency`, which applies explicit per-frame budgets.
    pub(super) fn prewarm_scene_pipeline(&mut self, r: &mut dyn RenderApi) -> EngineResult<()> {
        let started = std::time::Instant::now();
        let scene_color_format = if self.runtime_profile().hdr_scene_enabled() {
            super::super::render_quality::SCENE_HDR_COLOR_FORMAT
        } else {
            TextureFormat::Bgra8Unorm
        };
        let _ = self.gpu.require_primary_lit_pipeline_for(
            scene_color_format,
            self.runtime_profile().deferred_enabled(),
            r,
        )?;

        if newengine_ulog_api::ulog::trace_enabled() {
            newengine_ulog_api::ulog::trace!(
                "render prewarm: primary scene pipeline ready format={:?} elapsed_ms={:.2}",
                scene_color_format,
                started.elapsed().as_secs_f32() * 1000.0,
            );
        }
        Ok(())
    }

    /// Advances CPU/GPU residency for imported models, streamed terrain and
    /// primitive meshes without doing cold model decode/upload work in draw-list
    /// extraction.
    pub(super) fn pump_scene_gpu_residency(
        &mut self,
        r: &mut dyn RenderApi,
        scene: &Scene,
        thread_pool: Option<&ThreadPoolHandle>,
    ) -> EngineResult<u32> {
        let model_uploaded = self.pump_model_residency(r, scene, thread_pool)?;

        let interval = terrain_gpu_upload_interval_frames();
        if interval > 1 && !self.frame.frame_index.is_multiple_of(interval) {
            return Ok(model_uploaded);
        }

        let world = scene.world();
        let terrain_budget = terrain_gpu_upload_budget_per_frame();
        let mut terrain_uploaded = 0_u32;
        if terrain_budget > 0 {
            for (entity, terrain) in world.query::<ProceduralTerrain>() {
                if terrain_uploaded >= terrain_budget {
                    break;
                }

                let mesh_key = terrain.mesh_key();
                if self.gpu.meshes.terrain_cache.contains_key(&mesh_key) {
                    continue;
                }

                let Some(prepared) = world.get::<PreparedRenderMesh>(entity) else {
                    continue;
                };

                let gpu =
                    upload_primitive_mesh(r, prepared.mesh.as_ref(), "streamed_proc_terrain")?;
                self.gpu.meshes.terrain_cache.insert(mesh_key, gpu);
                terrain_uploaded = terrain_uploaded.saturating_add(1);
            }
        }

        let primitive_budget = primitive_gpu_upload_budget_per_frame();
        let mut primitive_uploaded = 0_u32;
        if primitive_budget > 0 {
            let mut unique = BTreeSet::<PrimitiveId>::new();
            for (_entity, primitive) in world.query::<Primitive>() {
                unique.insert(primitive.id);
            }
            let registry_lock = self.bridges.scene.primitives();
            let registry = registry_lock.read();
            for primitive_id in unique {
                if primitive_uploaded >= primitive_budget {
                    break;
                }
                if self.gpu.meshes.prim_cache.contains_key(&primitive_id) {
                    continue;
                }
                let started = std::time::Instant::now();
                let _ = ensure_primitive_gpu(
                    &registry,
                    primitive_id,
                    &mut self.gpu.meshes.prim_cache,
                    r,
                )?;
                let elapsed_ms = started.elapsed().as_secs_f32() * 1000.0;
                primitive_uploaded = primitive_uploaded.saturating_add(1);
                if elapsed_ms >= primitive_gpu_upload_warn_ms() {
                    newengine_ulog_api::ulog::warn!(
                        "render residency: primitive gpu upload exceeded responsiveness budget frame={} primitive={:?} elapsed_ms={:.2} warn_ms={:.2}",
                        self.frame.frame_index,
                        primitive_id,
                        elapsed_ms,
                        primitive_gpu_upload_warn_ms(),
                    );
                } else if newengine_ulog_api::ulog::trace_enabled() {
                    newengine_ulog_api::ulog::trace!(
                        "render residency: primitive gpu upload frame={} primitive={:?} elapsed_ms={:.2}",
                        self.frame.frame_index,
                        primitive_id,
                        elapsed_ms,
                    );
                }
            }
        }

        let total_uploaded = model_uploaded
            .saturating_add(terrain_uploaded)
            .saturating_add(primitive_uploaded);
        if total_uploaded > 0 && newengine_ulog_api::ulog::trace_enabled() {
            newengine_ulog_api::ulog::trace!(
                "render residency: bounded gpu uploads frame={} models={} terrain={} primitives={} model_budget={} terrain_budget={} primitive_budget={}",
                self.frame.frame_index,
                model_uploaded,
                terrain_uploaded,
                primitive_uploaded,
                crate::runtime_policy::streaming_policy().model_gpu_uploads_per_frame,
                terrain_budget,
                primitive_budget,
            );
        }
        Ok(total_uploaded)
    }
}

fn terrain_gpu_upload_budget_per_frame() -> u32 {
    crate::runtime_policy::streaming_policy().terrain_gpu_uploads_per_frame
}

fn primitive_gpu_upload_budget_per_frame() -> u32 {
    crate::runtime_policy::streaming_policy().primitive_gpu_uploads_per_frame
}

fn primitive_gpu_upload_warn_ms() -> f32 {
    crate::runtime_policy::streaming_policy().primitive_gpu_upload_warn_ms
}

fn terrain_gpu_upload_interval_frames() -> u64 {
    crate::runtime_policy::streaming_policy().terrain_gpu_upload_interval_frames
}
