use newengine_core::render::{
    BindGroupId, BufferSlice, DrawIndexedArgs, IndexFormat, PipelineId, SamplerId, TextureId,
};
use newengine_math::{Mat4, Vec3};

use newengine_bounds::Bounds;
use newengine_materials::api::{MaterialId, MaterialRegistryApi};
use newengine_primitives::Primitive;
use newengine_transform::GlobalTransform;

use super::super::super::gpu::{ensure_primitive_gpu, upload_primitive_mesh, PrimitiveGpu};
use super::super::super::material_bindings::LitMaterialPlan;
use super::super::draw_bucket::{BucketedIndexedDrawStream, IndexedDrawPacket};
use super::super::instancing::{
    diagnostic_instance_token, draw_indexed_instanced_args, InstanceBatchKey, InstanceBatchSet,
    InstancedReplayState, RenderInstanceRaw,
};
use super::mesh_visibility::{
    distance_sq_to_camera, foliage_instance_budget, forward_sphere_visible, primitive_budget,
    primitive_cast_shadows_enabled, primitive_shadow_max_distance, primitive_visibility_settings,
    render_scene_culling_enabled, scene_forward_cone_dot, shadow_caster_visible,
    sort_by_distance_then_key, terrain_budget, terrain_cast_shadows_enabled,
    terrain_forward_max_distance, terrain_near_accept_distance, terrain_receive_shadows_enabled,
    transform_sphere,
};
use crate::gameplay::display_visible_in_mode;
use crate::gameplay::{EnvironmentDomeRenderState, TerrainMaterialLayers};
use crate::render_controller::RuntimeRenderController;
use newengine_math::collections::FxHashMap;
use newengine_model_domain_api::{
    MeshRenderOptions, MeshRenderRole, MeshShadowPolicy, MeshTransformPolicy,
};
use newengine_procedural_noise::ProceduralTerrain;
use newengine_render_feature_api::PackedLights;

mod mesh_passes_primitive;
mod mesh_passes_shadow;
mod scene_mesh_pass;

pub use self::mesh_passes_primitive::{
    draw_asset_preview_bundle, draw_primitives, draw_primitives_gbuffer,
};
pub use self::mesh_passes_shadow::{draw_primitives_shadow, draw_procedural_terrain_shadow};
use self::scene_mesh_pass::{route_diagnostics_due, SceneMeshPass};

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
    surface_layers: Option<TerrainMaterialLayers>,
    shadow_policy: MeshShadowPolicy,
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
    let terrain_culling_enabled = render_scene_culling_enabled();
    let terrain_max_distance = terrain_forward_max_distance();
    let terrain_cone_dot = scene_forward_cone_dot();

    let mut entries: Vec<TerrainDrawEntry> = Vec::new();
    for (id, terrain, gt) in world.query2::<ProceduralTerrain, GlobalTransform>() {
        if !display_visible_in_mode(world, id, runtime) {
            continue;
        }
        let mesh_key = terrain.mesh_key();
        if runtime && terrain_culling_enabled {
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
                terrain_max_distance,
                terrain_cone_dot,
                terrain_near_accept_distance(radius_ws),
            ) {
                continue;
            }
        }
        let render_options = world
            .get::<MeshRenderOptions>(id)
            .cloned()
            .unwrap_or_else(MeshRenderOptions::terrain_patch);
        entries.push(TerrainDrawEntry {
            distance_sq: distance_sq_to_camera(gt.0, camera_position),
            entity_key: id.stable_u64(),
            mesh_key,
            base_color: terrain.base_color,
            model: gt.0,
            material: world.get::<newengine_materials::MaterialRef>(id).copied(),
            surface_layers: world.get::<TerrainMaterialLayers>(id).cloned(),
            shadow_policy: render_options.shadow_policy,
        });
    }
    entries.sort_by(|a, b| {
        a.distance_sq
            .partial_cmp(&b.distance_sq)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.entity_key.cmp(&b.entity_key))
    });
    let terrain_candidate_count = entries.len();
    let terrain_forward_budget = terrain_budget(runtime, false);
    entries.truncate(terrain_forward_budget);
    let terrain_planned_count = entries.len();

    let mut terrain_gpu_missing = 0usize;
    let mut terrain_submitted = 0usize;
    let mut terrain_shadow_bound = false;
    let mut stream = BucketedIndexedDrawStream::with_capacity(entries.len());
    for entry in entries {
        let entity_key = entry.entity_key;
        let mesh_key = entry.mesh_key;
        let model = entry.model;
        let material = entry.material;
        let surface_layers = entry.surface_layers;
        let shadow_policy = entry.shadow_policy;
        let Some(gpu) = this.gpu.meshes.terrain_cache.get(&mesh_key).copied() else {
            // Render residency is advanced by `pump_scene_gpu_residency` before
            // feature extraction. Missing GPU buffers are skipped for this frame
            // instead of being uploaded on the draw-list hot path.
            terrain_gpu_missing = terrain_gpu_missing.saturating_add(1);
            continue;
        };

        let mvp = viewproj * model;
        let resolved = material.and_then(|mr| mats.resolve(mr.id));
        let material_plan = LitMaterialPlan::from_resolved(resolved.as_ref(), entry.base_color);

        let terrain_receive_shadows = terrain_receive_shadows_enabled(shadow_policy);
        terrain_shadow_bound |= !pass.is_gbuffer() && terrain_receive_shadows;
        let terrain_shadow_texture = if pass.is_gbuffer() || !terrain_receive_shadows {
            lit.white_texture
        } else {
            shadow_texture
        };
        let key = entity_key
            ^ 0x7e44_1000_0000_0000u64
            ^ if pass.is_gbuffer() {
                0x0000_0000_0b00_0000u64
            } else {
                0
            }
            ^ ((terrain_shadow_texture.get() as u64) << 32);
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
                    if pass.is_gbuffer() {
                        lit.gbuffer_terrain_pipeline
                    } else {
                        lit.terrain_pipeline
                    },
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
            terrain_shadow_texture,
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
            material_plan.alpha_cutoff,
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
        terrain_submitted = terrain_submitted.saturating_add(1);
        this.diagnostics
            .overlay_metrics
            .record_indexed_triangles(gpu.index_count);
    }
    if runtime && route_diagnostics_due(this.frame.frame_index) {
        newengine_ulog_api::ulog::debug!(
            "terrain.draw_list: pass='{}' candidates={} planned={} submitted={} gpu_missing={} budget={} shadow_texture_bound={} policy='draw all resident demo ring; no terrain pop/degrade budget clamp'",
            pass.label(),
            terrain_candidate_count,
            terrain_planned_count,
            terrain_submitted,
            terrain_gpu_missing,
            terrain_forward_budget,
            terrain_shadow_bound,
        );
    }
    stream.emit_sorted(r)?;

    Ok(())
}
