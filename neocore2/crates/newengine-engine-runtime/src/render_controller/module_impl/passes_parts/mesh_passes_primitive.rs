use super::*;
use newengine_math::{collections::FxHashSet, hash_combine_u64};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct PrimitivePlanKey {
    primitive_id: u64,
    material_id: u64,
    color: [u32; 4],
    sky_pipeline: bool,
    shadow_pass: bool,
}

impl PrimitivePlanKey {
    #[inline]
    pub(super) fn new(
        prim: Primitive,
        material_ref: Option<newengine_materials::MaterialRef>,
        sky_pipeline: bool,
        shadow_pass: bool,
    ) -> Self {
        Self {
            primitive_id: prim.id.0,
            material_id: material_ref
                .map(|mr| mr.id.raw())
                .unwrap_or(MaterialId::invalid().raw()),
            color: [
                prim.color[0].to_bits(),
                prim.color[1].to_bits(),
                prim.color[2].to_bits(),
                prim.color[3].to_bits(),
            ],
            sky_pipeline,
            shadow_pass,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct PrimitiveGpuPlan {
    pub(super) gpu: PrimitiveGpu,
    pub(super) pipeline: PipelineId,
    pub(super) bind_group: BindGroupId,
    pub(super) base_texture: TextureId,
    pub(super) normal_texture: TextureId,
    pub(super) roughness_texture: TextureId,
    pub(super) shadow_texture: TextureId,
    pub(super) sampler: SamplerId,
    pub(super) mesh_key: u64,
    pub(super) base_color: [f32; 4],
    pub(super) emissive_radiance: [f32; 3],
    pub(super) alpha_cutoff: f32,
    pub(super) uv_transform: [f32; 4],
    pub(super) material_params: [f32; 4],
}

#[inline]
pub(super) fn instance_batch_ubo_key(
    prefix: u64,
    pipeline: PipelineId,
    base_texture: TextureId,
    normal_texture: TextureId,
    roughness_texture: TextureId,
    shadow_texture: TextureId,
    sampler: SamplerId,
) -> u64 {
    let mut h = prefix;
    h = hash_combine_u64(h, pipeline.get() as u64);
    h = hash_combine_u64(h, base_texture.get() as u64);
    h = hash_combine_u64(h, normal_texture.get() as u64);
    h = hash_combine_u64(h, roughness_texture.get() as u64);
    h = hash_combine_u64(h, shadow_texture.get() as u64);
    hash_combine_u64(h, sampler.get() as u64)
}

const PRIMITIVE_DRAW_FOLLOW_VIEW: u8 = 0x01;
const PRIMITIVE_DRAW_SKY_ROLE: u8 = 0x02;
const PRIMITIVE_DRAW_SKY_BACKGROUND: u8 = 0x04;
const PRIMITIVE_DRAW_RECEIVE_SHADOWS: u8 = 0x08;
#[inline]
fn primitive_mesh_render_options(explicit: Option<&MeshRenderOptions>) -> MeshRenderOptions {
    explicit
        .cloned()
        .unwrap_or_else(MeshRenderOptions::world_opaque)
}

#[inline]
fn primitive_draw_flags(options: &MeshRenderOptions) -> u8 {
    let mut flags = 0u8;
    if matches!(options.transform_policy, MeshTransformPolicy::FollowCamera) {
        flags |= PRIMITIVE_DRAW_FOLLOW_VIEW;
    }
    if options.is_sky_role() {
        flags |= PRIMITIVE_DRAW_SKY_ROLE;
    }
    if matches!(options.role, MeshRenderRole::SkyBackground) {
        flags |= PRIMITIVE_DRAW_SKY_BACKGROUND;
    }
    if matches!(
        options.shadow_policy,
        MeshShadowPolicy::ReceiveOnly
            | MeshShadowPolicy::CastAndReceive
            | MeshShadowPolicy::ProfileControlled
    ) {
        flags |= PRIMITIVE_DRAW_RECEIVE_SHADOWS;
    }
    flags
}

#[inline]
fn has_primitive_flag(flags: u8, flag: u8) -> bool {
    (flags & flag) != 0
}

#[derive(Clone, Copy)]
struct PrimitivePassRoleCullRule {
    pass: SceneMeshPass,
    roles: &'static [MeshRenderRole],
    reason: &'static str,
}

const SKY_MESH_ROLES: &[MeshRenderRole] = &[
    MeshRenderRole::SkyBackground,
    MeshRenderRole::CelestialBillboard,
    MeshRenderRole::WeatherVolume,
];

const NON_WORLD_VIEWPORT_ROLES: &[MeshRenderRole] = &[
    MeshRenderRole::CollisionProxy,
    MeshRenderRole::EditorGizmo,
    MeshRenderRole::DebugPrimitive,
];

const PRIMITIVE_PASS_ROLE_CULL_RULES: &[PrimitivePassRoleCullRule] = &[
    PrimitivePassRoleCullRule {
        pass: SceneMeshPass::GBuffer,
        roles: SKY_MESH_ROLES,
        reason: "sky_role_not_allowed_in_gbuffer",
    },
    PrimitivePassRoleCullRule {
        pass: SceneMeshPass::Forward,
        roles: NON_WORLD_VIEWPORT_ROLES,
        reason: "non_world_mesh_role_not_allowed_in_viewport_forward",
    },
];

#[inline]
fn primitive_role_cull_reason(
    options: &MeshRenderOptions,
    pass: SceneMeshPass,
    draw_sky_visuals: bool,
    deferred: bool,
) -> Option<&'static str> {
    if options.is_sky_role() && !draw_sky_visuals {
        return Some("sky_visuals_disabled_by_runtime_profile");
    }
    if deferred
        && matches!(pass, SceneMeshPass::Forward)
        && !matches!(
            options.role,
            MeshRenderRole::SkyBackground
                | MeshRenderRole::CelestialBillboard
                | MeshRenderRole::WeatherVolume
                | MeshRenderRole::WorldTransparent
                | MeshRenderRole::FirstPersonViewModel
        )
    {
        return Some("opaque_role_routed_to_deferred_gbuffer");
    }
    PRIMITIVE_PASS_ROLE_CULL_RULES
        .iter()
        .find(|rule| rule.pass == pass && rule.roles.contains(&options.role))
        .map(|rule| rule.reason)
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
    deferred: bool,
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
        deferred,
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
    deferred: bool,
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

    let mut sky_entries: Vec<(
        f32,
        u64,
        Primitive,
        Mat4,
        Option<newengine_materials::MaterialRef>,
        u8,
        Option<SkyDomeRuntime>,
    )> = Vec::new();
    let mut entries: Vec<(
        f32,
        u64,
        Primitive,
        Mat4,
        Option<newengine_materials::MaterialRef>,
        u8,
        Option<SkyDomeRuntime>,
    )> = Vec::new();
    let mut sky_seen = 0usize;
    let mut sky_profile_culled = 0usize;
    for (id, prim, gt) in world.query2::<Primitive, GlobalTransform>() {
        if !display_visible_in_mode(world, id, runtime) {
            continue;
        }
        let sky_dome_runtime = world.get::<SkyDomeRuntime>(id);
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
        if runtime && !follows_view {
            if let Some(bounds) = world.get::<Bounds>(id) {
                let (center_ws, radius_ws) =
                    transform_sphere(gt.0, bounds.local_sphere.center, bounds.local_sphere.radius);
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
        } else {
            entries.push(entry);
        }
    }
    sort_by_distance_then_key(&mut sky_entries);
    sort_by_distance_then_key(&mut entries);
    entries.truncate(primitive_budget(runtime, false));
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
    let mut plan_cache: FxHashMap<PrimitivePlanKey, PrimitiveGpuPlan> = FxHashMap::default();
    let mut written_ubos = FxHashSet::<u64>::default();
    let mut sky_background_batches = InstanceBatchSet::default();
    let mut sky_foreground_batches = InstanceBatchSet::default();
    let mut opaque_batches = InstanceBatchSet::default();

    // Keep sky in ordered replay buckets. `InstanceBatchSet` sorts by pipeline / bind
    // group / mesh for performance, so the dome must not share the same unordered
    // set with sun/moon discs: draw authored dome first, sky foreground discs next,
    // then world opaque batches.
    for (_distance_sq, _entity_key, prim, model, material_ref, draw_flags, sky_runtime) in
        sky_entries.into_iter().chain(entries.into_iter())
    {
        let follows_view = has_primitive_flag(draw_flags, PRIMITIVE_DRAW_FOLLOW_VIEW);
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

        let instance = RenderInstanceRaw::new(
            model,
            viewproj * model,
            plan.base_color,
            plan.uv_transform,
            plan.material_params,
            plan.emissive_radiance,
            plan.alpha_cutoff,
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
            "primitive.batch.plan: pass='{}' plans={} shared_ubos={} batches=[sky_bg:{},sky_fg:{},opaque:{}] instances=[sky_bg:{},sky_fg:{},opaque:{}] policy='UBO keyed by pipeline texture set; mesh transform/material scalars stay in instance data'",
            pass.label(),
            plan_cache.len(),
            written_ubos.len(),
            sky_background_batches.batch_count(),
            sky_foreground_batches.batch_count(),
            opaque_batches.batch_count(),
            sky_background_batches.instance_count(),
            sky_foreground_batches.instance_count(),
            opaque_batches.instance_count(),
        );
    }

    if sky_background_batches.is_empty()
        && sky_foreground_batches.is_empty()
        && opaque_batches.is_empty()
    {
        return Ok(());
    }

    let ordered_batches = sky_background_batches
        .into_sorted_batches()
        .into_iter()
        .chain(sky_foreground_batches.into_sorted_batches())
        .chain(opaque_batches.into_sorted_batches())
        .collect::<Vec<_>>();
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
