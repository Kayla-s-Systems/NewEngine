#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::EngineResult;
use newengine_math::Mat4;

use super::lights::PackedLights;

#[inline]
pub(super) fn write_lit_ubo_ex(
    r: &mut dyn newengine_core::render::RenderApi,
    ubo: newengine_core::render::BufferId,
    mvp: Mat4,
    model: Mat4,
    base_color: [f32; 4],
    emissive_radiance: [f32; 3],
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
    for i in 0..4 {
        let off = base_off + i * 4;
        bytes[off..off + 4].copy_from_slice(&base_color[i].to_ne_bytes());
    }

    // std140 vec3 is padded to vec4.
    let em_off = 144;
    for i in 0..3 {
        let off = em_off + i * 4;
        bytes[off..off + 4].copy_from_slice(&emissive_radiance[i].to_ne_bytes());
    }
    bytes[em_off + 12..em_off + 16].copy_from_slice(&0.0_f32.to_ne_bytes());

    lights.write_into(&mut bytes);

    let uv_off = 352;
    for i in 0..4 {
        let off = uv_off + i * 4;
        bytes[off..off + 4].copy_from_slice(&uv_transform[i].to_ne_bytes());
    }

    let mat_off = 368;
    for i in 0..4 {
        let off = mat_off + i * 4;
        bytes[off..off + 4].copy_from_slice(&material_params[i].to_ne_bytes());
    }

    let light_mvp_off = 384;
    let light_mvp_cols = lights.shadow_light_mvp.to_cols_array();
    for (i, f) in light_mvp_cols.iter().enumerate() {
        let off = light_mvp_off + i * 4;
        bytes[off..off + 4].copy_from_slice(&f.to_ne_bytes());
    }

    let shadow_off = 448;
    for i in 0..4 {
        let off = shadow_off + i * 4;
        bytes[off..off + 4].copy_from_slice(&lights.shadow_params[i].to_ne_bytes());
    }

    r.write_buffer(ubo, 0, &bytes)
}
