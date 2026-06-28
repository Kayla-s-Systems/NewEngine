
use newengine_core::render::{BindGroupId, BufferSlice, DrawIndexedArgs, IndexFormat, PipelineId, SamplerId, TextureId};
use newengine_math::{Mat4, Vec3};

use newengine_materials::api::{MaterialId, MaterialRegistryApi};
use newengine_primitives::Primitive;
use newengine_transform::GlobalTransform;
use newengine_bounds::Bounds;

use super::super::super::gpu::{ensure_primitive_gpu, PrimitiveGpu};
use super::super::draw_bucket::{BucketedIndexedDrawStream, IndexedDrawPacket};
use super::super::instancing::{
    draw_indexed_instanced_args, InstanceBatchKey, InstanceBatchSet, InstancedReplayState,
    RenderInstanceRaw,
};
use super::super::super::material_bindings::LitMaterialPlan;
use newengine_render_feature_api::PackedLights;
use crate::render_controller::RuntimeRenderController;
use crate::gameplay::display_visible_in_mode;
use crate::scene_bridge::{SkyDomeRuntime, SkyVisualKind, SkyVisualRuntime, TerrainSurfaceLayers};
use super::mesh_visibility::{
    distance_sq_to_camera, forward_sphere_visible, primitive_budget, primitive_forward_max_distance,
    primitive_near_accept_distance, primitive_shadow_max_distance, scene_forward_cone_dot,
    shadow_caster_visible, sort_by_distance_then_key, terrain_budget, terrain_forward_max_distance,
    terrain_near_accept_distance, transform_sphere,
};
use newengine_procedural_noise::ProceduralTerrain;
use newengine_math::collections::FxHashMap;

mod mesh_passes_shadow;
mod scene_mesh_pass;
pub use self::mesh_passes_shadow::{draw_primitives_shadow, draw_procedural_terrain_shadow};
use self::scene_mesh_pass::SceneMeshPass;


#[inline]
fn draw_authored_sky_background_mesh() -> bool {
    crate::env_config::var_bool("NEWENGINE_RENDER_DRAW_SKY_MESH", true)
}

pub(crate) fn publish_camera_spawn(
    bridge: &crate::viewport_bridge::ViewportBridge,
    camera_position: Vec3,
    camera_forward: Vec3,
) {
    bridge.publish_camera_spawn(camera_position, camera_forward);
}

#[derive(Clone)]
struct TerrainDrawEntry {
    distance_sq: f32,
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
    draw_procedural_terrain_for_pass(
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
    )
}

pub fn draw_procedural_terrain_gbuffer(
    this: &mut RuntimeRenderController,
    r: &mut dyn newengine_core::render::RenderApi,
    scene: &newengine_scene::Scene,
    lit: newengine_material_domain_api::LitPipeline,
    viewproj: Mat4,
    lights: &PackedLights,
    runtime: bool,
    camera_position: Vec3,
    camera_forward: Vec3,
) -> newengine_core::EngineResult<()> {
    draw_procedural_terrain_for_pass(
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
    )
}

fn draw_procedural_terrain_for_pass(
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
            distance_sq: distance_sq_to_camera(gt.0, camera_position),
            entity_key: id.stable_u64(),
            mesh_key,
            base_color: terrain.base_color,
            model: gt.0,
            material: world.get::<newengine_materials::MaterialRef>(id).copied(),
            surface_layers: world.get::<TerrainSurfaceLayers>(id).cloned(),
        });
    }
    entries.sort_by(|a, b| {
        a.distance_sq
            .partial_cmp(&b.distance_sq)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.entity_key.cmp(&b.entity_key))
    });
    entries.truncate(terrain_budget(runtime, false));

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

        let key = entity_key
            ^ 0x7e44_1000_0000_0000u64
            ^ if pass.is_gbuffer() { 0x0000_0000_0b00_0000u64 } else { 0 };
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
                    if pass.is_gbuffer() { lit.gbuffer_terrain_pipeline } else { lit.terrain_pipeline },
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
                let pipeline = if pass.is_gbuffer() {
                    if material_plan.double_sided {
                        lit.gbuffer_double_sided_pipeline
                    } else {
                        lit.gbuffer_pipeline
                    }
                } else if material_plan.double_sided {
                    lit.double_sided_pipeline
                } else {
                    lit.pipeline
                };
                (
                    pipeline,
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
            if pass.is_gbuffer() { lit.white_texture } else { shadow_texture },
            sampler,
        )?;
        per.last_seen_frame = this.frame.frame_index;
        this.gpu.material.per_draw_ubo.insert(key, per);

        super::super::passes_ubo::write_lit_ubo_ex(
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
    let local_offset = Vec3::new(cols[12], cols[13], cols[14]);
    cols[12] = camera_position.x + local_offset.x;
    cols[13] = camera_position.y + local_offset.y;
    cols[14] = camera_position.z + local_offset.z;
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
    )
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
) -> newengine_core::EngineResult<()> {
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
) -> newengine_core::EngineResult<()> {
    let world = scene.world();
    let reg_lock = this.bridges.scene.primitives();
    let reg = reg_lock.read();
    let mats_lock = this.bridges.scene.materials();
    let mats = mats_lock.read();

    let mut sky_entries: Vec<(f32, u64, Primitive, Mat4, Option<newengine_materials::MaterialRef>, bool, bool)> = Vec::new();
    let mut entries: Vec<(f32, u64, Primitive, Mat4, Option<newengine_materials::MaterialRef>, bool, bool)> = Vec::new();
    let mut sky_seen = 0usize;
    let mut sky_profile_culled = 0usize;
    for (id, prim, gt) in world.query2::<Primitive, GlobalTransform>() {
        if !display_visible_in_mode(world, id, runtime) {
            continue;
        }
        let sky_visual_kind = world.get::<SkyVisualRuntime>(id).map(|visual| visual.kind);
        let background_sky = sky_visual_kind == Some(SkyVisualKind::Dome);
        let follow_camera_sky = world
            .get::<SkyDomeRuntime>(id)
            .map(|sky| sky.follow_camera)
            .unwrap_or(false);
        if follow_camera_sky {
            sky_seen += 1;
        }
        // Diagnostics as Truth: the authored sky dome is a valid scene asset,
        // but it must not be replayed through the generic lit primitive path by
        // default. When the camera sits inside a follow-camera dome, any depth,
        // winding or pipeline-state mismatch turns the whole frame into a
        // camera-dependent single-color sky sample. Until a dedicated sky pass
        // owns this draw explicitly, the runtime uses SkyClearColorRuntime as
        // the background and leaves the world draw list authoritative.
        let authored_sky_background_mesh_enabled = draw_authored_sky_background_mesh();
        if follow_camera_sky
            && (!this.runtime_profile().draw_sky_visuals()
                || (background_sky && !authored_sky_background_mesh_enabled))
        {
            sky_profile_culled += 1;
            if background_sky && this.frame.frame_index <= 2 {
                newengine_ulog_api::ulog::info!(
                    "sky.draw_list: authored background dome skipped policy='clear-color-background until dedicated sky pass' reason='prevents follow-camera dome from covering world frame' env='NEWENGINE_RENDER_DRAW_SKY_MESH=1 to opt in'"
                );
            }
            continue;
        }
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
        let entry = (
            if follow_camera_sky { 0.0 } else { distance_sq_to_camera(gt.0, camera_position) },
            key,
            *prim,
            gt.0,
            world.get::<newengine_materials::MaterialRef>(id).copied(),
            follow_camera_sky,
            background_sky,
        );
        if follow_camera_sky {
            sky_entries.push(entry);
        } else {
            entries.push(entry);
        }
    }
    sort_by_distance_then_key(&mut sky_entries);
    sort_by_distance_then_key(&mut entries);
    entries.truncate(primitive_budget(runtime, false));
    if runtime && (this.frame.frame_index <= 8 || this.frame.frame_index % 240 == 0) {
        newengine_ulog_api::ulog::debug!(
            "sky.draw_list: seen={} emitted={} profile_culled={} pass='viewport_forward' depth_write=false shadow=false follow_camera=true dome_background_mesh=true opaque_candidates={} opaque_budget={} runtime_profile_sky_native={}",
            sky_seen,
            sky_entries.len(),
            sky_profile_culled,
            entries.len(),
            primitive_budget(runtime, false),
            this.runtime_profile().draw_sky_visuals()
        );
    }
    let mut plan_cache: FxHashMap<PrimitivePlanKey, PrimitiveGpuPlan> = FxHashMap::default();
    let mut sky_background_batches = InstanceBatchSet::default();
    let mut sky_foreground_batches = InstanceBatchSet::default();
    let mut opaque_batches = InstanceBatchSet::default();

    // Keep sky in ordered replay buckets. `InstanceBatchSet` sorts by pipeline / bind
    // group / mesh for performance, so the dome must not share the same unordered
    // set with sun/moon discs: draw authored dome first, sky foreground discs next,
    // then world opaque batches.
    for (_distance_sq, _entity_key, prim, model, material_ref, follow_camera_sky, background_sky) in sky_entries.into_iter().chain(entries.into_iter()) {
        let model = if follow_camera_sky {
            recenter_model_translation(model, camera_position)
        } else {
            model
        };
        if pass.is_gbuffer() && follow_camera_sky {
            continue;
        }
        let plan_key = PrimitivePlanKey::new(prim, material_ref, follow_camera_sky, pass.is_gbuffer());
        let plan = if let Some(plan) = plan_cache.get(&plan_key).copied() {
            plan
        } else {
            let gpu = ensure_primitive_gpu(&reg, prim.id, &mut this.gpu.meshes.prim_cache, r)?;
            let resolved = material_ref.and_then(|mr| mats.resolve(mr.id));
            let material_plan = LitMaterialPlan::from_resolved(resolved.as_ref(), prim.color);

            let base_tex = this.material_texture_or_default(r, material_plan.base_color_texture, lit.white_texture);
            let normal_tex = this.material_texture_or_default(r, material_plan.normal_texture, lit.flat_normal_texture);
            let roughness_tex = this.material_texture_or_default(r, material_plan.roughness_texture, lit.white_texture);
            let sampler = if follow_camera_sky {
                lit.clamp_sampler
            } else if material_plan.has_textures() {
                lit.repeat_sampler
            } else {
                lit.clamp_sampler
            };
            let material_shadow_texture = if pass.is_gbuffer()
                || follow_camera_sky
                || !material_plan.receive_shadows
            {
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
            } else if follow_camera_sky {
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
        if follow_camera_sky && background_sky {
            sky_background_batches.push(batch_key, plan.pipeline, plan.bind_group, plan.gpu, instance);
        } else if follow_camera_sky {
            sky_foreground_batches.push(batch_key, plan.pipeline, plan.bind_group, plan.gpu, instance);
        } else {
            opaque_batches.push(batch_key, plan.pipeline, plan.bind_group, plan.gpu, instance);
        }
        this.diagnostics.overlay_metrics.record_indexed_triangles(plan.gpu.index_count);
    }

    if sky_background_batches.is_empty() && sky_foreground_batches.is_empty() && opaque_batches.is_empty() {
        return Ok(());
    }

    let mut replay = InstancedReplayState::default();
    for batch in sky_background_batches
        .into_sorted_batches()
        .into_iter()
        .chain(sky_foreground_batches.into_sorted_batches().into_iter())
        .chain(opaque_batches.into_sorted_batches().into_iter())
    {
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

