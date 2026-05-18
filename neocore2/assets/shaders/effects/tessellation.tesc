#version 450
#include "common.glsl"

#ifndef TESS_FACTOR
#define TESS_FACTOR 4.000000
#endif
#ifndef TESS_MIN_DISTANCE
#define TESS_MIN_DISTANCE 8.000000
#endif
#ifndef TESS_MAX_DISTANCE
#define TESS_MAX_DISTANCE 96.000000
#endif

layout(vertices = 3) out;

layout(location = 0) in vec3 v_position[];
layout(location = 1) in vec3 v_normal[];
layout(location = 2) in vec2 v_uv[];

layout(location = 0) out vec3 tc_position[];
layout(location = 1) out vec3 tc_normal[];
layout(location = 2) out vec2 tc_uv[];

layout(push_constant) uniform TessControlPushConstants {
    vec4 camera_position_and_lod_bias;
} pc;

float tess_level_for_edge(vec3 a, vec3 b) {
    vec3 mid = (a + b) * 0.5;
    float d = distance(mid, pc.camera_position_and_lod_bias.xyz);
    float near_tess = max(float(TESS_FACTOR) + pc.camera_position_and_lod_bias.w, 1.0);
    float t = 1.0 - smoothstep(float(TESS_MIN_DISTANCE), float(TESS_MAX_DISTANCE), d);
    return clamp(mix(1.0, near_tess, t), 1.0, 64.0);
}

void main() {
    tc_position[gl_InvocationID] = v_position[gl_InvocationID];
    tc_normal[gl_InvocationID] = normalize(v_normal[gl_InvocationID]);
    tc_uv[gl_InvocationID] = v_uv[gl_InvocationID];
    if (gl_InvocationID == 0) {
        gl_TessLevelOuter[0] = tess_level_for_edge(v_position[1], v_position[2]);
        gl_TessLevelOuter[1] = tess_level_for_edge(v_position[2], v_position[0]);
        gl_TessLevelOuter[2] = tess_level_for_edge(v_position[0], v_position[1]);
        gl_TessLevelInner[0] = (gl_TessLevelOuter[0] + gl_TessLevelOuter[1] + gl_TessLevelOuter[2]) / 3.0;
    }
}
