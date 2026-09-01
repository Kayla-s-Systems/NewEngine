use super::*;

pub(super) fn draw_model_components_shadow(
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
    use newengine_core::render::{BufferSlice, DrawIndexedArgs, IndexFormat};

    let world = scene.world();
    let shadow_max_distance = primitive_shadow_max_distance(runtime);
    let shadow_max_distance_sq = shadow_max_distance * shadow_max_distance;

    for (entity, model_component, global) in
        world.query2::<newengine_gameplay_world_runtime::gameplay::ModelRenderComponent, GlobalTransform>()
    {
        if !display_visible_in_mode(world, entity, runtime) {
            continue;
        }
        let render_model = newengine_gameplay_world_runtime::gameplay::player_render_model_matrix(world, entity, global.0);
        let Some(bundle) = this.cached_model_bundle(&model_component.logical_path) else {
            continue;
        };
        let render_options = world
            .get::<MeshRenderOptions>(entity)
            .cloned()
            .unwrap_or_else(|| bundle.configuration.render_options.clone());
        if !primitive_cast_shadows_enabled(&render_options) {
            continue;
        }
        if runtime {
            let model_bounds =
                RuntimeRenderController::model_bundle_bounds(&bundle).or_else(|| {
                    world
                        .get::<Bounds>(entity)
                        .map(|bounds| (bounds.local_sphere.center, bounds.local_sphere.radius))
                });
            if let Some((center, radius)) = model_bounds {
                let (center_ws, radius_ws) = transform_sphere(render_model, center, radius);
                if center_ws.distance_squared(camera_position) > shadow_max_distance_sq
                    || !shadow_caster_visible(this.shadows_current_cull(), center_ws, radius_ws)
                    || !shadow_caster_projected_radius_visible(
                        cascade_index,
                        cascade_texel_world_size,
                        radius_ws,
                    )
                {
                    continue;
                }
            }
        }

        let tint = world
            .get::<newengine_scene_bridge_runtime::scene_bridge::SceneImportedAssetDescriptor>(entity)
            .map(|descriptor| descriptor.tint)
            .unwrap_or([1.0, 1.0, 1.0, 1.0]);
        let force_double_sided = matches!(
            render_options.cull_policy,
            newengine_model_domain_api::MeshCullPolicy::None
        );

        for (part_index, part) in bundle.parts.iter().enumerate() {
            let primitive_id =
                RuntimeRenderController::model_part_primitive_id(&bundle, part_index);
            let Some(gpu) = this.gpu.meshes.prim_cache.get(&primitive_id).copied() else {
                continue;
            };
            let resolved = newengine_materials::MaterialResolved {
                id: MaterialId::invalid(),
                desc: part.material.descriptor,
                textures: part.material.textures.clone(),
            };
            let mut material_plan =
                LitMaterialPlan::from_resolved(Some(&resolved), part.material.fallback_color);
            if !material_plan.cast_shadows {
                continue;
            }
            for (channel, tint_channel) in material_plan.base_color.iter_mut().zip(tint) {
                *channel *= tint_channel;
            }

            let base_texture = if material_plan.alpha_cutoff > 0.0 {
                this.material_texture_or_default(
                    r,
                    material_plan.base_color_texture,
                    lit.white_texture,
                )
            } else {
                lit.white_texture
            };
            let pipeline = if force_double_sided || material_plan.double_sided {
                lit.shadow_double_sided_pipeline
            } else {
                lit.shadow_pipeline
            };
            let mut ubo_key = 0x6d6f_6465_6c5f_7368u64;
            ubo_key = hash_combine_u64(ubo_key, entity.stable_u64());
            ubo_key = hash_combine_u64(ubo_key, primitive_id.0);
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
            r.set_vertex_buffer(0, BufferSlice::new(gpu.vb, 0))?;
            r.set_index_buffer(BufferSlice::new(gpu.ib, 0), IndexFormat::U32)?;
            r.draw_indexed(DrawIndexedArgs::new(gpu.index_count))?;
        }
    }

    Ok(())
}
