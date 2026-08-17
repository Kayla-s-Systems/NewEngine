use super::mesh_passes_primitive::{instance_batch_ubo_key, PrimitiveGpuPlan, PrimitivePlanKey};

use newengine_math::{collections::FxHashSet, hash_combine_u64};

use super::scene_mesh_pass::route_diagnostics_due;

use super::*;

#[inline]
fn shadow_light_view_key(light_viewproj: Mat4) -> u64 {
    let mut h = 0xa5ad_50c5_1a57_0001u64;
    for f in light_viewproj.to_cols_array() {
        h = hash_combine_u64(h, f.to_bits() as u64);
    }
    h
}

#[inline]
fn shadow_caster_projected_radius_visible(
    cascade_index: usize,
    cascade_texel_world_size: f32,
    radius_ws: f32,
) -> bool {
    if cascade_index < 2
        || !cascade_texel_world_size.is_finite()
        || cascade_texel_world_size <= 1.0e-6
    {
        return true;
    }
    let projected_radius_texels = radius_ws.abs() / cascade_texel_world_size;
    let min_radius_texels = if cascade_index >= 3 { 0.90 } else { 0.50 };
    projected_radius_texels >= min_radius_texels
}

pub fn draw_procedural_terrain_shadow(
    this: &mut RuntimeRenderController,
    r: &mut dyn newengine_core::render::RenderApi,
    scene: &newengine_scene::Scene,
    lit: newengine_material_domain_api::LitPipeline,
    light_viewproj: Mat4,
    lights: &PackedLights,
    runtime: bool,
) -> newengine_core::EngineResult<()> {
    let shadow_view_key = shadow_light_view_key(light_viewproj);
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
        let render_options = world
            .get::<MeshRenderOptions>(id)
            .cloned()
            .unwrap_or_else(MeshRenderOptions::terrain_patch);
        if !terrain_cast_shadows_enabled(render_options.shadow_policy) {
            continue;
        }
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
    let terrain_shadow_candidates = entries.len();
    let terrain_shadow_budget = terrain_budget(runtime, true);
    entries.truncate(terrain_shadow_budget);
    if runtime && route_diagnostics_due(this.frame.frame_index) {
        newengine_ulog_api::ulog::debug!(
            "terrain.draw_list: pass='shadow_casters' candidates={} planned={} budget={} policy='terrain casts only when authored ytyp shadow_policy is cast or cast_and_receive'",
            terrain_shadow_candidates,
            entries.len(),
            terrain_shadow_budget,
        );
    }

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
        let (center_ws, radius_ws) =
            transform_sphere(model, entry.bounds_center, entry.bounds_radius);
        if !shadow_caster_visible(this.shadows_current_cull(), center_ws, radius_ws) {
            continue;
        }

        let Some(gpu) = this.gpu.meshes.terrain_cache.get(&mesh_key).copied() else {
            // Shadow pass follows the same render-residency contract as forward:
            // not-ready streamed chunks wait until explicit GPU residency has
            // been advanced outside extraction.
            continue;
        };

        let key = hash_combine_u64(entity_key ^ 0x5a44_1000_0000_0000u64, shadow_view_key);
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
        super::super::super::passes_ubo::write_lit_ubo_ex(
            r,
            per.ubo,
            mvp,
            model,
            material_plan.base_color,
            material_plan.emissive_radiance,
            material_plan.alpha_cutoff,
            material_plan.uv_transform,
            material_plan.material_params,
            lights,
        )?;

        stream.push(IndexedDrawPacket {
            pipeline: if material_plan.double_sided {
                lit.shadow_double_sided_pipeline
            } else {
                lit.shadow_pipeline
            },
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

    let mut entries: Vec<(
        f32,
        u64,
        Primitive,
        Mat4,
        Option<newengine_materials::MaterialRef>,
    )> = Vec::new();
    let mut foliage_entries: Vec<(
        f32,
        u64,
        Primitive,
        Mat4,
        Option<newengine_materials::MaterialRef>,
    )> = Vec::new();
    let mut shadow_seen = 0usize;
    let mut shadow_policy_culled = 0usize;
    let mut shadow_distance_culled = 0usize;
    let mut shadow_light_culled = 0usize;
    let mut shadow_lod_culled = 0usize;
    for (id, prim, gt) in world.query2::<Primitive, GlobalTransform>() {
        shadow_seen = shadow_seen.saturating_add(1);
        if !display_visible_in_mode(world, id, runtime)
            || world.get::<EnvironmentDomeRenderState>(id).is_some()
        {
            continue;
        }
        let render_options = world
            .get::<MeshRenderOptions>(id)
            .cloned()
            .unwrap_or_else(MeshRenderOptions::world_opaque);
        if !primitive_cast_shadows_enabled(&render_options) {
            shadow_policy_culled = shadow_policy_culled.saturating_add(1);
            continue;
        }
        if runtime {
            if let Some(bounds) = world.get::<Bounds>(id) {
                let (center_ws, radius_ws) =
                    transform_sphere(gt.0, bounds.local_sphere.center, bounds.local_sphere.radius);
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
            distance_sq_to_camera(gt.0, camera_position),
            key,
            *prim,
            gt.0,
            world.get::<newengine_materials::MaterialRef>(id).copied(),
        );
        if matches!(
            render_options.role,
            newengine_model_domain_api::MeshRenderRole::FoliageInstanced
        ) {
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
    for (_distance_sq, _entity_key, prim, model, material_ref) in
        foliage_entries.into_iter().chain(entries.into_iter())
    {
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
                this.material_texture_or_default(
                    r,
                    material_plan.base_color_texture,
                    lit.white_texture,
                )
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
                lit.clamp_sampler,
            );

            let mut per = this.ensure_per_draw_ubo_with_binding(
                r,
                lit,
                ubo_key,
                base_texture,
                lit.flat_normal_texture,
                lit.white_texture,
                lit.white_texture,
                lit.clamp_sampler,
            )?;
            per.last_seen_frame = this.frame.frame_index;
            this.gpu.material.per_draw_ubo.insert(ubo_key, per);

            // The light-view transform is instance data, not UBO data. Keep
            // this key stable across shadow refreshes and share it between meshes
            // that use the same alpha texture/pipeline. This avoids allocating a
            // new UBO and bind group for every moving shadow projection.
            if written_ubos.insert(ubo_key) {
                super::super::super::passes_ubo::write_lit_ubo_ex(
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

#[cfg(test)]
mod shadow_caster_lod_tests {
    use super::shadow_caster_projected_radius_visible;

    #[test]
    fn shadow_caster_lod_keeps_near_and_rejects_subtexel_distant_casters() {
        assert!(shadow_caster_projected_radius_visible(0, 0.25, 0.05));
        assert!(shadow_caster_projected_radius_visible(1, 0.25, 0.05));
        assert!(!shadow_caster_projected_radius_visible(2, 1.0, 0.40));
        assert!(shadow_caster_projected_radius_visible(2, 1.0, 0.60));
        assert!(!shadow_caster_projected_radius_visible(3, 1.0, 0.80));
        assert!(shadow_caster_projected_radius_visible(3, 1.0, 1.00));
    }

    #[test]
    fn shadow_caster_lod_disables_itself_without_valid_texel_scale() {
        assert!(shadow_caster_projected_radius_visible(3, 0.0, 0.01));
        assert!(shadow_caster_projected_radius_visible(3, f32::NAN, 0.01));
    }
}
