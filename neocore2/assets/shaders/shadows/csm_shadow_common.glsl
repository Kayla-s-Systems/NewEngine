#ifndef NEWENGINE_CSM_SHADOW_COMMON_GLSL
#define NEWENGINE_CSM_SHADOW_COMMON_GLSL

#define NEWENGINE_CSM_MAX_CASCADES 8

struct CsmCascadeData {
    mat4 light_view_proj;
    mat4 shadow_to_atlas;
    vec4 split_near_far_resolution_cascade;
    vec4 atlas_rect;
};

layout(std140, set = 0, binding = 0) uniform CsmFrameBlock {
    CsmCascadeData cascades[NEWENGINE_CSM_MAX_CASCADES];
    vec4 atlas_size_inv_size;
    vec4 light_dir_pcss_radius;
    vec4 depth_bias_normal_bias_alpha_ref_flags;
} csm;

float csm_slope_scaled_bias(vec3 normal_ws, vec3 light_dir_ws) {
    float ndotl = clamp(dot(normalize(normal_ws), -normalize(light_dir_ws)), 0.0, 1.0);
    return csm.depth_bias_normal_bias_alpha_ref_flags.x
         + csm.depth_bias_normal_bias_alpha_ref_flags.y * (1.0 - ndotl);
}

vec2 csm_atlas_uv(uint cascade_index, vec3 shadow_ndc) {
    vec4 rect = csm.cascades[cascade_index].atlas_rect;
    vec2 uv = shadow_ndc.xy * 0.5 + 0.5;
    return rect.xy + uv * rect.zw;
}

#endif
