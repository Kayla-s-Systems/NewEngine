use super::*;

pub fn draw_procedural_terrain_shadow(
    this: &mut RuntimeRenderController,
    r: &mut dyn newengine_core::render::RenderApi,
    scene: &newengine_scene::Scene,
    lit: newengine_material_domain_api::LitPipeline,
    light_viewproj: Mat4,
    lights: &PackedLights,
    runtime: bool,
) -> newengine_core::EngineResult<()> {
    let world = scene.world();
    let mats_lock = this.bridges.scene.materials();
    let mats = mats_lock.read();

    let mut entries: Vec<TerrainShadowEntry> = Vec::new();
    for (id, terrain, gt) in world.query2::<ProceduralTerrain, GlobalTransform>() {
        if !display_visible_in_mode(world, id, runtime) {
            continue;
        }
        let mesh_key = terrain.mesh_key();
        let local_bounds = terrain.heightfield.local_bounds();
        entries.push(TerrainShadowEntry {
            entity_key: id.stable_u64(),
            mesh_key,
            base_color: terrain.base_color,
            bounds_center: local_bounds.center(),
            bounds_radius: local_bounds.half_extents().length(),
            model: gt.0,
            material: world.get::<newengine_materials::MaterialRef>(id).copied(),
        });
    }
    entries.sort_by(|a, b| a.entity_key.cmp(&b.entity_key));

    let mut stream = BucketedIndexedDrawStream::with_capacity(entries.len());
    for entry in entries {
        let entity_key = entry.entity_key;
        let mesh_key = entry.mesh_key;
        let model = entry.model;
        let material = entry.material;
        let resolved = material.and_then(|mr| mats.resolve(mr.id));
        let material_plan = LitMaterialPlan::from_resolved(resolved.as_ref(), entry.base_color);
        if !material_plan.cast_shadows {
            continue;
        }
        let (center_ws, radius_ws) = transform_sphere(model, entry.bounds_center, entry.bounds_radius);
        if !shadow_caster_visible(this.shadows_current_cull(), center_ws, radius_ws) {
            continue;
        }

        let Some(gpu) = this.gpu.meshes.terrain_cache.get(&mesh_key).copied() else {
            // Shadow pass follows the same render-residency contract as forward:
            // not-ready streamed chunks wait until explicit GPU residency has
            // been advanced outside extraction.
            continue;
        };

        let key = entity_key ^ 0x5a44_1000_0000_0000u64;
        let mut per = this.ensure_per_draw_ubo_with_binding(
            r,
            lit,
            key,
            lit.white_texture,
            lit.flat_normal_texture,
            lit.white_texture,
            lit.white_texture,
            lit.clamp_sampler,
        )?;
        per.last_seen_frame = this.frame.frame_index;
        this.gpu.material.per_draw_ubo.insert(key, per);

        let mvp = light_viewproj * model;
        super::super::passes_ubo::write_lit_ubo_ex(
            r,
            per.ubo,
            mvp,
            model,
            material_plan.base_color,
            material_plan.emissive_radiance,
            material_plan.uv_transform,
            material_plan.material_params,
            lights,
        )?;

        stream.push(IndexedDrawPacket {
            pipeline: if material_plan.double_sided { lit.shadow_double_sided_pipeline } else { lit.shadow_pipeline },
            bind_group: per.bg,
            vertex: BufferSlice::new(gpu.vb, 0),
            index: BufferSlice::new(gpu.ib, 0),
            index_format: IndexFormat::U32,
            args: DrawIndexedArgs::new(gpu.index_count),
        });
    }
    stream.emit_sorted(r)?;

    Ok(())
}

pub fn draw_primitives_shadow(
    this: &mut RuntimeRenderController,
    r: &mut dyn newengine_core::render::RenderApi,
    scene: &newengine_scene::Scene,
    lit: newengine_material_domain_api::LitPipeline,
    light_viewproj: Mat4,
    lights: &PackedLights,
    runtime: bool,
    camera_position: Vec3,
) -> newengine_core::EngineResult<()> {
    let world = scene.world();
    let reg_lock = this.bridges.scene.primitives();
    let reg = reg_lock.read();
    let mats_lock = this.bridges.scene.materials();
    let mats = mats_lock.read();

    let mut entries: Vec<(f32, u64, Primitive, Mat4, Option<newengine_materials::MaterialRef>)> = Vec::new();
    for (id, prim, gt) in world.query2::<Primitive, GlobalTransform>() {
        if !display_visible_in_mode(world, id, runtime) || world.get::<SkyDomeRuntime>(id).is_some() {
            continue;
        }
        if runtime {
            if let Some(bounds) = world.get::<Bounds>(id) {
                let (center_ws, radius_ws) = transform_sphere(
                    gt.0,
                    bounds.local_sphere.center,
                    bounds.local_sphere.radius,
                );
                if center_ws.distance_squared(camera_position)
                    > primitive_shadow_max_distance(runtime) * primitive_shadow_max_distance(runtime)
                {
                    continue;
                }
                if !shadow_caster_visible(this.shadows_current_cull(), center_ws, radius_ws) {
                    continue;
                }
            }
        }
        let key = id.stable_u64();
        entries.push((
            distance_sq_to_camera(gt.0, camera_position),
            key,
            *prim,
            gt.0,
            world.get::<newengine_materials::MaterialRef>(id).copied(),
        ));
    }
    sort_by_distance_then_key(&mut entries);
    entries.truncate(primitive_budget(runtime, true));

    let mut plan_cache: FxHashMap<PrimitivePlanKey, PrimitiveGpuPlan> = FxHashMap::default();
    let mut batches = InstanceBatchSet::default();
    for (_distance_sq, _entity_key, prim, model, material_ref) in entries {
        let plan_key = PrimitivePlanKey::new(prim, material_ref, false, true);
        let plan = if let Some(plan) = plan_cache.get(&plan_key).copied() {
            plan
        } else {
            let resolved = material_ref.and_then(|mr| mats.resolve(mr.id));
            let material_plan = LitMaterialPlan::from_resolved(resolved.as_ref(), prim.color);
            if !material_plan.cast_shadows {
                continue;
            }

            let gpu = ensure_primitive_gpu(&reg, prim.id, &mut this.gpu.meshes.prim_cache, r)?;
            let (center_ws, radius_ws) = transform_sphere(model, gpu.bounds_center, gpu.bounds_radius);
            if !shadow_caster_visible(this.shadows_current_cull(), center_ws, radius_ws) {
                continue;
            }
            let pipeline = if material_plan.double_sided {
                lit.shadow_instanced_double_sided_pipeline
            } else {
                lit.shadow_instanced_pipeline
            };
            let mesh_key = prim.id.0;
            let ubo_key = instance_batch_ubo_key(
                0x5b1d_5a50_0000_0000,
                pipeline,
                mesh_key,
                lit.white_texture,
                lit.flat_normal_texture,
                lit.white_texture,
                lit.white_texture,
                lit.clamp_sampler,
            );

            let mut per = this.ensure_per_draw_ubo_with_binding(
                r,
                lit,
                ubo_key,
                lit.white_texture,
                lit.flat_normal_texture,
                lit.white_texture,
                lit.white_texture,
                lit.clamp_sampler,
            )?;
            per.last_seen_frame = this.frame.frame_index;
            this.gpu.material.per_draw_ubo.insert(ubo_key, per);

            // Shadow instancing also shares one UBO per material/mesh bucket.
            super::super::passes_ubo::write_lit_ubo_ex(
                r,
                per.ubo,
                Mat4::IDENTITY,
                Mat4::IDENTITY,
                [1.0, 1.0, 1.0, 1.0],
                [0.0, 0.0, 0.0],
                [1.0, 1.0, 0.0, 0.0],
                [1.0, 0.75, 0.0, 1.0],
                lights,
            )?;

            let plan = PrimitiveGpuPlan {
                gpu,
                pipeline,
                bind_group: per.bg,
                base_texture: lit.white_texture,
                normal_texture: lit.flat_normal_texture,
                roughness_texture: lit.white_texture,
                shadow_texture: lit.white_texture,
                sampler: lit.clamp_sampler,
                mesh_key,
                base_color: material_plan.base_color,
                emissive_radiance: material_plan.emissive_radiance,
                uv_transform: material_plan.uv_transform,
                material_params: material_plan.material_params,
            };
            plan_cache.insert(plan_key, plan);
            plan
        };

        let (center_ws, radius_ws) = transform_sphere(model, plan.gpu.bounds_center, plan.gpu.bounds_radius);
        if !shadow_caster_visible(this.shadows_current_cull(), center_ws, radius_ws) {
            continue;
        }

        let instance = RenderInstanceRaw::new(
            model,
            light_viewproj * model,
            plan.base_color,
            plan.uv_transform,
            plan.material_params,
            plan.emissive_radiance,
        );
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
        batches.push(batch_key, plan.pipeline, plan.bind_group, plan.gpu, instance);
    }

    if batches.is_empty() {
        return Ok(());
    }

    let mut replay = InstancedReplayState::default();
    for batch in batches.into_sorted_batches() {
        let instance_count = batch.instances.len() as u32;
        let instance_slice = this.gpu.meshes.instance_uploader.upload(r, &batch.instances)?;
        replay.set_pipeline(r, batch.pipeline)?;
        replay.set_bind_group0(r, batch.bind_group)?;
        replay.set_vertex_buffer(r, 0, BufferSlice::new(batch.gpu.vb, 0))?;
        replay.set_vertex_buffer(r, 1, instance_slice)?;
        replay.set_index_buffer(r, BufferSlice::new(batch.gpu.ib, 0), IndexFormat::U32)?;
        r.draw_indexed(draw_indexed_instanced_args(batch.gpu.index_count, instance_count))?;
    }

    Ok(())
}


