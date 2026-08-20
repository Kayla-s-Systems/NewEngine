use super::plan::*;
use super::*;
use newengine_math::collections::FxHashSet;

pub fn draw_primitives(
    this: &mut RuntimeRenderController,
    r: &mut dyn newengine_core::render::RenderApi,
    scene: &newengine_scene::Scene,
    lit: newengine_material_domain_api::LitPipeline,
    viewproj: Mat4,
    lights: &PackedLights,
    shadow_texture: TextureId,
    runtime: bool,
    camera_position: Vec3,
    camera_forward: Vec3,
    deferred: bool,
) -> newengine_core::EngineResult<()> {
    if this.editor_viewport.is_active()
        && this.editor_viewport.shading() == newengine_ui_api::UiEditorViewportShading::Wireframe
    {
        draw_primitives_wireframe(this, r, scene, viewproj, runtime)?;
        return draw_editor_viewport_overlays(this, r, scene, viewproj);
    }
    draw_primitives_for_pass(
        this,
        r,
        scene,
        lit,
        SceneMeshPass::Forward,
        viewproj,
        lights,
        shadow_texture,
        runtime,
        camera_position,
        camera_forward,
        deferred,
    )?;
    draw_editor_viewport_overlays(this, r, scene, viewproj)
}

pub fn draw_primitives_gbuffer(
    this: &mut RuntimeRenderController,
    r: &mut dyn newengine_core::render::RenderApi,
    scene: &newengine_scene::Scene,
    lit: newengine_material_domain_api::LitPipeline,
    viewproj: Mat4,
    lights: &PackedLights,
    runtime: bool,
    camera_position: Vec3,
    camera_forward: Vec3,
    deferred: bool,
) -> newengine_core::EngineResult<()> {
    if this.editor_viewport.is_active()
        && this.editor_viewport.shading() == newengine_ui_api::UiEditorViewportShading::Wireframe
    {
        return Ok(());
    }
    draw_primitives_for_pass(
        this,
        r,
        scene,
        lit,
        SceneMeshPass::GBuffer,
        viewproj,
        lights,
        lit.white_texture,
        runtime,
        camera_position,
        camera_forward,
        deferred,
    )
}

fn draw_primitives_for_pass(
    this: &mut RuntimeRenderController,
    r: &mut dyn newengine_core::render::RenderApi,
    scene: &newengine_scene::Scene,
    lit: newengine_material_domain_api::LitPipeline,
    pass: SceneMeshPass,
    viewproj: Mat4,
    lights: &PackedLights,
    shadow_texture: TextureId,
    runtime: bool,
    camera_position: Vec3,
    camera_forward: Vec3,
    deferred: bool,
) -> newengine_core::EngineResult<()> {
    let world = scene.world();
    let reg_lock = this.bridges.scene.primitives();
    let reg = reg_lock.read();
    let mats_lock = this.bridges.scene.materials();
    let mats = mats_lock.read();
    let stage_profile = runtime
        && (this.frame.frame_index <= 3 || this.frame.frame_index.is_multiple_of(30))
        && crate::runtime_policy::render_runtime_policy().primitive_stage_log;
    let stage_total_started = stage_profile.then(std::time::Instant::now);
    let scan_started = stage_profile.then(std::time::Instant::now);
    let visibility_settings = primitive_visibility_settings(runtime);

    type PrimitiveDrawEntry = (
        f32,
        u64,
        Primitive,
        Mat4,
        Option<newengine_materials::MaterialRef>,
        u8,
        Option<EnvironmentDomeRenderState>,
    );

    let mut sky_entries: Vec<PrimitiveDrawEntry> = Vec::new();
    let mut foliage_entries: Vec<PrimitiveDrawEntry> = Vec::new();
    let mut entries: Vec<PrimitiveDrawEntry> = Vec::new();
    let mut sky_seen = 0usize;
    let mut sky_profile_culled = 0usize;
    for (id, prim, gt) in world.query2::<Primitive, GlobalTransform>() {
        if !display_visible_in_mode(world, id, runtime) {
            continue;
        }
        let sky_dome_runtime = world.get::<EnvironmentDomeRenderState>(id);
        let asset_label = sky_dome_runtime
            .and_then(|sky| sky.asset_ref.as_deref())
            .unwrap_or("primitive://runtime");
        let definition_label = sky_dome_runtime
            .and_then(|sky| sky.definition_ref.as_deref())
            .unwrap_or("<none>");
        let mesh_render_options = primitive_mesh_render_options(world.get::<MeshRenderOptions>(id));
        let draw_flags = primitive_draw_flags(&mesh_render_options);
        let follows_view = has_primitive_flag(draw_flags, PRIMITIVE_DRAW_FOLLOW_VIEW);
        let sky_role = has_primitive_flag(draw_flags, PRIMITIVE_DRAW_SKY_ROLE);
        let background_sky = has_primitive_flag(draw_flags, PRIMITIVE_DRAW_SKY_BACKGROUND);
        if sky_role {
            sky_seen += 1;
        }
        if let Some(culled_reason) = primitive_role_cull_reason(
            &mesh_render_options,
            pass,
            this.runtime_profile().draw_sky_visuals(),
            deferred,
        ) {
            if sky_role {
                sky_profile_culled += 1;
            }
            if runtime && (route_diagnostics_due(this.frame.frame_index)) {
                newengine_ulog_api::ulog::debug!(
                    "mesh.role.route: definition='{}' asset='{}' role={:?} transform_policy={:?} pass='{}' emitted=false culled_reason='{}' asset_source='engine.assets.definitions'",
                    definition_label,
                    asset_label,
                    mesh_render_options.role,
                    mesh_render_options.transform_policy,
                    pass.label(),
                    culled_reason
                );
            }
            if background_sky && this.frame.frame_index <= 2 {
                newengine_ulog_api::ulog::info!(
                    "sky.draw_list: authored background dome skipped policy='mesh-render-role-routing' role={:?} reason='{}'",
                    mesh_render_options.role,
                    culled_reason
                );
            }
            continue;
        }
        if runtime && !follows_view && visibility_settings.culling_enabled {
            if let Some(bounds) = world.get::<Bounds>(id) {
                let (center_ws, radius_ws) =
                    transform_sphere(gt.0, bounds.local_sphere.center, bounds.local_sphere.radius);
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
            } else if distance_sq_to_camera(gt.0, camera_position)
                > visibility_settings.max_distance * visibility_settings.max_distance
            {
                continue;
            }
        }
        let key = id.stable_u64();
        let entry = (
            if follows_view || sky_role {
                0.0
            } else {
                distance_sq_to_camera(gt.0, camera_position)
            },
            key,
            *prim,
            gt.0,
            world.get::<newengine_materials::MaterialRef>(id).copied(),
            draw_flags,
            sky_dome_runtime.cloned(),
        );
        if sky_role {
            sky_entries.push(entry);
        } else if has_primitive_flag(draw_flags, PRIMITIVE_DRAW_FOLIAGE_ROLE) {
            foliage_entries.push(entry);
        } else {
            entries.push(entry);
        }
    }
    sort_by_distance_then_key(&mut sky_entries);
    sort_by_distance_then_key(&mut foliage_entries);
    sort_by_distance_then_key(&mut entries);
    foliage_entries.truncate(foliage_instance_budget(runtime, false));
    entries.truncate(primitive_budget(runtime, false));
    let scan_ms = scan_started
        .map(|started| started.elapsed().as_secs_f64() * 1000.0)
        .unwrap_or(0.0);
    let plan_started = stage_profile.then(std::time::Instant::now);
    if runtime && (route_diagnostics_due(this.frame.frame_index)) {
        newengine_ulog_api::ulog::debug!(
            "sky.draw_list: seen={} emitted={} profile_culled={} pass='viewport_forward' depth_write=false shadow=false route='mesh_render_options' opaque_candidates={} opaque_budget={} draw_sky_visuals={}",
            sky_seen,
            sky_entries.len(),
            sky_profile_culled,
            entries.len(),
            primitive_budget(runtime, false),
            this.runtime_profile().draw_sky_visuals()
        );
    }
    let plan_cache_capacity = sky_entries
        .len()
        .saturating_add(foliage_entries.len())
        .saturating_add(entries.len());
    let mut plan_cache: FxHashMap<PrimitivePlanKey, PrimitiveGpuPlan> =
        FxHashMap::with_capacity_and_hasher(plan_cache_capacity, Default::default());
    let mut written_ubos = FxHashSet::<u64>::default();
    let mut sky_background_batches = InstanceBatchSet::default();
    let mut sky_foreground_batches = InstanceBatchSet::default();
    let mut foliage_batches = InstanceBatchSet::default();
    let mut opaque_batches = InstanceBatchSet::default();

    // Keep sky in ordered replay buckets. `InstanceBatchSet` sorts by pipeline / bind
    // group / mesh for performance, so the dome must not share the same unordered
    // set with sun/moon discs: draw authored dome first, sky foreground discs next,
    // then world opaque batches.
    for (_distance_sq, _entity_key, prim, model, material_ref, draw_flags, sky_runtime) in
        sky_entries
            .into_iter()
            .chain(foliage_entries.into_iter())
            .chain(entries.into_iter())
    {
        let follows_view = has_primitive_flag(draw_flags, PRIMITIVE_DRAW_FOLLOW_VIEW);
        let foliage_role = has_primitive_flag(draw_flags, PRIMITIVE_DRAW_FOLIAGE_ROLE);
        let sky_role = has_primitive_flag(draw_flags, PRIMITIVE_DRAW_SKY_ROLE);
        let background_sky = has_primitive_flag(draw_flags, PRIMITIVE_DRAW_SKY_BACKGROUND);
        let receive_shadows = has_primitive_flag(draw_flags, PRIMITIVE_DRAW_RECEIVE_SHADOWS);
        let model = if follows_view {
            recenter_model_translation(model, camera_position)
        } else {
            model
        };
        if pass.is_gbuffer() && sky_role {
            continue;
        }
        let plan_key = PrimitivePlanKey::new(prim, material_ref, sky_role, pass.is_gbuffer());
        let plan = if let Some(plan) = plan_cache.get(&plan_key).copied() {
            plan
        } else {
            let gpu = ensure_primitive_gpu(&reg, prim.id, &mut this.gpu.meshes.prim_cache, r)?;
            let resolved = material_ref.and_then(|mr| mats.resolve(mr.id));
            let mut material_plan = LitMaterialPlan::from_resolved(resolved.as_ref(), prim.color);
            if let Some(runtime) = sky_runtime.as_ref() {
                material_plan.uv_transform = runtime.uv_transform;
                material_plan.material_params = runtime.material_params;
                material_plan.emissive_radiance = runtime.emissive_params;
            }

            let base_tex = this.material_texture_or_default(
                r,
                material_plan.base_color_texture,
                lit.white_texture,
            );
            let normal_tex = this.material_texture_or_default(
                r,
                material_plan.normal_texture,
                lit.flat_normal_texture,
            );
            let roughness_tex = this.material_texture_or_default(
                r,
                material_plan.roughness_texture,
                lit.white_texture,
            );
            let sampler = if sky_role {
                // Procedural SkyDome noise is tiled in a projected cloud plane.
                lit.repeat_sampler
            } else if material_plan.alpha_cutoff > 0.0 {
                // Alpha-card atlases must not wrap transparent border texels onto
                // the opposite edge. Repeat sampling creates foliage/card seams.
                lit.clamp_sampler
            } else if material_plan.has_textures() {
                lit.repeat_sampler
            } else {
                lit.clamp_sampler
            };
            let material_shadow_texture =
                if pass.is_gbuffer() || !receive_shadows || !material_plan.receive_shadows {
                    lit.white_texture
                } else {
                    shadow_texture
                };
            let pipeline = if pass.is_gbuffer() {
                if material_plan.double_sided {
                    lit.gbuffer_instanced_double_sided_pipeline
                } else {
                    lit.gbuffer_instanced_pipeline
                }
            } else if sky_role {
                lit.sky_instanced_pipeline
            } else if material_plan.double_sided {
                lit.instanced_double_sided_pipeline
            } else {
                lit.instanced_pipeline
            };
            let mesh_key = prim.id.0;
            let ubo_key = instance_batch_ubo_key(
                0x1b17_f011_0000_0000,
                pipeline,
                base_tex,
                normal_tex,
                roughness_tex,
                material_shadow_texture,
                sampler,
            );

            let mut per = this.ensure_per_draw_ubo_with_binding(
                r,
                lit,
                ubo_key,
                base_tex,
                normal_tex,
                roughness_tex,
                material_shadow_texture,
                sampler,
            )?;
            per.last_seen_frame = this.frame.frame_index;
            this.gpu.material.per_draw_ubo.insert(ubo_key, per);

            // Instance transforms and material scalars live in the instance
            // buffer. The UBO depends only on the pipeline texture set and frame
            // lighting, so different meshes can share it safely. Update each
            // shared UBO once per pass/frame instead of once per mesh plan.
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
                base_texture: base_tex,
                normal_texture: normal_tex,
                roughness_texture: roughness_tex,
                shadow_texture: material_shadow_texture,
                sampler,
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

        if runtime
            && matches!(pass, SceneMeshPass::Forward)
            && lights.shadow_view_forward[3] >= 6.5
            && route_diagnostics_due(this.frame.frame_index)
        {
            let cols = model.to_cols_array();
            newengine_ulog_api::ulog::debug!(
                "receiver diagnostic instance: primitive_id={} token24={} origin=({:.3},{:.3},{:.3}) base=({:.3},{:.3},{:.3},{:.3}) material_params=({:.3},{:.3},{:.3},{:.3})",
                prim.id.0,
                diagnostic_instance_token(prim.id.0),
                cols[12], cols[13], cols[14],
                plan.base_color[0], plan.base_color[1], plan.base_color[2], plan.base_color[3],
                plan.material_params[0], plan.material_params[1], plan.material_params[2], plan.material_params[3],
            );
        }

        let instance = RenderInstanceRaw::new(
            model,
            viewproj * model,
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
        if sky_role && background_sky {
            sky_background_batches.push(
                batch_key,
                plan.pipeline,
                plan.bind_group,
                plan.gpu,
                instance,
            );
        } else if sky_role {
            sky_foreground_batches.push(
                batch_key,
                plan.pipeline,
                plan.bind_group,
                plan.gpu,
                instance,
            );
        } else if foliage_role {
            foliage_batches.push(
                batch_key,
                plan.pipeline,
                plan.bind_group,
                plan.gpu,
                instance,
            );
        } else {
            opaque_batches.push(
                batch_key,
                plan.pipeline,
                plan.bind_group,
                plan.gpu,
                instance,
            );
        }
        this.diagnostics
            .overlay_metrics
            .record_indexed_triangles(plan.gpu.index_count);
    }

    if runtime && route_diagnostics_due(this.frame.frame_index) {
        newengine_ulog_api::ulog::debug!(
            "primitive.batch.plan: pass='{}' plans={} shared_ubos={} batches=[sky_bg:{},sky_fg:{},foliage:{},opaque:{}] instances=[sky_bg:{},sky_fg:{},foliage:{},opaque:{}] policy='UBO keyed by pipeline texture set; mesh transform/material scalars stay in instance data'",
            pass.label(),
            plan_cache.len(),
            written_ubos.len(),
            sky_background_batches.batch_count(),
            sky_foreground_batches.batch_count(),
            foliage_batches.batch_count(),
            opaque_batches.batch_count(),
            sky_background_batches.instance_count(),
            sky_foreground_batches.instance_count(),
            foliage_batches.instance_count(),
            opaque_batches.instance_count(),
        );
    }

    let foliage_batch_count = foliage_batches.batch_count();
    let foliage_instance_count = foliage_batches.instance_count();
    if runtime && this.frame.frame_index <= 3 && foliage_instance_count > 0 {
        newengine_ulog_api::ulog::info!(
            "foliage.instance_batch: frame={} pass='{}' gpu_batches={} instances={} policy='MeshRenderRole::FoliageInstanced -> shared source mesh + hardware instance buffer'",
            this.frame.frame_index,
            pass.label(),
            foliage_batch_count,
            foliage_instance_count,
        );
    }

    if sky_background_batches.is_empty()
        && sky_foreground_batches.is_empty()
        && foliage_batches.is_empty()
        && opaque_batches.is_empty()
    {
        return Ok(());
    }

    let plan_ms = plan_started
        .map(|started| started.elapsed().as_secs_f64() * 1000.0)
        .unwrap_or(0.0);
    let upload_started = stage_profile.then(std::time::Instant::now);
    let ordered_batches = sky_background_batches
        .into_sorted_batches()
        .into_iter()
        .chain(sky_foreground_batches.into_sorted_batches())
        .chain(foliage_batches.into_sorted_batches())
        .chain(opaque_batches.into_sorted_batches())
        .collect::<Vec<_>>();
    let packed_upload = this
        .gpu
        .meshes
        .instance_uploader
        .upload_batches(r, &ordered_batches)?;
    let upload_ms = upload_started
        .map(|started| started.elapsed().as_secs_f64() * 1000.0)
        .unwrap_or(0.0);
    let replay_started = stage_profile.then(std::time::Instant::now);

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

    let replay_ms = replay_started
        .map(|started| started.elapsed().as_secs_f64() * 1000.0)
        .unwrap_or(0.0);
    if stage_profile {
        let total_ms = stage_total_started
            .map(|started| started.elapsed().as_secs_f64() * 1000.0)
            .unwrap_or(0.0);
        newengine_ulog_api::ulog::info!(
            "primitive.stage.profile: frame={} pass='{}' total_ms={:.3} scan_ms={:.3} plan_batch_ms={:.3} upload_ms={:.3} replay_ms={:.3} batches={} instances={} bytes={}",
            this.frame.frame_index,
            pass.label(),
            total_ms,
            scan_ms,
            plan_ms,
            upload_ms,
            replay_ms,
            packed_upload.slices.len(),
            packed_upload.instance_count,
            packed_upload.bytes_written,
        );
    }

    if runtime && route_diagnostics_due(this.frame.frame_index) {
        newengine_ulog_api::ulog::debug!(
            "primitive.instance_upload: pass='{}' writes=1 batches={} instances={} bytes={} policy='single packed write per pass'",
            pass.label(),
            packed_upload.slices.len(),
            packed_upload.instance_count,
            packed_upload.bytes_written,
        );
    }

    Ok(())
}

fn draw_primitives_wireframe(
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
    let reg_lock = this.bridges.scene.primitives();
    let reg = reg_lock.read();
    let mut bytes = Vec::<u8>::new();
    let mut vertex_count = 0usize;

    'entities: for (entity, primitive, global) in world.query2::<Primitive, GlobalTransform>() {
        if !display_visible_in_mode(world, entity, runtime) {
            continue;
        }
        let Ok(mesh) = reg.build_mesh(primitive.id) else {
            continue;
        };
        let color = primitive.color;
        let model = global.0;
        for triangle in mesh.indices.chunks_exact(3) {
            let edges = [
                (triangle[0], triangle[1]),
                (triangle[1], triangle[2]),
                (triangle[2], triangle[0]),
            ];
            for (a, b) in edges {
                if vertex_count + 2 > MAX_WIREFRAME_VERTICES {
                    break 'entities;
                }
                for index in [a, b] {
                    let Some(vertex) = mesh.vertices.get(index as usize) else {
                        continue;
                    };
                    let position = model.transform_point3(Vec3::new(
                        vertex.pos[0],
                        vertex.pos[1],
                        vertex.pos[2],
                    ));
                    let clip = viewproj * Vec4::new(position.x, position.y, position.z, 1.0);
                    for value in [
                        clip.x, clip.y, clip.z, clip.w,
                        color[0], color[1], color[2], color[3],
                    ] {
                        bytes.extend_from_slice(&value.to_ne_bytes());
                    }
                    vertex_count += 1;
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

fn draw_editor_viewport_overlays(
    this: &mut RuntimeRenderController,
    r: &mut dyn newengine_core::render::RenderApi,
    scene: &newengine_scene::Scene,
    viewproj: Mat4,
) -> newengine_core::EngineResult<()> {
    use newengine_core::render::{BufferSlice, DrawArgs};
    use newengine_math::Vec4;

    if !this.editor_viewport.is_active() {
        return Ok(());
    }
    let state = this.editor_viewport.state();
    if !state.show_grid && !state.show_bounds && !state.show_collision {
        return Ok(());
    }

    let world = scene.world();
    let mut bytes = Vec::<u8>::new();
    let mut vertex_count = 0usize;
    let mut push_line = |a: Vec3, b: Vec3, color: [f32; 4]| {
        for position in [a, b] {
            let clip = viewproj * Vec4::new(position.x, position.y, position.z, 1.0);
            for value in [
                clip.x, clip.y, clip.z, clip.w,
                color[0], color[1], color[2], color[3],
            ] {
                bytes.extend_from_slice(&value.to_ne_bytes());
            }
            vertex_count += 1;
        }
    };

    if state.show_grid {
        let step = state.translation_snap_units.max(1.0);
        let half_cells = 20i32;
        let extent = step * half_cells as f32;
        let minor = [0.24, 0.26, 0.29, 0.75];
        let major = [0.38, 0.41, 0.45, 0.92];
        for cell in -half_cells..=half_cells {
            let offset = cell as f32 * step;
            let color = if cell == 0 || cell % 5 == 0 { major } else { minor };
            push_line(
                Vec3::new(-extent, 0.0, offset),
                Vec3::new(extent, 0.0, offset),
                color,
            );
            push_line(
                Vec3::new(offset, 0.0, -extent),
                Vec3::new(offset, 0.0, extent),
                color,
            );
        }
    }

    if state.show_bounds {
        if let Some(selected) = this.bridges.scene.selection() {
            if let (Some(bounds), Some(global)) = (
                world.get::<Bounds>(selected),
                world.get::<GlobalTransform>(selected),
            ) {
                let (center, radius) = transform_sphere(
                    global.0,
                    bounds.local_sphere.center,
                    bounds.local_sphere.radius,
                );
                push_wire_cube(&mut push_line, center, Vec3::splat(radius), [1.0, 0.72, 0.12, 1.0]);
            }
        }
    }

    if state.show_collision {
        for (entity, body, global) in world.query2::<crate::gameplay::PhysicsBodyDesc, GlobalTransform>() {
            if world.get::<crate::editor_viewport::EditorGizmoAxisComponent>(entity).is_some() {
                continue;
            }
            let bounds = body.to_bounds();
            let (center, radius) = transform_sphere(
                global.0,
                bounds.local_sphere.center,
                bounds.local_sphere.radius,
            );
            push_wire_cube(
                &mut push_line,
                center,
                Vec3::splat(radius),
                if body.is_trigger() {
                    [0.85, 0.25, 0.86, 1.0]
                } else {
                    [0.20, 0.92, 0.38, 1.0]
                },
            );
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

fn push_wire_cube(
    push_line: &mut impl FnMut(Vec3, Vec3, [f32; 4]),
    center: Vec3,
    half_extents: Vec3,
    color: [f32; 4],
) {
    let h = half_extents;
    let p = [
        center + Vec3::new(-h.x, -h.y, -h.z),
        center + Vec3::new( h.x, -h.y, -h.z),
        center + Vec3::new( h.x,  h.y, -h.z),
        center + Vec3::new(-h.x,  h.y, -h.z),
        center + Vec3::new(-h.x, -h.y,  h.z),
        center + Vec3::new( h.x, -h.y,  h.z),
        center + Vec3::new( h.x,  h.y,  h.z),
        center + Vec3::new(-h.x,  h.y,  h.z),
    ];
    for (a, b) in [
        (0,1),(1,2),(2,3),(3,0),
        (4,5),(5,6),(6,7),(7,4),
        (0,4),(1,5),(2,6),(3,7),
    ] {
        push_line(p[a], p[b], color);
    }
}
