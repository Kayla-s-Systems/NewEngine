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

#[inline]
fn asset_preview_model_scale(extent: Vec3) -> f32 {
    2.2 / extent.x.max(extent.y).max(extent.z).max(0.001)
}

#[inline]
fn asset_preview_model_transform(center: Vec3, extent: Vec3, angle_radians: f32) -> Mat4 {
    let scale = asset_preview_model_scale(extent);
    let rotation = newengine_math::Quat::from_rotation_y(angle_radians);

    // Center in model space first, then scale and rotate. Encoding the centering
    // as the final world translation (`-center * scale`) is incorrect for a
    // rotated asset: the rotation moves the scaled source center away from the
    // origin, leaving only a thin edge or an empty preview for off-origin meshes.
    Mat4::from_scale_rotation_translation(Vec3::splat(scale), rotation, Vec3::ZERO)
        * Mat4::from_translation(-center)
}

const PREVIEW_CAMERA_MIN_PITCH: f32 = -1.30;
const PREVIEW_CAMERA_MAX_PITCH: f32 = 1.30;
const PREVIEW_CAMERA_MIN_DISTANCE: f32 = 1.65;
const PREVIEW_CAMERA_MAX_DISTANCE: f32 = 12.0;

#[inline]
fn asset_preview_camera_position(view: newengine_render_feature_api::AssetPreviewView) -> Vec3 {
    let pitch = view
        .pitch_radians
        .clamp(PREVIEW_CAMERA_MIN_PITCH, PREVIEW_CAMERA_MAX_PITCH);
    let distance = view
        .distance
        .clamp(PREVIEW_CAMERA_MIN_DISTANCE, PREVIEW_CAMERA_MAX_DISTANCE);
    let horizontal = pitch.cos() * distance;
    Vec3::new(
        view.yaw_radians.sin() * horizontal,
        pitch.sin() * distance,
        view.yaw_radians.cos() * horizontal,
    )
}

const PREVIEW_GRID_HALF_EXTENT: f32 = 4.0;
const PREVIEW_GRID_MINOR_STEP: f32 = 0.25;
const PREVIEW_GRID_MAJOR_STEP: f32 = 1.0;

fn push_asset_preview_grid_quad(
    vertices: &mut Vec<newengine_primitives::PrimitiveVertex>,
    indices: &mut Vec<u32>,
    along_x: bool,
    offset: f32,
    half_width: f32,
) {
    let base = vertices.len() as u32;
    let e = PREVIEW_GRID_HALF_EXTENT;
    let (p0, p1, p2, p3) = if along_x {
        (
            [-e, 0.0, offset - half_width],
            [e, 0.0, offset - half_width],
            [e, 0.0, offset + half_width],
            [-e, 0.0, offset + half_width],
        )
    } else {
        (
            [offset - half_width, 0.0, -e],
            [offset + half_width, 0.0, -e],
            [offset + half_width, 0.0, e],
            [offset - half_width, 0.0, e],
        )
    };
    for pos in [p0, p1, p2, p3] {
        vertices.push(newengine_primitives::PrimitiveVertex {
            pos,
            nrm: [0.0, 1.0, 0.0],
            uv: [0.0, 0.0],
        });
    }
    // Double-sided preview pipeline makes winding irrelevant, but keep the
    // authored normal facing +Y for correct editor-preview illumination.
    indices.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
}

fn asset_preview_grid_mesh(
    step: f32,
    half_width: f32,
    omit_major_lines: bool,
) -> newengine_primitives::PrimitiveMesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let count = (PREVIEW_GRID_HALF_EXTENT / step).round() as i32;
    let major_every = (PREVIEW_GRID_MAJOR_STEP / step).round().max(1.0) as i32;
    for index in -count..=count {
        if index == 0 || (omit_major_lines && index % major_every == 0) {
            continue;
        }
        let offset = index as f32 * step;
        push_asset_preview_grid_quad(&mut vertices, &mut indices, true, offset, half_width);
        push_asset_preview_grid_quad(&mut vertices, &mut indices, false, offset, half_width);
    }
    newengine_primitives::PrimitiveMesh {
        vertices,
        indices,
        bounds_center: Vec3::ZERO,
        bounds_radius: PREVIEW_GRID_HALF_EXTENT * std::f32::consts::SQRT_2,
    }
}

fn asset_preview_axis_mesh(along_x: bool, half_width: f32) -> newengine_primitives::PrimitiveMesh {
    let mut vertices = Vec::with_capacity(4);
    let mut indices = Vec::with_capacity(6);
    push_asset_preview_grid_quad(&mut vertices, &mut indices, along_x, 0.0, half_width);
    newengine_primitives::PrimitiveMesh {
        vertices,
        indices,
        bounds_center: Vec3::ZERO,
        bounds_radius: PREVIEW_GRID_HALF_EXTENT,
    }
}

fn draw_asset_preview_grid_layer(
    this: &mut RuntimeRenderController,
    r: &mut dyn newengine_core::render::RenderApi,
    lit: newengine_material_domain_api::LitPipeline,
    viewproj: Mat4,
    lights: &PackedLights,
    identity: &str,
    mesh: newengine_primitives::PrimitiveMesh,
    model: Mat4,
    color: [f32; 4],
) -> newengine_core::EngineResult<()> {
    let primitive_id =
        newengine_primitives::PrimitiveId::new(newengine_primitives::fnv1a_64(identity));
    let gpu = if let Some(gpu) = this.gpu.meshes.prim_cache.get(&primitive_id).copied() {
        gpu
    } else {
        let gpu = upload_primitive_mesh(r, &mesh, identity)?;
        this.gpu.meshes.prim_cache.insert(primitive_id, gpu);
        gpu
    };
    let material_plan = LitMaterialPlan::from_resolved(None, color);
    let pipeline = lit.double_sided_pipeline;
    let key = newengine_primitives::fnv1a_64(&format!(
        "asset.preview.grid.ubo:{identity}:{}",
        pipeline.get()
    ));
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
    super::super::super::passes_ubo::write_lit_ubo_ex(
        r,
        per.ubo,
        viewproj * model,
        model,
        material_plan.base_color,
        [color[0] * 0.55, color[1] * 0.55, color[2] * 0.55],
        0.0,
        material_plan.uv_transform,
        material_plan.material_params,
        lights,
    )?;
    r.set_pipeline(pipeline)?;
    r.set_bind_group(0, per.bg)?;
    r.set_vertex_buffer(0, BufferSlice::new(gpu.vb, 0))?;
    r.set_index_buffer(BufferSlice::new(gpu.ib, 0), IndexFormat::U32)?;
    r.draw_indexed(DrawIndexedArgs::new(gpu.index_count))?;
    this.diagnostics
        .overlay_metrics
        .record_indexed_triangles(gpu.index_count);
    Ok(())
}

fn draw_asset_preview_editor_grid(
    this: &mut RuntimeRenderController,
    r: &mut dyn newengine_core::render::RenderApi,
    lit: newengine_material_domain_api::LitPipeline,
    viewproj: Mat4,
    lights: &PackedLights,
    floor_y: f32,
) -> newengine_core::EngineResult<()> {
    draw_asset_preview_grid_layer(
        this,
        r,
        lit,
        viewproj,
        lights,
        "asset.preview.grid.minor",
        asset_preview_grid_mesh(PREVIEW_GRID_MINOR_STEP, 0.004, true),
        Mat4::from_translation(Vec3::new(0.0, floor_y - 0.018, 0.0)),
        [0.19, 0.22, 0.26, 1.0],
    )?;
    draw_asset_preview_grid_layer(
        this,
        r,
        lit,
        viewproj,
        lights,
        "asset.preview.grid.major",
        asset_preview_grid_mesh(PREVIEW_GRID_MAJOR_STEP, 0.009, false),
        Mat4::from_translation(Vec3::new(0.0, floor_y - 0.012, 0.0)),
        [0.34, 0.38, 0.44, 1.0],
    )?;
    draw_asset_preview_grid_layer(
        this,
        r,
        lit,
        viewproj,
        lights,
        "asset.preview.grid.axis_x",
        asset_preview_axis_mesh(true, 0.014),
        Mat4::from_translation(Vec3::new(0.0, floor_y - 0.006, 0.0)),
        [0.72, 0.20, 0.18, 1.0],
    )?;
    draw_asset_preview_grid_layer(
        this,
        r,
        lit,
        viewproj,
        lights,
        "asset.preview.grid.axis_z",
        asset_preview_axis_mesh(false, 0.014),
        Mat4::from_translation(Vec3::new(0.0, floor_y - 0.006, 0.0)),
        [0.18, 0.42, 0.76, 1.0],
    )
}

pub fn draw_asset_preview_bundle(
    this: &mut RuntimeRenderController,
    r: &mut dyn newengine_core::render::RenderApi,
    bundle: &newengine_model_domain_api::ModelAssetBundle,
    lit: newengine_material_domain_api::LitPipeline,
    viewport_extent: newengine_core::render::Extent2D,
    preview_view: newengine_render_feature_api::AssetPreviewView,
) -> newengine_core::EngineResult<()> {
    if bundle.parts.is_empty() {
        return Ok(());
    }

    // Use the exact vertex AABB. Per-part bounding spheres deliberately
    // overestimate the other axes and made tall characters float above the
    // editor grid while also shrinking them unnecessarily in the preview.
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    let mut bounded_vertices = 0usize;
    for part in &bundle.parts {
        for vertex in &part.mesh.vertices {
            let position = Vec3::new(vertex.pos[0], vertex.pos[1], vertex.pos[2]);
            if !position.is_finite() {
                continue;
            }
            min[0] = min[0].min(position.x);
            min[1] = min[1].min(position.y);
            min[2] = min[2].min(position.z);
            max[0] = max[0].max(position.x);
            max[1] = max[1].max(position.y);
            max[2] = max[2].max(position.z);
            bounded_vertices += 1;
        }
    }
    if bounded_vertices == 0 {
        return Ok(());
    }
    let center = Vec3::new(
        (min[0] + max[0]) * 0.5,
        (min[1] + max[1]) * 0.5,
        (min[2] + max[2]) * 0.5,
    );
    let extent = Vec3::new(max[0] - min[0], max[1] - min[1], max[2] - min[2]);
    // Interaction orbits the camera. The model itself remains stable, so an
    // idle preview can keep its cached render target without per-frame work.
    let preview_scale = asset_preview_model_scale(extent);
    let model = asset_preview_model_transform(center, extent, 0.0);
    let floor_y = -extent.y * preview_scale * 0.5;

    let aspect = viewport_extent.width.max(1) as f32 / viewport_extent.height.max(1) as f32;
    let camera_target = Vec3::new(
        preview_view.target_offset[0],
        preview_view.target_offset[1],
        preview_view.target_offset[2],
    );
    let camera_position = camera_target + asset_preview_camera_position(preview_view);
    let view = Mat4::look_at_rh(camera_position, camera_target, Vec3::Y);
    // The authored UI samples a Vulkan render target as an image. Vulkan's
    // framebuffer Y direction is opposite to the mathematical camera space;
    // flip clip-space Y here so the preview texture is displayed upright.
    let mut projection = Mat4::perspective_rh(45.0_f32.to_radians(), aspect, 0.05, 100.0);
    projection.y_axis.y = -projection.y_axis.y;
    let viewproj = projection * view;
    let lights = PackedLights {
        ambient: [0.72, 0.76, 0.82, 0.9],
        dir_dir_intensity: [-0.45, -0.75, -0.5, 3.0],
        dir_color: [1.0, 0.97, 0.92, 0.0],
        ..PackedLights::default()
    }
    .with_camera_position([camera_position.x, camera_position.y, camera_position.z]);

    draw_asset_preview_editor_grid(this, r, lit, viewproj, &lights, floor_y)?;

    let force_double_sided = matches!(
        bundle.configuration.render_options.cull_policy,
        newengine_model_domain_api::MeshCullPolicy::None
    );
    let mut uploaded_parts = 0usize;
    for (index, part) in bundle.parts.iter().enumerate() {
        let identity = format!(
            "asset.preview:{}:{}:{}",
            bundle.source, bundle.dependency_graph.stable_cache_key, index
        );
        let primitive_id =
            newengine_primitives::PrimitiveId::new(newengine_primitives::fnv1a_64(&identity));
        let gpu = if let Some(gpu) = this.gpu.meshes.prim_cache.get(&primitive_id).copied() {
            gpu
        } else {
            let gpu = upload_primitive_mesh(r, &part.mesh, &identity)?;
            this.gpu.meshes.prim_cache.insert(primitive_id, gpu);
            uploaded_parts += 1;
            gpu
        };
        let resolved = newengine_materials::MaterialResolved {
            id: MaterialId::invalid(),
            desc: part.material.descriptor,
            textures: part.material.textures.clone(),
        };
        let material_plan =
            LitMaterialPlan::from_resolved(Some(&resolved), part.material.fallback_color);
        let base_texture = this.material_texture_or_default(
            r,
            material_plan.base_color_texture,
            lit.white_texture,
        );
        let normal_texture = this.material_texture_or_default(
            r,
            material_plan.normal_texture,
            lit.flat_normal_texture,
        );
        let roughness_texture =
            this.material_texture_or_default(r, material_plan.roughness_texture, lit.white_texture);
        let sampler = if material_plan.alpha_cutoff > 0.0 {
            lit.clamp_sampler
        } else if material_plan.has_textures() {
            lit.repeat_sampler
        } else {
            lit.clamp_sampler
        };
        let pipeline = if force_double_sided || material_plan.double_sided {
            lit.double_sided_pipeline
        } else {
            lit.pipeline
        };
        let key = newengine_primitives::fnv1a_64(&format!(
            "asset.preview.ubo:{}:{}:{}:{}:{}",
            identity,
            pipeline.get(),
            base_texture.get(),
            normal_texture.get(),
            roughness_texture.get(),
        ));
        let mut per = this.ensure_per_draw_ubo_with_binding(
            r,
            lit,
            key,
            base_texture,
            normal_texture,
            roughness_texture,
            lit.white_texture,
            sampler,
        )?;
        per.last_seen_frame = this.frame.frame_index;
        this.gpu.material.per_draw_ubo.insert(key, per);
        super::super::super::passes_ubo::write_lit_ubo_ex(
            r,
            per.ubo,
            viewproj * model,
            model,
            material_plan.base_color,
            material_plan.emissive_radiance,
            material_plan.alpha_cutoff,
            material_plan.uv_transform,
            material_plan.material_params,
            &lights,
        )?;
        r.set_pipeline(pipeline)?;
        r.set_bind_group(0, per.bg)?;
        r.set_vertex_buffer(0, BufferSlice::new(gpu.vb, 0))?;
        r.set_index_buffer(BufferSlice::new(gpu.ib, 0), IndexFormat::U32)?;
        r.draw_indexed(DrawIndexedArgs::new(gpu.index_count))?;
        this.diagnostics
            .overlay_metrics
            .record_indexed_triangles(gpu.index_count);
    }
    if uploaded_parts > 0 {
        newengine_ulog_api::ulog::info!(
            "asset preview: render packet uploaded source='{}' uploaded_parts={} total_parts={} graph_cache_key='{}' bounds_center=({:.3},{:.3},{:.3}) bounds_extent=({:.3},{:.3},{:.3}) preview_scale={:.6} first_base_color={:?}",
            bundle.source,
            uploaded_parts,
            bundle.parts.len(),
            bundle.dependency_graph.stable_cache_key,
            center.x,
            center.y,
            center.z,
            extent.x,
            extent.y,
            extent.z,
            2.2 / extent.x.max(extent.y).max(extent.z).max(0.001),
            bundle.parts.first().map(|part| part.material.descriptor.base_color)
        );
    }
    Ok(())
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

    type PrimitiveDrawEntry = (
        f32,
        u64,
        Primitive,
        Mat4,
        Option<newengine_materials::MaterialRef>,
        u8,
        Option<SkyDomeRuntime>,
    );

    let mut sky_entries: Vec<PrimitiveDrawEntry> = Vec::new();
    let mut entries: Vec<PrimitiveDrawEntry> = Vec::new();
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

#[cfg(test)]
mod asset_preview_transform_tests {
    use super::*;

    #[test]
    fn preview_transform_keeps_off_origin_bounds_center_at_origin() {
        let center = Vec3::new(128.0, -32.0, 75.0);
        let extent = Vec3::new(10.0, 4.0, 7.0);
        let transform = asset_preview_model_transform(center, extent, 1.1);
        let transformed_center = transform.transform_point3(center);
        assert!(
            transformed_center.length() < 0.0001,
            "{transformed_center:?}"
        );
    }

    #[test]
    fn preview_camera_position_respects_requested_distance() {
        let view = newengine_render_feature_api::AssetPreviewView {
            yaw_radians: 1.1,
            pitch_radians: 0.4,
            distance: 5.25,
            target_offset: [0.0, 0.0, 0.0],
        };
        let position = asset_preview_camera_position(view);
        assert!((position.length() - 5.25).abs() < 0.0001);
        assert!(position.y > 0.0);
    }

    #[test]
    fn preview_transform_normalizes_largest_extent() {
        let transform = asset_preview_model_transform(Vec3::ZERO, Vec3::new(10.0, 4.0, 7.0), 0.0);
        let transformed = transform.transform_vector3(Vec3::new(10.0, 0.0, 0.0));
        assert!((transformed.length() - 2.2).abs() < 0.0001);
    }
}
