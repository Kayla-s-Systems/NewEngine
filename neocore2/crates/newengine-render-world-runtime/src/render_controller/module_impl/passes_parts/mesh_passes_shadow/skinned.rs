use super::*;

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
) -> newengine_core::EngineResult<()> {
    use crate::render_controller::gpu::{ensure_player_skin_gpu, ensure_skin_palette_gpu};

    let world = scene.world();
    let (shadow_snapshot, _snapshot_reused) = this.skinned_shadow_scene_snapshot(scene, runtime);
    let reg_lock = this.bridges.scene.primitives();
    let reg = reg_lock.read();
    let mats_lock = this.bridges.scene.materials();
    let mats = mats_lock.read();
    let shadow_max_distance = primitive_shadow_max_distance(runtime);
    let shadow_max_distance_sq = shadow_max_distance * shadow_max_distance;

    for source in shadow_snapshot.entries.iter() {
        let entity = source.entity;
        let prim = source.primitive;
        let render_model = source.render_model;
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
        if runtime
            && (source.proxy_center_ws.distance_squared(camera_position) > shadow_max_distance_sq
                || !shadow_caster_visible(
                    this.shadows_current_cull(),
                    source.proxy_center_ws,
                    source.proxy_radius_ws,
                )
                || !shadow_caster_projected_radius_visible(
                    cascade_index,
                    cascade_texel_world_size,
                    source.proxy_radius_ws,
                ))
        {
            continue;
        }

        let material_ref = source.material_ref;
        let resolved = material_ref.and_then(|reference| mats.resolve(reference.id));
        let material_plan = LitMaterialPlan::from_resolved(resolved.as_ref(), prim.color);
        if !material_plan.cast_shadows {
            continue;
        }

        let gpu = ensure_primitive_gpu(&reg, prim.id, &mut this.gpu.meshes.prim_cache, r)?;
        let skin_gpu = ensure_player_skin_gpu(
            &mut this.gpu.meshes.skin_vertex_cache,
            prim.id,
            gpu,
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
        let pose_generation = source.pose_generation;
        let palette_gpu = ensure_skin_palette_gpu(
            &mut this.gpu.meshes.skin_palette_cache,
            &mut this.gpu.lifetimes.resources,
            source.owner.stable_u64(),
            pose_generation,
            pose,
            lit.skin_bgl,
            this.frame.frame_index,
            this.backend_execution.host_visible_ring_slots(),
            r,
        )?;
        let base_texture = if material_plan.alpha_cutoff > 0.0 {
            let Some(path) = material_plan.base_color_texture else {
                // A masked skinned caster without its authored opacity source cannot
                // produce a valid silhouette. Never replace it with an opaque white card.
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
        let mut ubo_key = 0x736b_696e_5f73_6864u64;
        ubo_key = hash_combine_u64(ubo_key, entity.stable_u64());
        ubo_key = hash_combine_u64(ubo_key, prim.id.0);
        ubo_key = hash_combine_u64(ubo_key, shadow_ubo_view.cache_discriminator());
        ubo_key = hash_combine_u64(ubo_key, base_texture.get() as u64);
        ubo_key = hash_combine_u64(ubo_key, pipeline.get() as u64);
        let per = this.ensure_per_draw_ubo_with_binding(
            r,
            lit,
            ubo_key,
            base_texture,
            lit.flat_normal_texture,
            lit.white_texture,
            lit.white_texture,
            lit.white_texture,
            lit.clamp_sampler,
        )?;
        crate::render_controller::module_impl::passes_ubo::write_lit_ubo_ex(
            r,
            per.ubo,
            light_viewproj * render_model,
            render_model,
            material_plan.base_color,
            material_plan.emissive_radiance,
            material_plan.alpha_cutoff,
            material_plan.uv_transform,
            material_plan.material_params,
            lights,
        )?;
        r.set_pipeline(pipeline)?;
        r.set_bind_group(0, per.bg)?;
        r.set_bind_group(1, palette_gpu.bg)?;
        r.set_vertex_buffer(0, BufferSlice::new(gpu.vb, 0))?;
        r.set_vertex_buffer(1, BufferSlice::new(skin_gpu.vb, 0))?;
        r.set_index_buffer(BufferSlice::new(gpu.ib, 0), IndexFormat::U32)?;
        r.draw_indexed(DrawIndexedArgs::new(gpu.index_count))?;
    }
    Ok(())
}
