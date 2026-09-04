#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::EngineResult;
use newengine_math::Mat4;

use newengine_render_feature_api::PackedLights;

#[inline]
fn pack_skinned_shadow_ubo(
    mvp: Mat4,
    uv_transform: [f32; 4],
    alpha_cutoff: f32,
    caster_bias: f32,
) -> [u8; PackedLights::UBO_SIZE] {
    // Keep the standard lit bind-group ABI, but pack only fields consumed by
    // game_sun_shadow_depth_skinned_v1.vert. All other fields remain zero.
    let mut bytes = [0u8; PackedLights::UBO_SIZE];

    for (i, value) in mvp.to_cols_array().iter().enumerate() {
        let off = i * 4;
        bytes[off..off + 4].copy_from_slice(&value.to_ne_bytes());
    }

    // u_emissive.w carries alpha_cutoff in the shared ABI.
    bytes[156..160].copy_from_slice(&alpha_cutoff.max(0.0).to_ne_bytes());

    for (i, value) in uv_transform.iter().enumerate() {
        let off = 352 + i * 4;
        bytes[off..off + 4].copy_from_slice(&value.to_ne_bytes());
    }

    // u_shadow_params.y is the only shadow parameter consumed by the skinned
    // shadow vertex shader; it controls conservative caster depth bias.
    bytes[708..712].copy_from_slice(&caster_bias.max(0.0).to_ne_bytes());
    bytes
}

#[inline]
pub(super) fn write_skinned_shadow_ubo(
    r: &mut dyn newengine_core::render::RenderApi,
    ubo: newengine_core::render::BufferId,
    mvp: Mat4,
    uv_transform: [f32; 4],
    alpha_cutoff: f32,
    caster_bias: f32,
) -> EngineResult<()> {
    let bytes = pack_skinned_shadow_ubo(mvp, uv_transform, alpha_cutoff, caster_bias);
    r.write_buffer(ubo, 0, &bytes)
}

#[inline]
pub(super) fn write_lit_ubo_ex(
    r: &mut dyn newengine_core::render::RenderApi,
    ubo: newengine_core::render::BufferId,
    mvp: Mat4,
    model: Mat4,
    base_color: [f32; 4],
    emissive_radiance: [f32; 3],
    alpha_cutoff: f32,
    uv_transform: [f32; 4],
    material_params: [f32; 4],
    lights: &PackedLights,
) -> EngineResult<()> {
    let mut bytes: [u8; PackedLights::UBO_SIZE] = [0u8; PackedLights::UBO_SIZE];

    let mvp_cols = mvp.to_cols_array();
    for (i, f) in mvp_cols.iter().enumerate() {
        let off = i * 4;
        bytes[off..off + 4].copy_from_slice(&f.to_ne_bytes());
    }

    let model_cols = model.to_cols_array();
    let model_off = 64;
    for (i, f) in model_cols.iter().enumerate() {
        let off = model_off + i * 4;
        bytes[off..off + 4].copy_from_slice(&f.to_ne_bytes());
    }

    let base_off = 128;
    for (i, component) in base_color.iter().enumerate() {
        let off = base_off + i * 4;
        bytes[off..off + 4].copy_from_slice(&component.to_ne_bytes());
    }

    // std140 vec3 is padded to vec4.
    let em_off = 144;
    for (i, component) in emissive_radiance.iter().enumerate() {
        let off = em_off + i * 4;
        bytes[off..off + 4].copy_from_slice(&component.to_ne_bytes());
    }
    bytes[em_off + 12..em_off + 16].copy_from_slice(&alpha_cutoff.max(0.0).to_ne_bytes());

    lights.write_into(&mut bytes);

    let uv_off = 352;
    for (i, component) in uv_transform.iter().enumerate() {
        let off = uv_off + i * 4;
        bytes[off..off + 4].copy_from_slice(&component.to_ne_bytes());
    }

    let mat_off = 368;
    for (i, component) in material_params.iter().enumerate() {
        let off = mat_off + i * 4;
        bytes[off..off + 4].copy_from_slice(&component.to_ne_bytes());
    }

    let light_mvp_off = 384;
    let light_mvp_cols = lights.shadow_light_mvp.to_cols_array();
    for (i, f) in light_mvp_cols.iter().enumerate() {
        let off = light_mvp_off + i * 4;
        bytes[off..off + 4].copy_from_slice(&f.to_ne_bytes());
    }

    let cascade_mvp_off = 448;
    for (cascade, cascade_mvp) in lights
        .shadow_cascade_light_mvp
        .iter()
        .take(newengine_render_feature_api::MAX_DIRECTIONAL_SHADOW_CASCADES)
        .enumerate()
    {
        let cascade_cols = cascade_mvp.to_cols_array();
        for (i, f) in cascade_cols.iter().enumerate() {
            let off = cascade_mvp_off + cascade * 64 + i * 4;
            bytes[off..off + 4].copy_from_slice(&f.to_ne_bytes());
        }
    }

    let shadow_off = 704;
    for (i, value) in lights.shadow_params.iter().enumerate() {
        let off = shadow_off + i * 4;
        bytes[off..off + 4].copy_from_slice(&value.to_ne_bytes());
    }

    let shadow_extra_off = 720;
    for (i, value) in lights.shadow_extra.iter().enumerate() {
        let off = shadow_extra_off + i * 4;
        bytes[off..off + 4].copy_from_slice(&value.to_ne_bytes());
    }

    let shadow_splits_off = 736;
    for (i, value) in lights.shadow_cascade_splits.iter().enumerate() {
        let off = shadow_splits_off + i * 4;
        bytes[off..off + 4].copy_from_slice(&value.to_ne_bytes());
    }

    for (base, values) in [
        (752usize, lights.shadow_pcss0),
        (768usize, lights.shadow_pcss1),
        (784usize, lights.cloud_shadow_map0),
        (800usize, lights.cloud_shadow_map1),
        (816usize, lights.cloud_shadow_map2),
        (832usize, lights.cloud_shadow_map3),
        (848usize, lights.cloud_shadow_map4),
        (864usize, lights.shadow_view_forward),
    ] {
        for (i, value) in values.iter().enumerate() {
            let off = base + i * 4;
            bytes[off..off + 4].copy_from_slice(&value.to_ne_bytes());
        }
    }

    r.write_buffer(ubo, 0, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f32_at(bytes: &[u8], offset: usize) -> f32 {
        f32::from_ne_bytes(bytes[offset..offset + 4].try_into().unwrap())
    }

    #[test]
    fn skinned_shadow_ubo_packs_only_shadow_shader_contract() {
        let mvp = Mat4::from_cols_array(&[
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0,
        ]);
        let bytes = pack_skinned_shadow_ubo(mvp, [2.0, 3.0, 4.0, 5.0], 0.42, 0.007);
        assert_eq!(f32_at(&bytes, 0), 1.0);
        assert_eq!(f32_at(&bytes, 60), 16.0);
        assert_eq!(f32_at(&bytes, 156), 0.42);
        assert_eq!(f32_at(&bytes, 352), 2.0);
        assert_eq!(f32_at(&bytes, 364), 5.0);
        assert_eq!(f32_at(&bytes, 708), 0.007);
        // Representative unused shared-lit fields remain zero.
        assert_eq!(f32_at(&bytes, 128), 0.0);
        assert_eq!(f32_at(&bytes, 384), 0.0);
        assert_eq!(f32_at(&bytes, 752), 0.0);
    }

    #[test]
    fn lit_ubo_allocation_matches_packed_lighting_contract() {
        assert_eq!(
            newengine_material_domain_api::LIT_UBO_SIZE as usize,
            newengine_render_feature_api::PackedLights::UBO_SIZE,
            "material-domain UBO allocation must cover the complete packed lighting ABI",
        );
    }
}
