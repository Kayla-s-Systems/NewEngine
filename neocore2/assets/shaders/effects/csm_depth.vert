#version 450
#extension GL_GOOGLE_include_directive : require
#include "common.glsl"

#ifndef SHADOW_DEPTH_BIAS
#define SHADOW_DEPTH_BIAS 0.000800
#endif
#ifndef SHADOW_NORMAL_BIAS
#define SHADOW_NORMAL_BIAS 0.020000
#endif

layout(location = 0) in vec3 a_position;
layout(location = 1) in vec3 a_normal;
layout(location = 2) in vec2 a_uv;

layout(push_constant) uniform ShadowPushConstants {
    mat4 light_view_proj;
    vec4 light_dir_and_cascade;
} pc;

void main() {
    vec3 n = normalize(a_normal);
    vec3 l = normalize(pc.light_dir_and_cascade.xyz);
    float ndotl = clamp(dot(n, -l), 0.0, 1.0);
    float normal_bias = float(SHADOW_NORMAL_BIAS) * (1.0 - ndotl);
    vec3 biased_position = a_position + n * normal_bias - l * float(SHADOW_DEPTH_BIAS);
    gl_Position = pc.light_view_proj * vec4(biased_position, 1.0);
}
