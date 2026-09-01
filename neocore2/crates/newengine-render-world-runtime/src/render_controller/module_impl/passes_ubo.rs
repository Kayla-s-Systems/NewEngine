#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::EngineResult;
use newengine_math::Mat4;

use newengine_render_feature_api::PackedLights;

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
    for cascade in 0..newengine_render_feature_api::MAX_DIRECTIONAL_SHADOW_CASCADES {
        let cascade_cols = lights.shadow_cascade_light_mvp[cascade].to_cols_array();
        for (i, f) in cascade_cols.iter().enumerate() {
            let off = cascade_mvp_off + cascade * 64 + i * 4;
            bytes[off..off + 4].copy_from_slice(&f.to_ne_bytes());
        }
    }

    let shadow_off = 704;
    for i in 0..4 {
        let off = shadow_off + i * 4;
        bytes[off..off + 4].copy_from_slice(&lights.shadow_params[i].to_ne_bytes());
    }

    let shadow_extra_off = 720;
    for i in 0..4 {
        let off = shadow_extra_off + i * 4;
        bytes[off..off + 4].copy_from_slice(&lights.shadow_extra[i].to_ne_bytes());
    }

    let shadow_splits_off = 736;
    for i in 0..4 {
        let off = shadow_splits_off + i * 4;
        bytes[off..off + 4].copy_from_slice(&lights.shadow_cascade_splits[i].to_ne_bytes());
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
    #[test]
    fn lit_ubo_allocation_matches_packed_lighting_contract() {
        assert_eq!(
            newengine_material_domain_api::LIT_UBO_SIZE as usize,
            newengine_render_feature_api::PackedLights::UBO_SIZE,
            "material-domain UBO allocation must cover the complete packed lighting ABI",
        );
    }
}
