#![forbid(unsafe_op_in_unsafe_fn)]

use crate::scene_bridge::PreparedTerrainPrimitiveMesh;
use newengine_core::render::RenderApi;
use newengine_core::EngineResult;
use newengine_primitives::Primitive;
use newengine_procedural_noise::ProceduralTerrain;
use newengine_scene::Scene;

use super::super::gpu::{ensure_primitive_gpu, upload_primitive_mesh};
use super::RuntimeRenderController;

impl RuntimeRenderController {
    /// Builds the expensive immutable GPU resources while the loading projection is still active.
    ///
    /// The reference renderer keeps draw-list population and resource residency
    /// ahead of presentation. This small warmup step follows the same principle:
    /// terrain meshes, primitive meshes and lit pipelines are created before the
    /// first public gameplay frame, so frame one does not absorb every cold cache
    /// cost at once.
    pub(super) fn prewarm_scene_gpu_resources(
        &mut self,
        r: &mut dyn RenderApi,
        scene: &Scene,
    ) -> EngineResult<()> {
        let started = std::time::Instant::now();
        if let Err(e) = self.gpu.require_primary_lit_pipeline(r) {
            newengine_ulog_api::ulog::warn!(
                "render prewarm: material pipeline is not ready; continuing mesh residency and retrying pipeline on later frames err='{}'",
                e
            );
        }

        let world = scene.world();
        let mut terrain_uploaded = 0_u32;
        for (_entity, terrain) in world.query::<ProceduralTerrain>() {
            let mesh_key = terrain.mesh_key();
            if self.gpu.meshes.terrain_cache.contains_key(&mesh_key) {
                continue;
            }
            let gpu = if let Some(prepared) = world.get::<PreparedTerrainPrimitiveMesh>(_entity) {
                upload_primitive_mesh(r, prepared.mesh.as_ref(), "prewarm_proc_terrain")?
            } else {
                let mesh = terrain.heightfield.to_primitive_mesh();
                upload_primitive_mesh(r, &mesh, "prewarm_proc_terrain")?
            };
            self.gpu.meshes.terrain_cache.insert(mesh_key, gpu);
            terrain_uploaded = terrain_uploaded.saturating_add(1);
        }

        let reg_lock = self.bridges.scene.primitives();
        let reg = reg_lock.read();
        let mut primitive_uploaded = 0_u32;
        for (_entity, prim) in world.query::<Primitive>() {
            if self.gpu.meshes.prim_cache.contains_key(&prim.id) {
                continue;
            }
            let _ = ensure_primitive_gpu(&reg, prim.id, &mut self.gpu.meshes.prim_cache, r)?;
            primitive_uploaded = primitive_uploaded.saturating_add(1);
        }

        if terrain_uploaded > 0 || primitive_uploaded > 0 {
            newengine_ulog_api::ulog::info!(
                "render prewarm: gpu resources prepared terrain_meshes={} primitive_meshes={} elapsed_ms={:.2}",
                terrain_uploaded,
                primitive_uploaded,
                started.elapsed().as_secs_f32() * 1000.0
            );
        }

        Ok(())
    }

    /// Advances render residency for streamed terrain without doing cold uploads
    /// inside draw-list extraction.
    ///
    /// The scene streamer may create ECS chunks before their GPU buffers are
    /// resident. This method performs a small, explicit upload budget before
    /// feature extraction; draw recording then only references already-ready
    /// mesh handles and skips not-ready chunks for the current frame.
    pub(super) fn pump_scene_gpu_residency(
        &mut self,
        r: &mut dyn RenderApi,
        scene: &Scene,
    ) -> EngineResult<u32> {
        let budget = terrain_gpu_upload_budget_per_frame();
        if budget == 0 {
            return Ok(0);
        }

        let interval = terrain_gpu_upload_interval_frames();
        if interval > 1 && self.frame.frame_index % interval != 0 {
            return Ok(0);
        }

        let world = scene.world();
        let mut uploaded = 0_u32;
        for (entity, terrain) in world.query::<ProceduralTerrain>() {
            if uploaded >= budget {
                break;
            }

            let mesh_key = terrain.mesh_key();
            if self.gpu.meshes.terrain_cache.contains_key(&mesh_key) {
                continue;
            }

            let Some(prepared) = world.get::<PreparedTerrainPrimitiveMesh>(entity) else {
                // Compatibility scenes without a prepared mesh are handled by
                // launch prewarm. Runtime streaming must not convert heightfields
                // on the frame extraction path.
                continue;
            };

            let gpu = upload_primitive_mesh(r, prepared.mesh.as_ref(), "streamed_proc_terrain")?;
            self.gpu.meshes.terrain_cache.insert(mesh_key, gpu);
            uploaded = uploaded.saturating_add(1);
        }

        if uploaded > 0 && newengine_ulog_api::ulog::trace_enabled() {
            newengine_ulog_api::ulog::trace!(
                "render residency: terrain gpu uploads frame={} uploaded={} budget={}",
                self.frame.frame_index,
                uploaded,
                budget
            );
        }

        Ok(uploaded)
    }
}

fn terrain_gpu_upload_budget_per_frame() -> u32 {
    crate::env_config::var_u32("NEWENGINE_TERRAIN_GPU_UPLOADS_PER_FRAME", 8, 0, 32)
}

fn terrain_gpu_upload_interval_frames() -> u64 {
    // Terrain visibility should not lag behind streaming by several frames.
    // Uploads are still explicitly budgeted, but the default pumps residency
    // every frame so the player does not stare at an empty world while chunks
    // are already CPU-prepared.
    crate::env_config::var_u64("NEWENGINE_TERRAIN_GPU_UPLOAD_INTERVAL_FRAMES", 1, 1, 240)
}
