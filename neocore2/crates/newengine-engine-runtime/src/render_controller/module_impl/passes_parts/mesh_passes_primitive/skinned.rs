use super::*;

/// Draws player-owned skinned primitive parts through a dedicated non-instanced
/// character path. Static/foliage batching deliberately excludes these entities.
pub(crate) fn draw_skinned_player_primitives(
    this: &mut RuntimeRenderController,
    r: &mut dyn newengine_core::render::RenderApi,
    scene: &newengine_scene::Scene,
    lit: newengine_material_domain_api::LitPipeline,
    pass: SceneMeshPass,
    viewproj: Mat4,
    lights: &PackedLights,
    shadow_texture: TextureId,
    local_shadow_texture: TextureId,
    runtime: bool,
    camera_position: Vec3,
    camera_forward: Vec3,
) -> newengine_core::EngineResult<()> {
    use crate::render_controller::gpu::{ensure_player_skin_gpu, ensure_skin_palette_gpu};

    let world = scene.world();
    let reg_lock = this.bridges.scene.primitives();
    let reg = reg_lock.read();
    let mats_lock = this.bridges.scene.materials();
    let mats = mats_lock.read();
    let visibility_settings = primitive_visibility_settings(runtime);

    for (entity, prim, global) in world.query2::<Primitive, GlobalTransform>() {
        let Some(skin) = world.get::<crate::gameplay::PlayerSkinBinding>(entity) else {
            continue;
        };
        if !display_visible_in_mode(world, entity, runtime) {
            continue;
        }
        let render_model = crate::gameplay::player_render_model_matrix(world, entity, global.0);
        let skip_equipped_weapon_skin = world
            .get::<crate::gameplay::PlayerModelBinding>(skin.owner)
            .is_none();
        let Some(pose) = world.get::<crate::gameplay::PlayerSkinPose>(skin.owner) else {
            continue;
        };
        if pose.palette.is_empty() {
            continue;
        }

        if runtime && visibility_settings.culling_enabled {
            if let Some(bounds) = world.get::<Bounds>(entity) {
                let (center_ws, radius_ws) = transform_sphere(
                    render_model,
                    bounds.local_sphere.center,
                    bounds.local_sphere.radius,
                );
                if !forward_sphere_visible(
                    camera_position,
                    camera_forward,
                    center_ws,
                    radius_ws,
                    visibility_settings.max_distance,
                    visibility_settings.cone_dot,
                    visibility_settings.near_accept_distance,
                ) {
                    continue;
                }
            }
        }

        let gpu = ensure_primitive_gpu(&reg, prim.id, &mut this.gpu.meshes.prim_cache, r)?;
        if skip_equipped_weapon_skin {
            // Keep the crash-isolation policy without synchronous per-frame tracing.
            continue;
        }
        let skin_gpu = ensure_player_skin_gpu(
            &mut this.gpu.meshes.skin_vertex_cache,
            prim.id,
            gpu,
            skin,
            r,
        )?;
        if skin_gpu.max_joint_index as usize >= pose.palette.len() {
            return Err(newengine_core::EngineError::other(format!(
                "skinned draw joint index outside palette entity={} primitive={} max_joint={} palette_joints={}",
                entity.stable_u64(),
                prim.id.0,
                skin_gpu.max_joint_index,
                pose.palette.len(),
            )));
        }
        let pose_generation = world
            .get::<crate::gameplay::PlayerModelBinding>(skin.owner)
            .map(|binding| binding.assignment_revision)
            .unwrap_or(0);
        let palette_gpu = ensure_skin_palette_gpu(
            &mut this.gpu.meshes.skin_palette_cache,
            &mut this.gpu.lifetimes.resources,
            skin.owner.stable_u64(),
            pose_generation,
            pose,
            lit.skin_bgl,
            this.frame.frame_index,
            r,
        )?;

        let material_ref = world
            .get::<newengine_materials::MaterialRef>(entity)
            .copied();
        let resolved = material_ref.and_then(|reference| mats.resolve(reference.id));
        let material_plan = LitMaterialPlan::from_resolved(resolved.as_ref(), prim.color);
        let base_texture = if let Some(path) = material_plan.base_color_texture {
            let Some(texture) = this.material_texture_if_ready(r, path, "render.skinned_character")
            else {
                // A declared character albedo is semantic content, not an optional detail.
                // Drawing it with the generic white texture turns skin/eyes into a grey PBR
                // fallback and hides residency failures. Omit this part until the authored
                // base texture is genuinely resident; neutral normal/roughness fallbacks are
                // still safe below because they do not replace the character's color identity.
                continue;
            };
            texture
        } else {
            lit.white_texture
        };
        let normal_texture = this.material_texture_or_default(
            r,
            material_plan.normal_texture,
            lit.flat_normal_texture,
        );
        let roughness_texture =
            this.material_texture_or_default(r, material_plan.roughness_texture, lit.white_texture);
        let sampler = if material_plan.alpha_cutoff > 0.0 {
            lit.clamp_sampler
        } else if material_plan.has_textures() {
            lit.repeat_sampler
        } else {
            lit.clamp_sampler
        };
        let pipeline = match pass {
            SceneMeshPass::Forward if material_plan.double_sided => {
                lit.skinned_double_sided_pipeline
            }
            SceneMeshPass::Forward => lit.skinned_pipeline,
            SceneMeshPass::GBuffer if material_plan.double_sided => {
                lit.gbuffer_skinned_double_sided_pipeline
            }
            SceneMeshPass::GBuffer => lit.gbuffer_skinned_pipeline,
        };
        let receive_shadow_texture =
            if matches!(pass, SceneMeshPass::Forward) && material_plan.receive_shadows {
                shadow_texture
            } else {
                lit.white_texture
            };
        let receive_local_shadow_texture =
            if matches!(pass, SceneMeshPass::Forward) && material_plan.receive_shadows {
                local_shadow_texture
            } else {
                lit.white_texture
            };
        // Per-draw UBOs are host-visible and may still be read by an in-flight frame.
        // Ring the cache key exactly like the skin palette so one character part cannot
        // overwrite the matrix/material UBO that the previous GPU frame is consuming.
        let frame_slot_key = (this.frame.frame_index & 3).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        let ubo_key = instance_batch_ubo_key(
            0x736b_696e_0000_0000 ^ entity.stable_u64() ^ prim.id.0 ^ frame_slot_key,
            pipeline,
            base_texture,
            normal_texture,
            roughness_texture,
            receive_shadow_texture,
            receive_local_shadow_texture,
            sampler,
        );
        let mut per = this.ensure_per_draw_ubo_with_binding(
            r,
            lit,
            ubo_key,
            base_texture,
            normal_texture,
            roughness_texture,
            receive_shadow_texture,
            receive_local_shadow_texture,
            sampler,
        )?;
        per.last_seen_frame = this.frame.frame_index;
        this.gpu.material.per_draw_ubo.insert(ubo_key, per);
        crate::render_controller::module_impl::passes_ubo::write_lit_ubo_ex(
            r,
            per.ubo,
            viewproj * render_model,
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
        this.diagnostics
            .overlay_metrics
            .record_indexed_triangles(gpu.index_count);
    }
    Ok(())
}
