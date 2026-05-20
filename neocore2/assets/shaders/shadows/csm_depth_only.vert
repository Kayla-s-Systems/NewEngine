#version 450
#extension GL_GOOGLE_include_directive : require
#include "csm_shadow_common.glsl"

layout(location = 0) in vec3 a_position;
layout(location = 1) in vec3 a_normal;
layout(location = 2) in vec2 a_uv;

layout(location = 0) out vec2 v_uv;

layout(push_constant) uniform CsmDepthPush {
    mat4 model;
    uint cascade_index;
    uint material_flags;
    uint bone_offset;
    uint _pad0;
} pc;

void main() {
    uint cascade = min(pc.cascade_index, uint(NEWENGINE_CSM_MAX_CASCADES - 1));
    vec3 light_dir = normalize(csm.light_dir_pcss_radius.xyz);
    float bias = csm_slope_scaled_bias(a_normal, light_dir);
    vec3 biased = a_position + normalize(a_normal) * bias - light_dir * bias;
    vec4 world = pc.model * vec4(biased, 1.0);
    gl_Position = csm.cascades[cascade].light_view_proj * world;
    v_uv = a_uv;
}
