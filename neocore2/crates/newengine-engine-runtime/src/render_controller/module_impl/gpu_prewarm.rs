#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::RenderApi;
use newengine_core::EngineResult;
use newengine_primitives::Primitive;
use newengine_procedural_noise::ProceduralTerrain;
use newengine_scene::Scene;

use super::super::gpu::{ensure_lit_pipeline, ensure_primitive_gpu, upload_primitive_mesh};
use super::RuntimeRenderController;

impl RuntimeRenderController {
    /// Builds the expensive immutable GPU resources while the native loading
    /// screen is still active.
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
        let _lit = ensure_lit_pipeline(&mut self.gpu.lit, r)?;

        let world = scene.world();
        let mut terrain_uploaded = 0_u32;
        for (_entity, terrain) in world.query::<ProceduralTerrain>() {
            let mesh_key = terrain.mesh_key();
            if self.gpu.terrain_cache.contains_key(&mesh_key) {
                continue;
            }
            let mesh = terrain.heightfield.to_primitive_mesh();
            let gpu = upload_primitive_mesh(r, &mesh, "prewarm_proc_terrain")?;
            self.gpu.terrain_cache.insert(mesh_key, gpu);
            terrain_uploaded = terrain_uploaded.saturating_add(1);
        }

        let reg_lock = self.bridges.scene.primitives();
        let reg = reg_lock.read();
        let mut primitive_uploaded = 0_u32;
        for (_entity, prim) in world.query::<Primitive>() {
            if self.gpu.prim_cache.contains_key(&prim.id) {
                continue;
            }
            let _ = ensure_primitive_gpu(&reg, prim.id, &mut self.gpu.prim_cache, r)?;
            primitive_uploaded = primitive_uploaded.saturating_add(1);
        }

        if terrain_uploaded > 0 || primitive_uploaded > 0 {
            log::info!(
                "render prewarm: gpu resources prepared terrain_meshes={} primitive_meshes={} elapsed_ms={:.2}",
                terrain_uploaded,
                primitive_uploaded,
                started.elapsed().as_secs_f32() * 1000.0
            );
        }

        Ok(())
    }
}
