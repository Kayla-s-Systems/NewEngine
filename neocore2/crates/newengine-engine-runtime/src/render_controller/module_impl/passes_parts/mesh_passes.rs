
use newengine_core::render::{BufferSlice, DrawArgs, DrawIndexedArgs, IndexFormat, PipelineId, SamplerId, TextureId};
use newengine_math::{Mat4, Quat, Vec3, Vec4};

use newengine_lighting::{DirectionalLight, PointLight};
use newengine_materials::api::MaterialRegistryApi;
use newengine_primitives::builtins as prim_builtins;
use newengine_primitives::Primitive;
use newengine_transform::GlobalTransform;

use super::super::gpu::{
    ensure_debug_line_pipeline, ensure_grid, ensure_primitive_gpu, upload_primitive_mesh, GridMeshParams,
};
use super::draw_bucket::{BucketedIndexedDrawStream, IndexedDrawPacket};
use super::instancing::{
    draw_indexed_instanced_args, InstanceBatchKey, InstanceBatchSet, InstancedReplayState,
    RenderInstanceRaw,
};
use super::super::material_bindings::LitMaterialPlan;
use super::grid;
use super::lights::PackedLights;
use super::RuntimeRenderController;
use crate::gameplay::{display_visible_in_mode, CollisionBody, CollisionShape};
use self::mesh_visibility::{distance_sq_to_camera, primitive_budget, sort_by_distance_then_key};
use newengine_procedural_noise::ProceduralTerrain;

pub(super) fn publish_camera_spawn(
    bridge: &crate::viewport_bridge::ViewportBridge,
    rig: &newengine_camera::CameraRig,
) {
    let view = rig.view_matrix();
    let inv_view = view.inverse();
    let cam_pos = Vec3::new(inv_view.w_axis.x, inv_view.w_axis.y, inv_view.w_axis.z);
    let cam_fwd = -Vec3::new(inv_view.z_axis.x, inv_view.z_axis.y, inv_view.z_axis.z);
    bridge.publish_camera_spawn(cam_pos, cam_fwd);
}

pub(super) fn draw_grid(
    this: &mut RuntimeRenderController,
    r: &mut dyn newengine_core::render::RenderApi,
    lit: super::super::gpu::LitPipeline,
    viewproj: Mat4,
    rig: &newengine_camera::CameraRig,
    bounds_radius: f32,
    lights: &PackedLights,
) -> newengine_core::EngineResult<()> {
    if !bounds_radius.is_finite() {
        return Ok(());
    }

    let g = ensure_grid(
        &mut this.grid,
        r,
        lit.bgl,
        GridMeshParams {
            half_lines: grid::HALF_LINES,
            major_every: grid::MAJOR_EVERY,
            minor_color: grid::MINOR_COLOR,
            major_color: grid::MAJOR_COLOR,
        },
    )?;

    let grid_model = grid::model_from_camera(rig);

    super::passes_ubo::write_lit_ubo(
        r,
        lit.grid_ubo,
        viewproj * grid_model,
        grid_model,
        [1.0, 1.0, 1.0, 1.0],
        [0.0, 0.0, 0.0],
        lights,
    )?;

    r.set_pipeline(g.pipeline)?;
    r.set_bind_group(0, lit.grid_bg)?;
    r.set_vertex_buffer(0, BufferSlice::new(g.vb, 0))?;
    r.draw(newengine_core::render::DrawArgs::new(g.vertex_count))?;
    this.overlay_metrics.record_vertices_as_triangles(g.vertex_count);
    Ok(())
}

pub(super) fn draw_procedural_terrain(
    this: &mut RuntimeRenderController,
    r: &mut dyn newengine_core::render::RenderApi,
    scene: &newengine_scene::Scene,
    lit: super::super::gpu::LitPipeline,
    viewproj: Mat4,
    lights: &PackedLights,
    shadow_texture: TextureId,
    runtime: bool,
) -> newengine_core::EngineResult<()> {
    let world = scene.world();
    let mats_lock = this.scene_bridge.materials();
    let mats = mats_lock.read();

    let mut entries: Vec<(u64, ProceduralTerrain, Mat4, Option<newengine_materials::MaterialRef>)> = Vec::new();
    for (id, terrain, gt) in world.query2::<ProceduralTerrain, GlobalTransform>() {
        if !display_visible_in_mode(world, id, runtime) {
            continue;
        }
        entries.push((
            id.stable_u64(),
            terrain.clone(),
            gt.0,
            world.get::<newengine_materials::MaterialRef>(id).copied(),
        ));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut stream = BucketedIndexedDrawStream::with_capacity(entries.len());
    for (entity_key, terrain, model, material) in entries {
        let mesh_key = terrain.mesh_key();
        let gpu = if let Some(gpu) = this.terrain_cache.get(&mesh_key).copied() {
            gpu
        } else {
            let mesh = terrain.heightfield.to_primitive_mesh();
            let gpu = upload_primitive_mesh(r, &mesh, "editor_proc_terrain")?;
            this.terrain_cache.insert(mesh_key, gpu);
            gpu
        };

        let mvp = viewproj * model;
        let resolved = material.and_then(|mr| mats.resolve(mr.id));
        let material_plan = LitMaterialPlan::from_resolved(resolved.as_ref(), terrain.base_color);
        let base_tex = this.material_texture_or_default(r, material_plan.base_color_texture, lit.white_texture);
        let normal_tex = this.material_texture_or_default(r, material_plan.normal_texture, lit.flat_normal_texture);
        let roughness_tex = this.material_texture_or_default(r, material_plan.roughness_texture, lit.white_texture);
        let sampler = if material_plan.has_textures() { lit.repeat_sampler } else { lit.clamp_sampler };

        let key = entity_key ^ 0x7e44_1000_0000_0000u64;
        let mut per = this.ensure_per_draw_ubo_with_binding(r, lit, key, base_tex, normal_tex, roughness_tex, shadow_texture, sampler)?;
        per.last_seen_frame = this.frame_index;
        this.per_draw_ubo.insert(key, per);

        super::passes_ubo::write_lit_ubo_ex(
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
            pipeline: if material_plan.double_sided { lit.double_sided_pipeline } else { lit.pipeline },
            bind_group: per.bg,
            vertex: BufferSlice::new(gpu.vb, 0),
            index: BufferSlice::new(gpu.ib, 0),
            index_format: IndexFormat::U32,
            args: DrawIndexedArgs::new(gpu.index_count),
        });
        this.overlay_metrics.record_indexed_triangles(gpu.index_count);
    }
    stream.emit_sorted(r)?;

    Ok(())
}


#[inline]
fn instance_batch_ubo_key(
    prefix: u64,
    pipeline: PipelineId,
    mesh_key: u64,
    base_texture: TextureId,
    normal_texture: TextureId,
    roughness_texture: TextureId,
    shadow_texture: TextureId,
    sampler: SamplerId,
) -> u64 {
    let mut h = prefix;
    h = mix_u64(h, pipeline.get() as u64);
    h = mix_u64(h, mesh_key);
    h = mix_u64(h, base_texture.get() as u64);
    h = mix_u64(h, normal_texture.get() as u64);
    h = mix_u64(h, roughness_texture.get() as u64);
    h = mix_u64(h, shadow_texture.get() as u64);
    mix_u64(h, sampler.get() as u64)
}

#[inline]
fn mix_u64(mut h: u64, v: u64) -> u64 {
    h ^= v.wrapping_add(0x9e37_79b9_7f4a_7c15).wrapping_add(h << 6).wrapping_add(h >> 2);
    h
}

pub(super) fn draw_primitives(
    this: &mut RuntimeRenderController,
    r: &mut dyn newengine_core::render::RenderApi,
    scene: &newengine_scene::Scene,
    lit: super::super::gpu::LitPipeline,
    viewproj: Mat4,
    lights: &PackedLights,
    shadow_texture: TextureId,
    runtime: bool,
    camera_position: Vec3,
) -> newengine_core::EngineResult<()> {
    let world = scene.world();
    let reg_lock = this.scene_bridge.primitives();
    let reg = reg_lock.read();
    let mats_lock = this.scene_bridge.materials();
    let mats = mats_lock.read();

    let mut entries: Vec<(f32, u64, Primitive, Mat4, Option<newengine_materials::MaterialRef>)> = Vec::new();
    for (id, prim, gt) in world.query2::<Primitive, GlobalTransform>() {
        if !display_visible_in_mode(world, id, runtime) {
            continue;
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
    entries.truncate(primitive_budget(runtime, false));

    let mut batches = InstanceBatchSet::default();
    for (_distance_sq, _entity_key, prim, model, material_ref) in entries {
        let gpu = ensure_primitive_gpu(&reg, prim.id, &mut this.prim_cache, r)?;
        let resolved = material_ref.and_then(|mr| mats.resolve(mr.id));
        let material_plan = LitMaterialPlan::from_resolved(resolved.as_ref(), prim.color);

        let base_tex = this.material_texture_or_default(r, material_plan.base_color_texture, lit.white_texture);
        let normal_tex = this.material_texture_or_default(r, material_plan.normal_texture, lit.flat_normal_texture);
        let roughness_tex = this.material_texture_or_default(r, material_plan.roughness_texture, lit.white_texture);
        let sampler = if material_plan.has_textures() { lit.repeat_sampler } else { lit.clamp_sampler };
        let material_shadow_texture = if material_plan.receive_shadows {
            shadow_texture
        } else {
            lit.white_texture
        };
        let pipeline = if material_plan.double_sided {
            lit.instanced_double_sided_pipeline
        } else {
            lit.instanced_pipeline
        };
        let mesh_key = prim.id.0;
        let ubo_key = instance_batch_ubo_key(
            0x1b17_f011_0000_0000,
            pipeline,
            mesh_key,
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
        per.last_seen_frame = this.frame_index;
        this.per_draw_ubo.insert(ubo_key, per);

        // Instance shaders take transforms/material scalars from the instance
        // buffer. The shared UBO still owns lights, shadow matrix and texture
        // bindings, so one bind group can serve the whole material/mesh bucket.
        super::passes_ubo::write_lit_ubo_ex(
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

        let instance = RenderInstanceRaw::new(
            model,
            viewproj * model,
            material_plan.base_color,
            material_plan.uv_transform,
            material_plan.material_params,
            material_plan.emissive_radiance,
        );
        let batch_key = InstanceBatchKey::new(
            pipeline,
            per.bg,
            gpu,
            base_tex,
            normal_tex,
            roughness_tex,
            material_shadow_texture,
            sampler,
            mesh_key,
        );
        batches.push(batch_key, pipeline, per.bg, gpu, instance);
        this.overlay_metrics.record_indexed_triangles(gpu.index_count);
    }

    if batches.is_empty() {
        return Ok(());
    }

    let mut replay = InstancedReplayState::default();
    for batch in batches.into_sorted_batches() {
        let instance_count = batch.instances.len() as u32;
        let instance_slice = this.instance_uploader.upload(r, &batch.instances)?;
        replay.set_pipeline(r, batch.pipeline)?;
        replay.set_bind_group0(r, batch.bind_group)?;
        replay.set_vertex_buffer(r, 0, BufferSlice::new(batch.gpu.vb, 0))?;
        replay.set_vertex_buffer(r, 1, instance_slice)?;
        replay.set_index_buffer(r, BufferSlice::new(batch.gpu.ib, 0), IndexFormat::U32)?;
        r.draw_indexed(draw_indexed_instanced_args(batch.gpu.index_count, instance_count))?;
    }

    Ok(())
}

pub(super) fn draw_procedural_terrain_shadow(
    this: &mut RuntimeRenderController,
    r: &mut dyn newengine_core::render::RenderApi,
    scene: &newengine_scene::Scene,
    lit: super::super::gpu::LitPipeline,
    light_viewproj: Mat4,
    lights: &PackedLights,
    runtime: bool,
) -> newengine_core::EngineResult<()> {
    let world = scene.world();
    let mats_lock = this.scene_bridge.materials();
    let mats = mats_lock.read();

    let mut entries: Vec<(u64, ProceduralTerrain, Mat4, Option<newengine_materials::MaterialRef>)> = Vec::new();
    for (id, terrain, gt) in world.query2::<ProceduralTerrain, GlobalTransform>() {
        if !display_visible_in_mode(world, id, runtime) {
            continue;
        }
        entries.push((
            id.stable_u64(),
            terrain.clone(),
            gt.0,
            world.get::<newengine_materials::MaterialRef>(id).copied(),
        ));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut stream = BucketedIndexedDrawStream::with_capacity(entries.len());
    for (entity_key, terrain, model, material) in entries {
        let resolved = material.and_then(|mr| mats.resolve(mr.id));
        let material_plan = LitMaterialPlan::from_resolved(resolved.as_ref(), terrain.base_color);
        if !material_plan.cast_shadows {
            continue;
        }

        let mesh_key = terrain.mesh_key();
        let gpu = if let Some(gpu) = this.terrain_cache.get(&mesh_key).copied() {
            gpu
        } else {
            let mesh = terrain.heightfield.to_primitive_mesh();
            let gpu = upload_primitive_mesh(r, &mesh, "editor_proc_terrain")?;
            this.terrain_cache.insert(mesh_key, gpu);
            gpu
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
        per.last_seen_frame = this.frame_index;
        this.per_draw_ubo.insert(key, per);

        let mvp = light_viewproj * model;
        super::passes_ubo::write_lit_ubo_ex(
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

pub(super) fn draw_primitives_shadow(
    this: &mut RuntimeRenderController,
    r: &mut dyn newengine_core::render::RenderApi,
    scene: &newengine_scene::Scene,
    lit: super::super::gpu::LitPipeline,
    light_viewproj: Mat4,
    lights: &PackedLights,
    runtime: bool,
    camera_position: Vec3,
) -> newengine_core::EngineResult<()> {
    let world = scene.world();
    let reg_lock = this.scene_bridge.primitives();
    let reg = reg_lock.read();
    let mats_lock = this.scene_bridge.materials();
    let mats = mats_lock.read();

    let mut entries: Vec<(f32, u64, Primitive, Mat4, Option<newengine_materials::MaterialRef>)> = Vec::new();
    for (id, prim, gt) in world.query2::<Primitive, GlobalTransform>() {
        if !display_visible_in_mode(world, id, runtime) {
            continue;
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

    let mut batches = InstanceBatchSet::default();
    for (_distance_sq, _entity_key, prim, model, material_ref) in entries {
        let resolved = material_ref.and_then(|mr| mats.resolve(mr.id));
        let material_plan = LitMaterialPlan::from_resolved(resolved.as_ref(), prim.color);
        if !material_plan.cast_shadows {
            continue;
        }

        let gpu = ensure_primitive_gpu(&reg, prim.id, &mut this.prim_cache, r)?;
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
        per.last_seen_frame = this.frame_index;
        this.per_draw_ubo.insert(ubo_key, per);

        super::passes_ubo::write_lit_ubo_ex(
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

        let instance = RenderInstanceRaw::new(
            model,
            light_viewproj * model,
            material_plan.base_color,
            material_plan.uv_transform,
            material_plan.material_params,
            material_plan.emissive_radiance,
        );
        let batch_key = InstanceBatchKey::new(
            pipeline,
            per.bg,
            gpu,
            lit.white_texture,
            lit.flat_normal_texture,
            lit.white_texture,
            lit.white_texture,
            lit.clamp_sampler,
            mesh_key,
        );
        batches.push(batch_key, pipeline, per.bg, gpu, instance);
    }

    if batches.is_empty() {
        return Ok(());
    }

    let mut replay = InstancedReplayState::default();
    for batch in batches.into_sorted_batches() {
        let instance_count = batch.instances.len() as u32;
        let instance_slice = this.instance_uploader.upload(r, &batch.instances)?;
        replay.set_pipeline(r, batch.pipeline)?;
        replay.set_bind_group0(r, batch.bind_group)?;
        replay.set_vertex_buffer(r, 0, BufferSlice::new(batch.gpu.vb, 0))?;
        replay.set_vertex_buffer(r, 1, instance_slice)?;
        replay.set_index_buffer(r, BufferSlice::new(batch.gpu.ib, 0), IndexFormat::U32)?;
        r.draw_indexed(draw_indexed_instanced_args(batch.gpu.index_count, instance_count))?;
    }

    Ok(())
}


