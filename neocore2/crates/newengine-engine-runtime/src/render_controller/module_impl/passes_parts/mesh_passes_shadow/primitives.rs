use super::*;

type ShadowPrimitiveEntry = (
    f32,
    u64,
    Primitive,
    Mat4,
    Option<newengine_materials::MaterialRef>,
    Option<newengine_model_domain_api::FoliageInstanceRuntime>,
);

pub(super) fn draw_primitives_shadow_body(
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
) -> newengine_core::EngineResult<()> {
    let world = scene.world();
    let reg_lock = this.bridges.scene.primitives();
    let reg = reg_lock.read();
    let mats_lock = this.bridges.scene.materials();
    let mats = mats_lock.read();
    let shadow_max_distance = primitive_shadow_max_distance(runtime);
    let shadow_max_distance_sq = shadow_max_distance * shadow_max_distance;

    let mut entries: Vec<ShadowPrimitiveEntry> = Vec::new();
    let mut foliage_entries: Vec<ShadowPrimitiveEntry> = Vec::new();
    let mut shadow_seen = 0usize;
    let mut shadow_policy_culled = 0usize;
    let mut shadow_distance_culled = 0usize;
    let mut shadow_light_culled = 0usize;
    let mut shadow_lod_culled = 0usize;
    for (id, prim, gt) in world.query2::<Primitive, GlobalTransform>() {
        shadow_seen = shadow_seen.saturating_add(1);
        if world
            .get::<crate::gameplay::PlayerSkinBinding>(id)
            .is_some()
        {
            continue;
        }
        if !display_visible_in_mode(world, id, runtime)
            || world.get::<EnvironmentDomeRenderState>(id).is_some()
        {
            continue;
        }
        let render_model = crate::gameplay::player_render_model_matrix(world, id, gt.0);
        let render_options = world
            .get::<MeshRenderOptions>(id)
            .cloned()
            .unwrap_or_else(MeshRenderOptions::world_opaque);
        if !primitive_cast_shadows_enabled(&render_options) {
            shadow_policy_culled = shadow_policy_culled.saturating_add(1);
            continue;
        }
        let foliage_role = matches!(
            render_options.role,
            newengine_model_domain_api::MeshRenderRole::FoliageInstanced
        );
        if foliage_role {
            if let Some(foliage) =
                world.get::<newengine_model_domain_api::FoliageInstanceRuntime>(id)
            {
                let distance = distance_sq_to_camera(render_model, camera_position).sqrt();
                if !foliage.is_visible(distance, true) {
                    shadow_distance_culled = shadow_distance_culled.saturating_add(1);
                    continue;
                }
            }
        }
        if runtime {
            if let Some(bounds) = world.get::<Bounds>(id) {
                let (center_ws, radius_ws) = transform_sphere(
                    render_model,
                    bounds.local_sphere.center,
                    bounds.local_sphere.radius,
                );
                if center_ws.distance_squared(camera_position) > shadow_max_distance_sq {
                    shadow_distance_culled = shadow_distance_culled.saturating_add(1);
                    continue;
                }
                if !shadow_caster_visible(this.shadows_current_cull(), center_ws, radius_ws) {
                    shadow_light_culled = shadow_light_culled.saturating_add(1);
                    continue;
                }
                if !shadow_caster_projected_radius_visible(
                    cascade_index,
                    cascade_texel_world_size,
                    radius_ws,
                ) {
                    shadow_lod_culled = shadow_lod_culled.saturating_add(1);
                    continue;
                }
            }
        }
        let key = id.stable_u64();
        let entry = (
            distance_sq_to_camera(render_model, camera_position),
            key,
            *prim,
            render_model,
            world.get::<newengine_materials::MaterialRef>(id).copied(),
            world
                .get::<newengine_model_domain_api::FoliageInstanceRuntime>(id)
                .copied(),
        );
        if foliage_role {
            foliage_entries.push(entry);
        } else {
            entries.push(entry);
        }
    }
    sort_by_distance_then_key(&mut foliage_entries);
    sort_by_distance_then_key(&mut entries);
    let shadow_visible = entries.len().saturating_add(foliage_entries.len());
    let shadow_budget = primitive_budget(runtime, true);
    let foliage_shadow_budget = foliage_instance_budget(runtime, true);
    entries.truncate(shadow_budget);
    foliage_entries.truncate(foliage_shadow_budget);

    let plan_capacity = entries.len().saturating_add(foliage_entries.len());
    let mut plan_cache: FxHashMap<PrimitivePlanKey, PrimitiveGpuPlan> =
        FxHashMap::with_capacity_and_hasher(plan_capacity, Default::default());
    let mut written_ubos = FxHashSet::<u64>::default();
    let mut batches = InstanceBatchSet::default();
    let mut shadow_submitted = 0usize;
    for (_distance_sq, _entity_key, prim, model, material_ref, foliage_runtime) in
        foliage_entries.into_iter().chain(entries)
    {
        let plan_key = PrimitivePlanKey::new(prim, material_ref, false, false, true);
        let plan = if let Some(plan) = plan_cache.get(&plan_key).copied() {
            plan
        } else {
            let resolved = material_ref.and_then(|mr| mats.resolve(mr.id));
            let material_plan = LitMaterialPlan::from_resolved(resolved.as_ref(), prim.color);
            if !material_plan.cast_shadows {
                continue;
            }

            let gpu = ensure_primitive_gpu(&reg, prim.id, &mut this.gpu.meshes.prim_cache, r)?;
            let (center_ws, radius_ws) =
                transform_sphere(model, gpu.bounds_center, gpu.bounds_radius);
            if !shadow_caster_visible(this.shadows_current_cull(), center_ws, radius_ws) {
                continue;
            }
            let pipeline = if material_plan.double_sided {
                lit.shadow_instanced_double_sided_pipeline
            } else {
                lit.shadow_instanced_pipeline
            };
            let base_texture = if material_plan.alpha_cutoff > 0.0 {
                let Some(path) = material_plan.base_color_texture else {
                    // An alpha-tested material without its authored opacity/base
                    // texture cannot produce a valid cutout silhouette.
                    continue;
                };
                let Some(texture) =
                    this.material_texture_if_ready(r, path, "render.shadow_foliage")
                else {
                    // Never cast a full rectangular card from the white fallback
                    // while a leaf/grass atlas is still streaming.
                    continue;
                };
                texture
            } else {
                lit.white_texture
            };
            let mesh_key = prim.id.0;
            let ubo_key = instance_batch_ubo_key(
                0x5b1d_5a50_0000_0000,
                pipeline,
                base_texture,
                lit.flat_normal_texture,
                lit.white_texture,
                lit.white_texture,
                lit.white_texture,
                lit.clamp_sampler,
            );

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

            // The light-view transform is instance data, not UBO data. Keep
            // this key stable across shadow refreshes and share it between meshes
            // that use the same alpha texture/pipeline. This avoids allocating a
            // new UBO and bind group for every moving shadow projection.
            if written_ubos.insert(ubo_key) {
                crate::render_controller::module_impl::passes_ubo::write_lit_ubo_ex(
                    r,
                    per.ubo,
                    Mat4::IDENTITY,
                    Mat4::IDENTITY,
                    [1.0, 1.0, 1.0, 1.0],
                    [0.0, 0.0, 0.0],
                    0.0,
                    [1.0, 1.0, 0.0, 0.0],
                    [1.0, 0.75, 0.0, 1.0],
                    lights,
                )?;
            }

            let plan = PrimitiveGpuPlan {
                gpu,
                pipeline,
                bind_group: per.bg,
                base_texture,
                normal_texture: lit.flat_normal_texture,
                roughness_texture: lit.white_texture,
                shadow_texture: lit.white_texture,
                sampler: lit.clamp_sampler,
                mesh_key,
                base_color: material_plan.base_color,
                emissive_radiance: material_plan.emissive_radiance,
                alpha_cutoff: material_plan.alpha_cutoff,
                uv_transform: material_plan.uv_transform,
                material_params: material_plan.material_params,
            };
            plan_cache.insert(plan_key, plan);
            plan
        };

        let (center_ws, radius_ws) =
            transform_sphere(model, plan.gpu.bounds_center, plan.gpu.bounds_radius);
        if !shadow_caster_visible(this.shadows_current_cull(), center_ws, radius_ws) {
            continue;
        }
        if !shadow_caster_projected_radius_visible(
            cascade_index,
            cascade_texel_world_size,
            radius_ws,
        ) {
            continue;
        }

        let instance = RenderInstanceRaw::new(
            model,
            light_viewproj * model,
            plan.base_color,
            plan.uv_transform,
            plan.material_params,
            plan.emissive_radiance,
            plan.alpha_cutoff,
            prim.id.0,
        );
        let instance = if let Some(wind) = foliage_runtime {
            instance.with_foliage_wind(wind.wind_enabled, wind.wind_direction, wind.wind_strength)
        } else {
            instance
        };
        let batch_key = InstanceBatchKey::new(
            plan.pipeline,
            plan.bind_group,
            plan.gpu,
            plan.base_texture,
            plan.normal_texture,
            plan.roughness_texture,
            plan.shadow_texture,
            plan.sampler,
            plan.mesh_key,
        );
        batches.push(
            batch_key,
            plan.pipeline,
            plan.bind_group,
            plan.gpu,
            instance,
        );
        shadow_submitted = shadow_submitted.saturating_add(1);
    }

    let shadow_log_due = runtime && route_diagnostics_due(this.frame.frame_index);
    let shadow_batch_count = batches.batch_count();
    let shadow_instance_count = batches.instance_count();
    if batches.is_empty() {
        if shadow_log_due {
            newengine_ulog_api::ulog::debug!(
                "primitive.draw_list: pass='shadow_casters' seen={} visible={} submitted=0 policy_culled={} distance_culled={} light_culled={} lod_culled={} budget={} foliage_budget={} plans={} shared_ubos={} batches={} instances={} policy='MeshShadowPolicy + stable light-space cull + shared texture-set UBO'",
                shadow_seen,
                shadow_visible,
                shadow_policy_culled,
                shadow_distance_culled,
                shadow_light_culled,
                shadow_lod_culled,
                shadow_budget,
                foliage_shadow_budget,
                plan_cache.len(),
                written_ubos.len(),
                shadow_batch_count,
                shadow_instance_count,
            );
        }
        return Ok(());
    }

    let ordered_batches = batches.into_sorted_batches();
    let packed_upload = this
        .gpu
        .meshes
        .instance_uploader
        .upload_batches(r, &ordered_batches)?;

    let mut replay = InstancedReplayState::default();
    for (batch, instance_slice) in ordered_batches
        .into_iter()
        .zip(packed_upload.slices.iter().copied())
    {
        let instance_count = batch.instances.len() as u32;
        replay.set_pipeline(r, batch.pipeline)?;
        replay.set_bind_group0(r, batch.bind_group)?;
        replay.set_vertex_buffer(r, 0, BufferSlice::new(batch.gpu.vb, 0))?;
        replay.set_vertex_buffer(r, 1, instance_slice)?;
        replay.set_index_buffer(r, BufferSlice::new(batch.gpu.ib, 0), IndexFormat::U32)?;
        r.draw_indexed(draw_indexed_instanced_args(
            batch.gpu.index_count,
            instance_count,
        ))?;
    }

    if shadow_log_due {
        newengine_ulog_api::ulog::debug!(
            "primitive.draw_list: pass='shadow_casters' seen={} visible={} submitted={} policy_culled={} distance_culled={} light_culled={} lod_culled={} budget={} foliage_budget={} plans={} shared_ubos={} batches={} instances={} upload_writes=1 upload_bytes={} policy='MeshShadowPolicy + stable light-space cull + shared texture-set UBO + packed instance upload'",
            shadow_seen,
            shadow_visible,
            shadow_submitted,
            shadow_policy_culled,
            shadow_distance_culled,
            shadow_light_culled,
            shadow_lod_culled,
            shadow_budget,
            foliage_shadow_budget,
            plan_cache.len(),
            written_ubos.len(),
            shadow_batch_count,
            shadow_instance_count,
            packed_upload.bytes_written,
        );
    }

    Ok(())
}
