use super::plan::primitive_role_cull_reason;
use super::*;

pub(crate) fn draw_model_components(
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
    deferred: bool,
) -> newengine_core::EngineResult<()> {
    use newengine_core::render::{BufferSlice, DrawIndexedArgs, IndexFormat};

    let world = scene.world();
    let visibility_settings = primitive_visibility_settings(runtime);

    for (entity, model_component, global) in
        world.query2::<crate::gameplay::ModelRenderComponent, GlobalTransform>()
    {
        if !display_visible_in_mode(world, entity, runtime) {
            continue;
        }
        let render_model = crate::gameplay::player_render_model_matrix(world, entity, global.0);
        let Some(bundle) = this.cached_model_bundle(&model_component.logical_path) else {
            continue;
        };
        let render_options = world
            .get::<MeshRenderOptions>(entity)
            .cloned()
            .unwrap_or_else(|| bundle.configuration.render_options.clone());
        if primitive_role_cull_reason(
            &render_options,
            pass,
            this.runtime_profile().draw_sky_visuals(),
            deferred,
        )
        .is_some()
        {
            continue;
        }

        if runtime && visibility_settings.culling_enabled {
            let model_bounds =
                RuntimeRenderController::model_bundle_bounds(&bundle).or_else(|| {
                    world
                        .get::<Bounds>(entity)
                        .map(|bounds| (bounds.local_sphere.center, bounds.local_sphere.radius))
                });
            if let Some((center, radius)) = model_bounds {
                let (center_ws, radius_ws) = transform_sphere(render_model, center, radius);
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
            } else if distance_sq_to_camera(render_model, camera_position)
                > visibility_settings.max_distance * visibility_settings.max_distance
            {
                continue;
            }
        }

        let tint = world
            .get::<crate::scene_bridge::SceneImportedAssetDescriptor>(entity)
            .map(|descriptor| descriptor.tint)
            .unwrap_or([1.0, 1.0, 1.0, 1.0]);
        let receives_shadows = matches!(
            render_options.shadow_policy,
            MeshShadowPolicy::ReceiveOnly
                | MeshShadowPolicy::CastAndReceive
                | MeshShadowPolicy::ProfileControlled
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
            for (channel, tint_channel) in material_plan.base_color.iter_mut().zip(tint) {
                *channel *= tint_channel;
            }

            let base_texture = this.material_texture_or_default(
                r,
                material_plan.base_color_texture,
                lit.white_texture,
            );
            let normal_texture = this.material_texture_or_default(
                r,
                material_plan.normal_texture,
                lit.flat_normal_texture,
            );
            let roughness_texture = this.material_texture_or_default(
                r,
                material_plan.roughness_texture,
                lit.white_texture,
            );
            let sampler = if material_plan.alpha_cutoff > 0.0 {
                lit.clamp_sampler
            } else if material_plan.has_textures() {
                lit.repeat_sampler
            } else {
                lit.clamp_sampler
            };
            let pipeline = match pass {
                SceneMeshPass::Forward => {
                    if material_plan.double_sided
                        || matches!(
                            render_options.cull_policy,
                            newengine_model_domain_api::MeshCullPolicy::None
                        )
                    {
                        lit.double_sided_pipeline
                    } else {
                        lit.pipeline
                    }
                }
                SceneMeshPass::GBuffer => {
                    if material_plan.double_sided
                        || matches!(
                            render_options.cull_policy,
                            newengine_model_domain_api::MeshCullPolicy::None
                        )
                    {
                        lit.gbuffer_double_sided_pipeline
                    } else {
                        lit.gbuffer_pipeline
                    }
                }
            };
            let receive_shadow_texture = if matches!(pass, SceneMeshPass::Forward)
                && receives_shadows
                && material_plan.receive_shadows
            {
                shadow_texture
            } else {
                lit.white_texture
            };
            let receive_local_shadow_texture = if matches!(pass, SceneMeshPass::Forward)
                && receives_shadows
                && material_plan.receive_shadows
            {
                local_shadow_texture
            } else {
                lit.white_texture
            };
            let ubo_key = instance_batch_ubo_key(
                0x6d6f_6465_6c00_0000 ^ entity.stable_u64() ^ primitive_id.0,
                pipeline,
                base_texture,
                normal_texture,
                roughness_texture,
                receive_shadow_texture,
                receive_local_shadow_texture,
                sampler,
            );
            let per = this.ensure_per_draw_ubo_with_binding(
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
            r.set_vertex_buffer(0, BufferSlice::new(gpu.vb, 0))?;
            r.set_index_buffer(BufferSlice::new(gpu.ib, 0), IndexFormat::U32)?;
            r.draw_indexed(DrawIndexedArgs::new(gpu.index_count))?;
            this.diagnostics
                .overlay_metrics
                .record_indexed_triangles(gpu.index_count);
        }
    }

    Ok(())
}

pub(crate) fn draw_model_components_wireframe(
    this: &mut RuntimeRenderController,
    r: &mut dyn newengine_core::render::RenderApi,
    scene: &newengine_scene::Scene,
    viewproj: Mat4,
    runtime: bool,
) -> newengine_core::EngineResult<()> {
    use newengine_core::render::{BufferSlice, DrawArgs};
    use newengine_math::Vec4;

    const MAX_WIREFRAME_VERTICES: usize = 240_000;
    let world = scene.world();
    let mut bytes = Vec::<u8>::new();
    let mut vertex_count = 0usize;

    'actors: for (entity, model_component, global) in
        world.query2::<crate::gameplay::ModelRenderComponent, GlobalTransform>()
    {
        if !display_visible_in_mode(world, entity, runtime) {
            continue;
        }
        let render_model = crate::gameplay::player_render_model_matrix(world, entity, global.0);
        let Some(bundle) = this.cached_model_bundle(&model_component.logical_path) else {
            continue;
        };
        let tint = world
            .get::<crate::scene_bridge::SceneImportedAssetDescriptor>(entity)
            .map(|descriptor| descriptor.tint)
            .unwrap_or([1.0, 1.0, 1.0, 1.0]);

        for part in &bundle.parts {
            let fallback = part.material.fallback_color;
            let color = [
                (fallback[0] * tint[0]).clamp(0.12, 1.0),
                (fallback[1] * tint[1]).clamp(0.12, 1.0),
                (fallback[2] * tint[2]).clamp(0.12, 1.0),
                1.0,
            ];
            for triangle in part.mesh.indices.chunks_exact(3) {
                for (a, b) in [
                    (triangle[0], triangle[1]),
                    (triangle[1], triangle[2]),
                    (triangle[2], triangle[0]),
                ] {
                    if vertex_count + 2 > MAX_WIREFRAME_VERTICES {
                        break 'actors;
                    }
                    let Some(a) = part.mesh.vertices.get(a as usize) else {
                        continue;
                    };
                    let Some(b) = part.mesh.vertices.get(b as usize) else {
                        continue;
                    };
                    for vertex in [a, b] {
                        let position = render_model.transform_point3(Vec3::new(
                            vertex.pos[0],
                            vertex.pos[1],
                            vertex.pos[2],
                        ));
                        let clip = viewproj * Vec4::new(position.x, position.y, position.z, 1.0);
                        for value in [
                            clip.x, clip.y, clip.z, clip.w, color[0], color[1], color[2], color[3],
                        ] {
                            bytes.extend_from_slice(&value.to_ne_bytes());
                        }
                        vertex_count += 1;
                    }
                }
            }
        }
    }

    if vertex_count < 2 {
        return Ok(());
    }
    let gpu = crate::render_controller::gpu::ensure_debug_line_pipeline(
        &mut this.gpu.meshes.collision_lines,
        r,
        vertex_count as u32,
    )?;
    r.write_buffer(gpu.vb, 0, &bytes)?;
    r.set_pipeline(gpu.pipeline)?;
    r.set_bind_group(0, gpu.bg)?;
    r.set_vertex_buffer(0, BufferSlice::new(gpu.vb, 0))?;
    r.draw(DrawArgs::new(vertex_count as u32))?;
    Ok(())
}
