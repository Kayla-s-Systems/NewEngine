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
            lit.white_texture,
            lit.clamp_sampler,
        )?;
        per.last_seen_frame = this.frame.frame_index;
        this.gpu.material.per_draw_ubo.insert(key, per);

        let mvp = light_viewproj * model;
        crate::render_controller::module_impl::passes_ubo::write_lit_ubo_ex(
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
