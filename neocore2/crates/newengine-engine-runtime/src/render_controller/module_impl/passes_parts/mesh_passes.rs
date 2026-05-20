
use newengine_core::render::{BindGroupId, BufferSlice, DrawIndexedArgs, IndexFormat, PipelineId, SamplerId, TextureId};
use newengine_math::{Mat4, Vec3};

use newengine_materials::api::{MaterialId, MaterialRegistryApi};
use newengine_primitives::Primitive;
use newengine_transform::GlobalTransform;
use newengine_bounds::Bounds;

use super::super::gpu::{ensure_primitive_gpu, PrimitiveGpu};
use super::draw_bucket::{BucketedIndexedDrawStream, IndexedDrawPacket};
use super::instancing::{
    draw_indexed_instanced_args, InstanceBatchKey, InstanceBatchSet, InstancedReplayState,
    RenderInstanceRaw,
};
use super::super::material_bindings::LitMaterialPlan;
use newengine_render_feature_api::PackedLights;
use crate::render_controller::RuntimeRenderController;
use crate::gameplay::display_visible_in_mode;
use crate::scene_bridge::{SkyDomeRuntime, TerrainSurfaceLayers};
use self::mesh_visibility::{
    distance_sq_to_camera, forward_sphere_visible, primitive_budget, primitive_forward_max_distance,
    primitive_near_accept_distance, primitive_shadow_max_distance, scene_forward_cone_dot,
    shadow_caster_visible, sort_by_distance_then_key, terrain_forward_max_distance,
    terrain_near_accept_distance, transform_sphere,
};
use newengine_procedural_noise::ProceduralTerrain;
use newengine_math::collections::FxHashMap;

pub(super) fn publish_camera_spawn(
    bridge: &crate::viewport_bridge::ViewportBridge,
    camera_position: Vec3,
    camera_forward: Vec3,
) {
    bridge.publish_camera_spawn(camera_position, camera_forward);
}

#[derive(Clone)]
struct TerrainDrawEntry {
    entity_key: u64,
    mesh_key: u64,
    base_color: [f32; 4],
    model: Mat4,
    material: Option<newengine_materials::MaterialRef>,
    surface_layers: Option<TerrainSurfaceLayers>,
}

#[derive(Clone)]
struct TerrainShadowEntry {
    entity_key: u64,
    mesh_key: u64,
    base_color: [f32; 4],
    bounds_center: Vec3,
    bounds_radius: f32,
    model: Mat4,
    material: Option<newengine_materials::MaterialRef>,
}


#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct PrimitivePlanKey {
    primitive_id: u64,
    material_id: u64,
    color: [u32; 4],
    follow_camera_sky: bool,
    shadow_pass: bool,
}

impl PrimitivePlanKey {
    #[inline]
    fn new(
        prim: Primitive,
        material_ref: Option<newengine_materials::MaterialRef>,
        follow_camera_sky: bool,
        shadow_pass: bool,
    ) -> Self {
        Self {
            primitive_id: prim.id.0,
            material_id: material_ref.map(|mr| mr.id.raw()).unwrap_or(MaterialId::invalid().raw()),
            color: [
                prim.color[0].to_bits(),
                prim.color[1].to_bits(),
                prim.color[2].to_bits(),
                prim.color[3].to_bits(),
            ],
            follow_camera_sky,
            shadow_pass,
        }
    }
}

#[derive(Clone, Copy)]
struct PrimitiveGpuPlan {
    gpu: PrimitiveGpu,
    pipeline: PipelineId,
    bind_group: BindGroupId,
    base_texture: TextureId,
    normal_texture: TextureId,
    roughness_texture: TextureId,
    shadow_texture: TextureId,
    sampler: SamplerId,
    mesh_key: u64,
    base_color: [f32; 4],
    emissive_radiance: [f32; 3],
    uv_transform: [f32; 4],
    material_params: [f32; 4],
}



pub fn draw_procedural_terrain(
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
) -> newengine_core::EngineResult<()> {
    let world = scene.world();
    let mats_lock = this.bridges.scene.materials();
    let mats = mats_lock.read();

    let mut entries: Vec<TerrainDrawEntry> = Vec::new();
    for (id, terrain, gt) in world.query2::<ProceduralTerrain, GlobalTransform>() {
        if !display_visible_in_mode(world, id, runtime) {
            continue;
        }
        let mesh_key = terrain.mesh_key();
        if runtime {
            let local_bounds = terrain.heightfield.local_bounds();
            let (center_ws, radius_ws) = transform_sphere(
                gt.0,
                local_bounds.center(),
                local_bounds.half_extents().length(),
            );
            if !forward_sphere_visible(
                camera_position,
                camera_forward,
                center_ws,
                radius_ws,
                terrain_forward_max_distance(),
                scene_forward_cone_dot(),
                terrain_near_accept_distance(radius_ws),
            ) {
                continue;
            }
        }
        entries.push(TerrainDrawEntry {
            entity_key: id.stable_u64(),
            mesh_key,
            base_color: terrain.base_color,
            model: gt.0,
            material: world.get::<newengine_materials::MaterialRef>(id).copied(),
            surface_layers: world.get::<TerrainSurfaceLayers>(id).cloned(),
        });
    }
    entries.sort_by(|a, b| a.entity_key.cmp(&b.entity_key));

    let mut stream = BucketedIndexedDrawStream::with_capacity(entries.len());
    for entry in entries {
        let entity_key = entry.entity_key;
        let mesh_key = entry.mesh_key;
        let model = entry.model;
        let material = entry.material;
        let surface_layers = entry.surface_layers;
        let Some(gpu) = this.gpu.meshes.terrain_cache.get(&mesh_key).copied() else {
            // Render residency is advanced by `pump_scene_gpu_residency` before
            // feature extraction. Missing GPU buffers are skipped for this frame
            // instead of being uploaded on the draw-list hot path.
            continue;
        };

        let mvp = viewproj * model;
        let resolved = material.and_then(|mr| mats.resolve(mr.id));
        let material_plan = LitMaterialPlan::from_resolved(resolved.as_ref(), entry.base_color);

        let key = entity_key ^ 0x7e44_1000_0000_0000u64;
        let (pipeline, base_tex, normal_tex, roughness_tex, sampler, material_params) =
            if let Some(layers) = surface_layers {
                let forest_tex = this.material_texture_or_default(
                    r,
                    Some(layers.forest_base_texture.as_str()),
                    lit.white_texture,
                );
                let sand_tex = this.material_texture_or_default(
                    r,
                    Some(layers.sand_base_texture.as_str()),
                    lit.white_texture,
                );
                let rock_tex = this.material_texture_or_default(
                    r,
                    Some(layers.rock_base_texture.as_str()),
                    lit.white_texture,
                );
                (
                    lit.terrain_pipeline,
                    forest_tex,
                    sand_tex,
                    rock_tex,
                    lit.repeat_sampler,
                    [
                        layers.patch_scale,
                        layers.blend_softness,
                        material_plan.material_params[1],
                        material_plan.material_params[3],
                    ],
                )
            } else {
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
                let sampler = if material_plan.has_textures() {
                    lit.repeat_sampler
                } else {
                    lit.clamp_sampler
                };
                (
                    if material_plan.double_sided { lit.double_sided_pipeline } else { lit.pipeline },
                    base_tex,
                    normal_tex,
                    roughness_tex,
                    sampler,
                    material_plan.material_params,
                )
            };

        let mut per = this.ensure_per_draw_ubo_with_binding(
            r,
            lit,
            key,
            base_tex,
            normal_tex,
            roughness_tex,
            shadow_texture,
            sampler,
        )?;
        per.last_seen_frame = this.frame.frame_index;
        this.gpu.material.per_draw_ubo.insert(key, per);

        super::passes_ubo::write_lit_ubo_ex(
            r,
            per.ubo,
            mvp,
            model,
            material_plan.base_color,
            material_plan.emissive_radiance,
            material_plan.uv_transform,
            material_params,
            lights,
        )?;

        stream.push(IndexedDrawPacket {
            pipeline,
            bind_group: per.bg,
            vertex: BufferSlice::new(gpu.vb, 0),
            index: BufferSlice::new(gpu.ib, 0),
            index_format: IndexFormat::U32,
            args: DrawIndexedArgs::new(gpu.index_count),
        });
        this.diagnostics.overlay_metrics.record_indexed_triangles(gpu.index_count);
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

#[inline]
fn recenter_model_translation(mut model: Mat4, camera_position: Vec3) -> Mat4 {
    let mut cols = model.to_cols_array();
    cols[12] = camera_position.x;
    cols[13] = camera_position.y;
    cols[14] = camera_position.z;
    model = Mat4::from_cols_array(&cols);
    model
}

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
) -> newengine_core::EngineResult<()> {
    let world = scene.world();
    let reg_lock = this.bridges.scene.primitives();
    let reg = reg_lock.read();
    let mats_lock = this.bridges.scene.materials();
    let mats = mats_lock.read();

    let mut entries: Vec<(f32, u64, Primitive, Mat4, Option<newengine_materials::MaterialRef>, bool)> = Vec::new();
    for (id, prim, gt) in world.query2::<Primitive, GlobalTransform>() {
        if !display_visible_in_mode(world, id, runtime) {
            continue;
        }
        let follow_camera_sky = world
            .get::<SkyDomeRuntime>(id)
            .map(|sky| sky.follow_camera)
            .unwrap_or(false);
        if runtime && !follow_camera_sky {
            if let Some(bounds) = world.get::<Bounds>(id) {
                let (center_ws, radius_ws) = transform_sphere(
                    gt.0,
                    bounds.local_sphere.center,
                    bounds.local_sphere.radius,
                );
                if !forward_sphere_visible(
                    camera_position,
                    camera_forward,
                    center_ws,
                    radius_ws,
                    primitive_forward_max_distance(runtime),
                    scene_forward_cone_dot(),
                    primitive_near_accept_distance(),
                ) {
                    continue;
                }
            } else if distance_sq_to_camera(gt.0, camera_position)
                > primitive_forward_max_distance(runtime) * primitive_forward_max_distance(runtime)
            {
                continue;
            }
        }
        let key = id.stable_u64();
        entries.push((
            if follow_camera_sky { 0.0 } else { distance_sq_to_camera(gt.0, camera_position) },
            key,
            *prim,
            gt.0,
            world.get::<newengine_materials::MaterialRef>(id).copied(),
            follow_camera_sky,
        ));
    }
    sort_by_distance_then_key(&mut entries);
    entries.truncate(primitive_budget(runtime, false));

    let mut plan_cache: FxHashMap<PrimitivePlanKey, PrimitiveGpuPlan> = FxHashMap::default();
    let mut batches = InstanceBatchSet::default();
    for (_distance_sq, _entity_key, prim, model, material_ref, follow_camera_sky) in entries {
        let model = if follow_camera_sky {
            recenter_model_translation(model, camera_position)
        } else {
            model
        };
        let plan_key = PrimitivePlanKey::new(prim, material_ref, follow_camera_sky, false);
        let plan = if let Some(plan) = plan_cache.get(&plan_key).copied() {
            plan
        } else {
            let gpu = ensure_primitive_gpu(&reg, prim.id, &mut this.gpu.meshes.prim_cache, r)?;
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
            per.last_seen_frame = this.frame.frame_index;
            this.gpu.material.per_draw_ubo.insert(ubo_key, per);

            // Instance shaders take transforms/material scalars from the instance
            // buffer. The shared UBO still owns lights, shadow matrix and texture
            // bindings, so one bind group can serve the whole material/mesh bucket.
            // Write it once per unique material/mesh bucket, not once per instance.
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
                uv_transform: material_plan.uv_transform,
                material_params: material_plan.material_params,
            };
            plan_cache.insert(plan_key, plan);
            plan
        };

        let instance = RenderInstanceRaw::new(
            model,
            viewproj * model,
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
        this.diagnostics.overlay_metrics.record_indexed_triangles(plan.gpu.index_count);
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


