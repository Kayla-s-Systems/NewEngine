use super::*;

use crate::render_controller::module_impl::frame_snapshots::{
    PreparedSkinnedShadowCaster, PreparedSkinnedShadowFramePlan,
};
use std::sync::Arc;

#[allow(clippy::too_many_arguments)]
fn prepare_skinned_shadow_frame_plan(
    this: &mut RuntimeRenderController,
    r: &mut dyn newengine_core::render::RenderApi,
    scene: &newengine_scene::Scene,
    lit: newengine_material_domain_api::LitPipeline,
    runtime: bool,
    camera_position: Vec3,
) -> newengine_core::EngineResult<Arc<PreparedSkinnedShadowFramePlan>> {
    use crate::render_controller::gpu::{ensure_player_skin_gpu, ensure_skin_palette_gpu};

    let frame_index = this.frame.frame_index;
    if let Some(plan) = this.frame.prepared_skinned_shadow_plan.as_ref() {
        if plan.matches(frame_index, scene, runtime) {
            return Ok(Arc::clone(plan));
        }
    }

    let world = scene.world();
    let (shadow_snapshot, _) = this.skinned_shadow_scene_snapshot(scene, runtime);
    let reg_lock = this.bridges.scene.primitives();
    let reg = reg_lock.read();
    let mats_lock = this.bridges.scene.materials();
    let mats = mats_lock.read();
    let shadow_max_distance = primitive_shadow_max_distance(runtime);
    let shadow_max_distance_sq = shadow_max_distance * shadow_max_distance;
    let mut entries = Vec::with_capacity(shadow_snapshot.entries.len());

    for source in shadow_snapshot.entries.iter() {
        // Preparation is shared by every directional cascade. Never consume current_caster_cull
        // here: at this point it belongs to whichever cascade happened to trigger preparation
        // first. Doing so would make the frame plan incomplete for later cascades.
        if runtime
            && source.proxy_center_ws.distance_squared(camera_position) > shadow_max_distance_sq
        {
            continue;
        }

        let entity = source.entity;
        let prim = source.primitive;
        let Some(skin) =
            world.get::<newengine_gameplay_world_runtime::gameplay::PlayerSkinBinding>(entity)
        else {
            continue;
        };
        debug_assert_eq!(skin.owner, source.owner);
        let Some(pose) =
            world.get::<newengine_gameplay_world_runtime::gameplay::PlayerSkinPose>(source.owner)
        else {
            continue;
        };
        if pose.palette.is_empty() {
            continue;
        }

        let resolved = source
            .material_ref
            .and_then(|reference| mats.resolve(reference.id));
        let material_plan = LitMaterialPlan::from_resolved(resolved.as_ref(), prim.color);
        if !material_plan.cast_shadows {
            continue;
        }

        let primitive_gpu =
            ensure_primitive_gpu(&reg, prim.id, &mut this.gpu.meshes.prim_cache, r)?;
        let skin_gpu = ensure_player_skin_gpu(
            &mut this.gpu.meshes.skin_vertex_cache,
            prim.id,
            primitive_gpu,
            skin,
            r,
        )?;
        if skin_gpu.max_joint_index as usize >= pose.palette.len() {
            return Err(newengine_core::EngineError::other(format!(
                "skinned shadow joint index outside palette entity={} primitive={} max_joint={} palette_joints={}",
                entity.stable_u64(),
                prim.id.0,
                skin_gpu.max_joint_index,
                pose.palette.len(),
            )));
        }
        let palette_gpu = ensure_skin_palette_gpu(
            &mut this.gpu.meshes.skin_palette_cache,
            &mut this.gpu.lifetimes.resources,
            source.owner.stable_u64(),
            source.pose_generation,
            pose,
            lit.skin_bgl,
            frame_index,
            this.backend_execution.host_visible_ring_slots(),
            r,
        )?;
        let base_texture = if material_plan.alpha_cutoff > 0.0 {
            let Some(path) = material_plan.base_color_texture else {
                continue;
            };
            let Some(texture) =
                this.material_texture_if_ready(r, path, "render.shadow_skinned_character")
            else {
                continue;
            };
            texture
        } else {
            lit.white_texture
        };
        let pipeline = if material_plan.double_sided {
            lit.shadow_skinned_double_sided_pipeline
        } else {
            lit.shadow_skinned_pipeline
        };

        entries.push(PreparedSkinnedShadowCaster {
            entity,
            primitive: prim,
            render_model: source.render_model,
            proxy_center_ws: source.proxy_center_ws,
            proxy_radius_ws: source.proxy_radius_ws,
            primitive_gpu,
            skin_gpu,
            palette_bg: palette_gpu.bg,
            base_texture,
            pipeline,
            alpha_cutoff: material_plan.alpha_cutoff,
            uv_transform: material_plan.uv_transform,
        });
    }

    let plan = Arc::new(PreparedSkinnedShadowFramePlan {
        frame_index,
        scene_key: scene as *const newengine_scene::Scene as usize,
        runtime,
        entries: entries.into_boxed_slice(),
    });
    this.frame.prepared_skinned_shadow_plan = Some(Arc::clone(&plan));
    Ok(plan)
}

pub(super) fn draw_skinned_player_primitives_shadow(
    this: &mut RuntimeRenderController,
    r: &mut dyn newengine_core::render::RenderApi,
    scene: &newengine_scene::Scene,
    lit: newengine_material_domain_api::LitPipeline,
    light_viewproj: Mat4,
    lights: &PackedLights,
    runtime: bool,
    camera_position: Vec3,
    cascade_index: usize,
    cascade_texel_world_size: f32,
    shadow_ubo_view: ShadowUboViewKey,
) -> newengine_core::EngineResult<usize> {
    let prepared =
        prepare_skinned_shadow_frame_plan(this, r, scene, lit, runtime, camera_position)?;

    let mut submitted = 0usize;
    for caster in prepared.entries.iter() {
        if runtime
            && (!shadow_caster_visible(
                this.shadows_current_cull(),
                caster.proxy_center_ws,
                caster.proxy_radius_ws,
            ) || !shadow_caster_projected_radius_visible(
                cascade_index,
                cascade_texel_world_size,
                caster.proxy_radius_ws,
            ))
        {
            continue;
        }

        let mut ubo_key = 0x736b_696e_5f73_6864u64;
        ubo_key = hash_combine_u64(ubo_key, caster.entity.stable_u64());
        ubo_key = hash_combine_u64(ubo_key, caster.primitive.id.0);
        ubo_key = hash_combine_u64(ubo_key, shadow_ubo_view.cache_discriminator());
        ubo_key = hash_combine_u64(ubo_key, caster.base_texture.get() as u64);
        ubo_key = hash_combine_u64(ubo_key, caster.pipeline.get() as u64);
        let per = this.ensure_per_draw_ubo_with_binding(
            r,
            lit,
            ubo_key,
            caster.base_texture,
            lit.flat_normal_texture,
            lit.white_texture,
            lit.white_texture,
            lit.white_texture,
            lit.clamp_sampler,
        )?;
        crate::render_controller::module_impl::passes_ubo::write_skinned_shadow_ubo(
            r,
            per.ubo,
            light_viewproj * caster.render_model,
            caster.uv_transform,
            caster.alpha_cutoff,
            lights.shadow_params[1],
        )?;
        r.set_pipeline(caster.pipeline)?;
        r.set_bind_group(0, per.bg)?;
        r.set_bind_group(1, caster.palette_bg)?;
        r.set_vertex_buffer(0, BufferSlice::new(caster.primitive_gpu.vb, 0))?;
        r.set_vertex_buffer(1, BufferSlice::new(caster.skin_gpu.vb, 0))?;
        r.set_index_buffer(
            BufferSlice::new(caster.primitive_gpu.ib, 0),
            IndexFormat::U32,
        )?;
        r.draw_indexed(DrawIndexedArgs::new(caster.primitive_gpu.index_count))?;
        submitted = submitted.saturating_add(1);
    }
    Ok(submitted)
}
