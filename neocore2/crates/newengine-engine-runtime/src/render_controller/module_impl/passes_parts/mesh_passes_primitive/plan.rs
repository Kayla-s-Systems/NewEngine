use super::*;
use newengine_math::hash_combine_u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(in super::super) struct PrimitivePlanKey {
    primitive_id: u64,
    material_id: u64,
    color: [u32; 4],
    sky_pipeline: bool,
    shadow_pass: bool,
}

impl PrimitivePlanKey {
    #[inline]
    pub(in super::super) fn new(
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
pub(in super::super) struct PrimitiveGpuPlan {
    pub(in super::super) gpu: PrimitiveGpu,
    pub(in super::super) pipeline: PipelineId,
    pub(in super::super) bind_group: BindGroupId,
    pub(in super::super) base_texture: TextureId,
    pub(in super::super) normal_texture: TextureId,
    pub(in super::super) roughness_texture: TextureId,
    pub(in super::super) shadow_texture: TextureId,
    pub(in super::super) sampler: SamplerId,
    pub(in super::super) mesh_key: u64,
    pub(in super::super) base_color: [f32; 4],
    pub(in super::super) emissive_radiance: [f32; 3],
    pub(in super::super) alpha_cutoff: f32,
    pub(in super::super) uv_transform: [f32; 4],
    pub(in super::super) material_params: [f32; 4],
}

#[inline]
pub(in super::super) fn instance_batch_ubo_key(
    prefix: u64,
    pipeline: PipelineId,
    base_texture: TextureId,
    normal_texture: TextureId,
    roughness_texture: TextureId,
    shadow_texture: TextureId,
    local_shadow_texture: TextureId,
    sampler: SamplerId,
) -> u64 {
    let mut h = prefix;
    h = hash_combine_u64(h, pipeline.get() as u64);
    h = hash_combine_u64(h, base_texture.get() as u64);
    h = hash_combine_u64(h, normal_texture.get() as u64);
    h = hash_combine_u64(h, roughness_texture.get() as u64);
    h = hash_combine_u64(h, shadow_texture.get() as u64);
    h = hash_combine_u64(h, local_shadow_texture.get() as u64);
    hash_combine_u64(h, sampler.get() as u64)
}

pub(super) const PRIMITIVE_DRAW_FOLLOW_VIEW: u8 = 0x01;
pub(super) const PRIMITIVE_DRAW_SKY_ROLE: u8 = 0x02;
pub(super) const PRIMITIVE_DRAW_SKY_BACKGROUND: u8 = 0x04;
pub(super) const PRIMITIVE_DRAW_RECEIVE_SHADOWS: u8 = 0x08;
pub(super) const PRIMITIVE_DRAW_FOLIAGE_ROLE: u8 = 0x10;
#[inline]
pub(super) fn primitive_mesh_render_options(
    explicit: Option<&MeshRenderOptions>,
) -> MeshRenderOptions {
    explicit
        .cloned()
        .unwrap_or_else(MeshRenderOptions::world_opaque)
}

#[inline]
pub(super) fn primitive_draw_flags(options: &MeshRenderOptions) -> u8 {
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
    if matches!(options.role, MeshRenderRole::FoliageInstanced) {
        flags |= PRIMITIVE_DRAW_FOLIAGE_ROLE;
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
pub(super) fn has_primitive_flag(flags: u8, flag: u8) -> bool {
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
pub(super) fn primitive_role_cull_reason(
    options: &MeshRenderOptions,
    pass: SceneMeshPass,
    draw_sky_visuals: bool,
    deferred: bool,
) -> Option<&'static str> {
    if options.is_sky_role() && !draw_sky_visuals {
        return Some("sky_visuals_disabled_by_runtime_profile");
    }
    if matches!(options.role, MeshRenderRole::EditorGizmo) && matches!(pass, SceneMeshPass::GBuffer)
    {
        return Some("editor_gizmo_forward_only");
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
                | MeshRenderRole::EditorGizmo
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
pub(super) fn recenter_model_translation(mut model: Mat4, camera_position: Vec3) -> Mat4 {
    let mut cols = model.to_cols_array();
    let local_offset = Vec3::new(cols[12], cols[13], cols[14]);
    cols[12] = camera_position.x + local_offset.x;
    cols[13] = camera_position.y + local_offset.y;
    cols[14] = camera_position.z + local_offset.z;
    model = Mat4::from_cols_array(&cols);
    model
}
