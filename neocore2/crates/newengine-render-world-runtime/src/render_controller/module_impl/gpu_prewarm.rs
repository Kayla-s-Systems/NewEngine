#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::{RenderApi, TextureFormat};
use newengine_core::{EngineResult, ThreadPoolHandle};
use newengine_gameplay_world_runtime::gameplay::{
    PreparedRenderMesh, PrimitiveGpuEvictionQueue, WorldActivationState,
};
use newengine_primitives::{Primitive, PrimitiveId};
use newengine_procedural_noise::ProceduralTerrain;
use newengine_scene::Scene;
use std::collections::BTreeSet;

use super::super::gpu::{ensure_primitive_gpu, upload_primitive_mesh};
use super::RuntimeRenderController;

impl RuntimeRenderController {
    /// Removes GPU primitive cache entries requested by world residency producers and retires
    /// native buffers through the renderer's frame-completion lifetime queue. Active ECS/model
    /// references win over stale eviction requests, so a cell that re-enters before this drain
    /// cannot lose a mesh still needed by the current frame.
    fn drain_primitive_gpu_evictions(&mut self, scene: &Scene) -> usize {
        let world = scene.world();
        let Some(queue) = world.resource::<PrimitiveGpuEvictionQueue>() else {
            return 0;
        };
        let requested = queue.drain();
        if requested.is_empty() {
            return 0;
        }

        let mut active = BTreeSet::<PrimitiveId>::new();
        active.extend(
            world
                .query::<Primitive>()
                .map(|(_, primitive)| primitive.id),
        );
        for (_, model) in
            world.query::<newengine_gameplay_world_runtime::gameplay::ModelRenderComponent>()
        {
            let source = model.logical_path.trim();
            let Some(bundle) = self.gpu.meshes.model_bundle_cache.get(source) else {
                continue;
            };
            active.extend(
                bundle
                    .parts
                    .iter()
                    .enumerate()
                    .map(|(part_index, _)| Self::model_part_primitive_id(bundle, part_index)),
            );
        }

        let mut evicted = 0usize;
        for id in requested {
            if active.contains(&id) {
                continue;
            }
            let Some(gpu) = self.gpu.meshes.prim_cache.remove(&id) else {
                continue;
            };
            self.gpu
                .lifetimes
                .resources
                .retire_buffer_after_frame(gpu.vb, self.frame.frame_index);
            self.gpu
                .lifetimes
                .resources
                .retire_buffer_after_frame(gpu.ib, self.frame.frame_index);
            evicted = evicted.saturating_add(1);
        }
        if evicted > 0 {
            self.invalidate_shadow_cache();
            self.invalidate_local_shadow_cache();
            newengine_ulog_api::ulog::debug!(
                "render residency: primitive gpu evictions frame={} evicted={} active={} cache_remaining={} policy='frame-completion deferred buffer retirement'",
                self.frame.frame_index,
                evicted,
                active.len(),
                self.gpu.meshes.prim_cache.len(),
            );
        }
        evicted
    }

    /// Reclaims procedural-terrain GPU meshes whose ECS owners have left the active
    /// streaming window. CPU terrain mesh memory is entity-owned; this closes the matching
    /// GPU high-water path while keeping buffer destruction frame-completion safe.
    fn drain_stale_terrain_gpu(&mut self, scene: &Scene) -> usize {
        let active = scene
            .world()
            .query::<ProceduralTerrain>()
            .map(|(_, terrain)| terrain.mesh_key())
            .collect::<BTreeSet<_>>();
        let stale = self
            .gpu
            .meshes
            .terrain_cache
            .keys()
            .copied()
            .filter(|mesh_key| !active.contains(mesh_key))
            .collect::<Vec<_>>();
        let mut evicted = 0usize;
        for mesh_key in stale {
            let Some(gpu) = self.gpu.meshes.terrain_cache.remove(&mesh_key) else {
                continue;
            };
            self.gpu
                .lifetimes
                .resources
                .retire_buffer_after_frame(gpu.vb, self.frame.frame_index);
            self.gpu
                .lifetimes
                .resources
                .retire_buffer_after_frame(gpu.ib, self.frame.frame_index);
            evicted = evicted.saturating_add(1);
        }
        if evicted > 0 {
            self.invalidate_shadow_cache();
            self.invalidate_local_shadow_cache();
            newengine_ulog_api::ulog::debug!(
                "render residency: terrain gpu evictions frame={} evicted={} active={} cache_remaining={} policy='owner-mark-sweep; frame-completion deferred retirement'",
                self.frame.frame_index,
                evicted,
                active.len(),
                self.gpu.meshes.terrain_cache.len(),
            );
        }
        evicted
    }

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
        let _primitive_evicted = self.drain_primitive_gpu_evictions(scene);
        let _terrain_evicted = self.drain_stale_terrain_gpu(scene);
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

        let (primitive_budget, primitive_budget_ms) = primitive_gpu_upload_limits(world);
        let primitive_upload_started = std::time::Instant::now();
        let mut primitive_uploaded = 0_u32;
        if primitive_budget > 0 {
            let mut unique = BTreeSet::<PrimitiveId>::new();
            for (_entity, primitive) in world.query::<Primitive>() {
                unique.insert(primitive.id);
            }
            let registry_lock = self.bridges.scene.primitives();
            let registry = registry_lock.read();
            for primitive_id in unique {
                if primitive_uploaded >= primitive_budget
                    || (primitive_uploaded > 0
                        && primitive_upload_started.elapsed().as_secs_f32() * 1000.0
                            >= primitive_budget_ms)
                {
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
                newengine_runtime_policy::streaming_policy().model_gpu_uploads_per_frame,
                terrain_budget,
                primitive_budget,
            );
        }
        Ok(total_uploaded)
    }
}

fn terrain_gpu_upload_budget_per_frame() -> u32 {
    newengine_runtime_policy::streaming_policy().terrain_gpu_uploads_per_frame
}

fn primitive_gpu_upload_limits(world: &newengine_ecs::World) -> (u32, f32) {
    let prelaunch = world
        .resource::<WorldActivationState>()
        .is_some_and(WorldActivationState::needs_prelaunch_gate);
    if prelaunch {
        let uploads = newengine_runtime_env::var_u32(
            "NEWENGINE_PRIMITIVE_GPU_PRELAUNCH_UPLOADS_PER_FRAME",
            8,
            1,
            64,
        );
        let budget_ms = newengine_runtime_env::var_f32(
            "NEWENGINE_PRIMITIVE_GPU_PRELAUNCH_BUDGET_MS",
            12.0,
            1.0,
            32.0,
        );
        (uploads, budget_ms)
    } else {
        (
            newengine_runtime_policy::streaming_policy().primitive_gpu_uploads_per_frame,
            f32::INFINITY,
        )
    }
}

fn primitive_gpu_upload_warn_ms() -> f32 {
    newengine_runtime_policy::streaming_policy().primitive_gpu_upload_warn_ms
}

fn terrain_gpu_upload_interval_frames() -> u64 {
    newengine_runtime_policy::streaming_policy().terrain_gpu_upload_interval_frames
}
