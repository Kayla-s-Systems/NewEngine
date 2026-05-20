#version 450
#extension GL_GOOGLE_include_directive : require
#include "csm_shadow_common.glsl"

layout(location = 0) in vec3 a_position;
layout(location = 1) in vec3 a_normal;
layout(location = 2) in vec2 a_uv;
layout(location = 3) in uvec4 a_bone_indices;
layout(location = 4) in vec4 a_bone_weights;

layout(std430, set = 2, binding = 0) readonly buffer SkinMatrices {
    mat4 bones[];
} skin;

layout(location = 0) out vec2 v_uv;

layout(push_constant) uniform CsmSkinnedDepthPush {
    mat4 model;
    uint cascade_index;
    uint material_flags;
    uint bone_offset;
    uint _pad0;
} pc;

mat4 skin_matrix() {
    return skin.bones[pc.bone_offset + a_bone_indices.x] * a_bone_weights.x
         + skin.bones[pc.bone_offset + a_bone_indices.y] * a_bone_weights.y
         + skin.bones[pc.bone_offset + a_bone_indices.z] * a_bone_weights.z
         + skin.bones[pc.bone_offset + a_bone_indices.w] * a_bone_weights.w;
}

void main() {
    uint cascade = min(pc.cascade_index, uint(NEWENGINE_CSM_MAX_CASCADES - 1));
    mat4 skin_m = skin_matrix();
    vec3 pos = (skin_m * vec4(a_position, 1.0)).xyz;
    vec3 n = normalize((skin_m * vec4(a_normal, 0.0)).xyz);
    vec3 light_dir = normalize(csm.light_dir_pcss_radius.xyz);
    float bias = csm_slope_scaled_bias(n, light_dir);
    vec4 world = pc.model * vec4(pos + n * bias - light_dir * bias, 1.0);
    gl_Position = csm.cascades[cascade].light_view_proj * world;
    v_uv = a_uv;
}
